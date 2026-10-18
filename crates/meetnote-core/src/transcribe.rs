//! Transcribe stage: each track through ffmpeg (16 kHz mono) and whisper-cli, merged by time.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config;

/// Who a track belongs to, from its file name.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Speaker {
    Me,
    Them,
    Unknown,
}

impl Speaker {
    /// `x.mic.wav` is Me, `x.system.wav` is Them; any other file is unlabelled.
    fn from_path(path: &str) -> Self {
        if path.ends_with(".mic.wav") {
            Speaker::Me
        } else if path.ends_with(".system.wav") {
            Speaker::Them
        } else {
            Speaker::Unknown
        }
    }

    fn label(self) -> &'static str {
        match self {
            Speaker::Me => "Me: ",
            Speaker::Them => "Them: ",
            Speaker::Unknown => "",
        }
    }
}

/// One whisper segment placed on the call's timeline.
#[derive(Debug, Clone, PartialEq)]
struct Segment {
    start_ms: u64,
    speaker: Speaker,
    text: String,
}

fn run(cmd: &mut Command) -> Result<(), Box<dyn Error>> {
    let status = cmd.status()?;
    if !status.success() {
        return Err(format!("{cmd:?} failed: {status}").into());
    }
    Ok(())
}

/// Transcribes each track on its own and merges the lines into one `[mm:ss] Me: …` transcript.
pub fn transcript(sources: &[String]) -> Result<String, Box<dyn Error>> {
    // Transcribe each track on its own: mic = "Me", system audio = "Them". Merge by timestamp.
    // tracks are aligned by start time only (ms-level skew); fine for transcripts.
    let model = config::var("WHISPER_MODEL").ok_or("set WHISPER_MODEL to a ggml model path")?;
    let mut segments = vec![];
    for src in sources {
        let speaker = Speaker::from_path(src);
        let stem = Path::new(src)
            .with_extension("")
            .to_string_lossy()
            .into_owned();
        let wav16 = format!("{stem}.16k.wav");
        run(Command::new("ffmpeg").args([
            "-y",
            "-loglevel",
            "error",
            "-i",
            src,
            "-ar",
            "16000",
            "-ac",
            "1",
            &wav16,
        ]))?;
        println!("Transcribing {src} with {model}...");
        let mut whisper = Command::new("whisper-cli");
        whisper.args([
            "-m", &model, "-f", &wav16, "-l", "auto", "-np", "-ocsv", "-of", &stem,
        ]);
        // Greedy decoding: 1.5x faster than the default 5-beam search with the same accuracy
        // (10 min of Vietnamese call: 54 s vs 80 s, differences mostly filler words). Splitting
        // the audio (-p) was faster still but dropped speech at the seams, so it stays off.
        whisper.args(["-bs", "1", "-bo", "1"]);
        // VAD gives real segment start times (else leading silence is folded in) and
        // stops hallucinated lines in the long silent stretches of each track.
        if let Some(vad) = config::var("WHISPER_VAD_MODEL").filter(|v| !v.is_empty()) {
            whisper.args(["--vad", "-vm", &vad]);
        }
        // Names whisper can't know (new products, people, project codes) bias it toward the right
        // spelling; tested not to push English words into a Vietnamese call.
        // whisper keeps only the last ~224 prompt tokens; long lists lose their start.
        if let Some(terms) = config::terms() {
            whisper.args(["--prompt", &terms]);
        }
        run(&mut whisper)?;
        for (start_ms, text) in parse_csv(&fs::read_to_string(format!("{stem}.csv"))?) {
            segments.push(Segment {
                start_ms,
                speaker,
                text,
            });
        }
    }
    Ok(merge(segments))
}

/// Orders segments by start time and renders one `[mm:ss] Me: text` line each. The sort is
/// stable, so segments starting together keep the order their tracks were given in.
fn merge(mut segments: Vec<Segment>) -> String {
    segments.sort_by_key(|s| s.start_ms);
    segments
        .iter()
        .map(|s| {
            format!(
                "[{:02}:{:02}] {}{}\n",
                s.start_ms / 60_000,
                s.start_ms / 1000 % 60,
                s.speaker.label(),
                s.text
            )
        })
        .collect()
}

/// whisper-cli `-ocsv` rows: `start_ms,end_ms,"text"` (header row skipped).
fn parse_csv(csv: &str) -> Vec<(u64, String)> {
    csv.lines()
        .filter_map(|line| {
            let mut cols = line.splitn(3, ',');
            let start = cols.next()?.parse().ok()?;
            let field = cols.nth(1)?.trim();
            let text = field
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(field)
                .replace("\"\"", "\"");
            (!text.trim().is_empty()).then(|| (start, text.trim().to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_whisper_csv() {
        let csv = "start,end,text\n0,2000,\" Hi, all.\"\n2000,4000,\" He said \"\"go\"\".\"\n4000,5000,\" \"\n";
        assert_eq!(
            parse_csv(csv),
            [(0, "Hi, all.".into()), (2000, "He said \"go\".".into())]
        );
    }

    #[test]
    fn parses_quotes_at_field_edges() {
        assert_eq!(
            parse_csv("0,1000,\" Say \"\"go\"\"\"\n1000,2000,\"\"\"yes\"\"\"\n"),
            [(0, "Say \"go\"".into()), (1000, "\"yes\"".into())]
        );
    }

    #[test]
    fn speaker_comes_from_track_name() {
        assert_eq!(Speaker::from_path("call.mic.wav"), Speaker::Me);
        assert_eq!(Speaker::from_path("call.system.wav"), Speaker::Them);
        assert_eq!(Speaker::from_path("call.m4a"), Speaker::Unknown);
    }

    #[test]
    fn merge_orders_by_start_time_across_tracks() {
        let seg = |start_ms, speaker, text: &str| Segment {
            start_ms,
            speaker,
            text: text.into(),
        };
        let merged = merge(vec![
            seg(3_170, Speaker::Me, "I can take it."),
            seg(65_000, Speaker::Me, "Done."),
            seg(0, Speaker::Them, "Who can take the bug?"),
            seg(3_170, Speaker::Them, "Same moment."),
            seg(0, Speaker::Unknown, "No label."),
        ]);
        assert_eq!(
            merged,
            "[00:00] Them: Who can take the bug?\n\
             [00:00] No label.\n\
             [00:03] Me: I can take it.\n\
             [00:03] Them: Same moment.\n\
             [01:05] Me: Done.\n"
        );
    }
}
