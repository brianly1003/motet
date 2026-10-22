#!/usr/bin/env bash
# Synthetic import and real local whisper-server checks; ffmpeg only creates fixtures.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK=$(mktemp -d)
SERVER_PID=""
cleanup() {
  if [[ -n "$SERVER_PID" ]]; then kill "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT
export MOTETTO_TEST_MEDIA="$WORK"
export OLLAMA_HOST=http://127.0.0.1:11434
MODEL="${WHISPER_MODEL:-$HOME/.cache/whisper/ggml-base.bin}"
[[ -f "$MODEL" ]] || { echo "missing model: $MODEL"; exit 2; }
for tool in cargo ffmpeg whisper-server say python3; do command -v "$tool" >/dev/null; done
say -o "$WORK/speech.aiff" 'We decided to ship on Friday. Alex will fix the login bug.'
for ext in wav mp3 m4a flac; do
  ffmpeg -loglevel error -y -i "$WORK/speech.aiff" "$WORK/speech.$ext"
done
ffmpeg -loglevel error -y -f lavfi -i color=c=black:s=160x90:r=1 -i "$WORK/speech.aiff" \
  -c:v mpeg4 -c:a aac -shortest "$WORK/speech.mp4"
mkdir "$WORK/bin"
printf '#!/bin/sh\necho ffmpeg-must-not-run >&2\nexit 99\n' > "$WORK/bin/ffmpeg"
chmod +x "$WORK/bin/ffmpeg"
export PATH="$WORK/bin:$PATH"
cd "$ROOT"
cargo test -q -p motetto-core imported_formats_decode_without_ffmpeg -- --ignored
printf 'PASS  WAV, AIFF, MP3, M4A, MP4 and FLAC imports without ffmpeg\n'
export MOTETTO_TEST_WHISPER_PORT
MOTETTO_TEST_WHISPER_PORT=$(python3 - <<'PY'
import socket
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    print(sock.getsockname()[1])
PY
)
whisper-server -m "$MODEL" --host 127.0.0.1 --port "$MOTETTO_TEST_WHISPER_PORT" -bs 1 -bo 1 > "$WORK/server.log" 2>&1 &
SERVER_PID=$!
python3 - <<'PY'
import os, time, urllib.request
url = 'http://127.0.0.1:' + os.environ['MOTETTO_TEST_WHISPER_PORT'] + '/'
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
for _ in range(100):
    try:
        with opener.open(url, timeout=1) as reply:
            if reply.status == 200: break
    except OSError:
        time.sleep(.2)
else:
    raise SystemExit('local whisper-server did not become ready')
PY
cargo test -q -p motetto-core real_sidecar_transcribes_synthetic_speech -- --ignored || { cat "$WORK/server.log"; exit 1; }
printf 'PASS  local whisper-server speech and absolute timestamps\n'
cargo build --release -q
printf 'WHISPER_MODEL=%s\nWHISPER_VAD_MODEL=\nOLLAMA_MODEL=%s\nFIX_MODEL=\n' "$MODEL" "${OLLAMA_MODEL:-qwen3:8b}" > "$WORK/.env"
(cd "$WORK" && "$ROOT/target/release/motetto" run --file speech.mp4 > run.log 2>&1) || { cat "$WORK/run.log"; exit 1; }
grep -qi friday "$WORK/speech.txt"
test -s "$WORK/speech.md"
printf 'PASS  MP4 CLI import produces transcript and notes without ffmpeg\n'
