#!/usr/bin/env bash
# Time one live-caption request at 2, 5 and 10 s slice lengths on a local whisper-server.
# Usage: scripts/live-bench.sh AUDIO START_S [MODEL] [-- extra whisper-server args]
# Example: scripts/live-bench.sh ~/Documents/Motet/imports/<id>/part-0000.mic.wav 98 \
#            ~/.cache/whisper/ggml-large-v3-q5_0.bin -- -ac 512
# Prints "<len>s <seconds> | <text>" twice per length (the first request warms up and is dropped).
set -euo pipefail
AUDIO=${1:?audio file}; START=${2:?start second}; MODEL=${3:-$HOME/.cache/whisper/ggml-large-v3-q5_0.bin}
shift $(( $# < 3 ? $# : 3 )); [[ ${1:-} == -- ]] && shift
VAD=${WHISPER_VAD_MODEL:-$HOME/.cache/whisper/ggml-silero-v5.1.2.bin}
PORT=${LIVE_BENCH_PORT:-18391}
for tool in whisper-server ffmpeg curl python3; do command -v "$tool" >/dev/null; done
[[ -f "$MODEL" && -f "$VAD" ]] || { echo "missing model: $MODEL or $VAD"; exit 2; }
WORK=$(mktemp -d)
SERVER_PID=""
cleanup() {
  if [[ -n "$SERVER_PID" ]]; then kill "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT
for len in 2 5 10; do
  ffmpeg -v error -y -ss "$START" -t "$len" -i "$AUDIO" -ar 16000 -ac 1 -c:a pcm_s16le "$WORK/s$len.wav"
done
# Same server flags as app/src-tauri/src/speech.rs, plus any extra arguments under test.
whisper-server --host 127.0.0.1 --port "$PORT" -m "$MODEL" -l auto -nlp --vad --vad-model "$VAD" "$@" \
  >/dev/null 2>&1 &
SERVER_PID=$!
until curl -s --noproxy '*' -o /dev/null "http://127.0.0.1:$PORT/"; do
  kill -0 "$SERVER_PID" 2>/dev/null || { echo "whisper-server exited"; exit 1; }
  sleep 0.2
done
for len in 2 5 10; do
  for rep in 0 1 2; do
    out=$(curl -s --noproxy '*' -w '\n%{time_total}' -F "file=@$WORK/s$len.wav" -F response_format=json \
      -F language=auto -F beam_size=1 -F best_of=1 "http://127.0.0.1:$PORT/inference")
    text=$(head -1 <<<"$out" | python3 -I -c 'import json,sys;print(json.load(sys.stdin).get("text","").strip()[:70])')
    [[ $rep -gt 0 ]] && echo "${len}s $(tail -1 <<<"$out")s | $text"
  done
done
