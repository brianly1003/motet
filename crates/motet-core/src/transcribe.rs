//! Transcribe stage: each track through Rust decoding (16 kHz mono) and whisper-cli, merged by time.
//! With the diarization models set, the call side and unlabelled recordings are split into speakers.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::{config, decode, diarize};

/// Who a track belongs to, from its file name.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Speaker {
    Me,
    Them,
    Unknown,
    /// A voice the diarizer told apart: "Speaker 2" and up on a call (Me is the first), from
    /// "Speaker 1" in a single-track recording.
    Voice(usize),
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

    fn label(self) -> String {
        match self {
            Speaker::Me => "Me: ".into(),
            Speaker::Them => "Them: ".into(),
            Speaker::Unknown => String::new(),
            Speaker::Voice(n) => format!("Speaker {n}: "),
        }
    }

    /// The label for the `id`-th voice the diarizer heard on this track (0 = first heard).
    fn voice(self, id: usize) -> Self {
        match self {
            Speaker::Them => Speaker::Voice(id + 2),
            _ => Speaker::Voice(id + 1),
        }
    }
}

/// One whisper segment placed on the call's timeline.
#[derive(Debug, Clone, PartialEq)]
struct Segment {
    start_ms: u64,
    end_ms: u64,
    microphone: bool,
    system: bool,
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
    transcript_parts(&[(sources.to_vec(), 0)], false)
}

/// Desktop recording parts use elapsed recording time, excluding the Stop/Resume pause.
/// Quiet mode prevents whisper from printing private transcript text to desktop logs.
pub fn transcript_parts(
    parts: &[(Vec<String>, u64)],
    quiet: bool,
) -> Result<String, Box<dyn Error>> {
    // Transcribe each track on its own: mic = "Me", system audio = "Them". Merge by timestamp.
    // tracks are aligned by start time only (ms-level skew); fine for transcripts.
    let model = config::var("WHISPER_MODEL").ok_or("set WHISPER_MODEL to a ggml model path")?;
    let mut segments = vec![];
    for (sources, offset_ms) in parts {
        for src in sources {
            let speaker = Speaker::from_path(src);
            let stem = Path::new(src)
                .with_extension("")
                .to_string_lossy()
                .into_owned();
            let wav16 = format!("{stem}.16k.wav");
            decode::to_wav(Path::new(src), Path::new(&wav16))?;
            println!("Transcribing {src} with {model}...");
            let mut whisper = crate::tools::command("whisper-cli");
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
            if quiet {
                whisper
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
            }
            run(&mut whisper)?;
            let rows = parse_csv(&fs::read_to_string(format!("{stem}.csv"))?);
            let speakers = if speaker == Speaker::Me {
                vec![speaker; rows.len()]
            } else {
                voices(src, Path::new(&wav16), speaker, &rows)
            };
            for ((start_ms, end_ms, text), label) in rows.into_iter().zip(speakers) {
                segments.push(Segment {
                    start_ms: start_ms.saturating_add(*offset_ms),
                    end_ms: end_ms.saturating_add(*offset_ms),
                    microphone: speaker == Speaker::Me,
                    system: speaker == Speaker::Them,
                    speaker: label,
                    text,
                });
            }
        }
    }
    Ok(merge(segments))
}

/// One speaker per whisper row: the diarizer's voices when it hears two or more, else the
/// track's own label. Diarizing is optional, so a failure is reported and the track keeps its
/// label instead of losing the meeting.
fn voices(src: &str, wav16: &Path, track: Speaker, rows: &[(u64, u64, String)]) -> Vec<Speaker> {
    let spans: Vec<(u64, u64)> = rows.iter().map(|(s, e, _)| (*s, *e)).collect();
    let turns = match diarize::turns(wav16) {
        Ok(Some(turns)) => turns,
        Ok(None) => return vec![track; rows.len()],
        Err(e) => {
            eprintln!("Speaker labels skipped for {src}: {e}");
            return vec![track; rows.len()];
        }
    };
    match diarize::assign(&turns, &spans) {
        Some((ids, count)) if count >= 2 => {
            println!("Found {count} speakers in {src}");
            ids.into_iter().map(|id| track.voice(id)).collect()
        }
        _ => vec![track; rows.len()],
    }
}

/// Orders segments by start time and renders one `[mm:ss] Me: text` line each. The sort is
/// stable, so segments starting together keep the order their tracks were given in.
fn merge(mut segments: Vec<Segment>) -> String {
    // Unknown single-file imports are not evidence of system-audio echo.
    let indices: Vec<_> = segments
        .iter()
        .enumerate()
        .filter(|(_, s)| s.microphone || s.system)
        .map(|(i, _)| i)
        .collect();
    let rows: Vec<_> = indices
        .iter()
        .map(|&i| {
            let s = &segments[i];
            (s.start_ms, s.end_ms, s.text.as_str(), s.microphone)
        })
        .collect();
    let duplicates = crate::echo::duplicates(&rows);
    let remove: std::collections::HashSet<_> = indices
        .into_iter()
        .zip(duplicates)
        .filter_map(|(i, duplicate)| duplicate.then_some(i))
        .collect();
    let mut index = 0;
    segments.retain(|_| {
        let keep = !remove.contains(&index);
        index += 1;
        keep
    });
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
fn parse_csv(csv: &str) -> Vec<(u64, u64, String)> {
    csv.lines()
        .filter_map(|line| {
            let mut cols = line.splitn(3, ',');
            let start = cols.next()?.parse().ok()?;
            let end = cols.next()?.parse().ok()?;
            let field = cols.next()?.trim();
            let text = field
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(field)
                .replace("\"\"", "\"");
            (!text.trim().is_empty()).then(|| (start, end, text.trim().to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_filters_mic_echo_of_diarized_system_but_not_unknown_imports() {
        let mic = Segment {
            start_ms: 100,
            end_ms: 1200,
            microphone: true,
            system: false,
            speaker: Speaker::Me,
            text: "One speaker in the video".into(),
        };
        let system = Segment {
            start_ms: 0,
            end_ms: 1000,
            microphone: false,
            system: true,
            speaker: Speaker::Voice(2),
            text: "One speaker in the video".into(),
        };
        assert_eq!(
            merge(vec![mic.clone(), system.clone()]),
            "[00:00] Speaker 2: One speaker in the video\n"
        );
        let unknown = Segment {
            system: false,
            speaker: Speaker::Unknown,
            ..system
        };
        assert!(merge(vec![mic, unknown]).contains("Me: One speaker"));
    }

    #[test]
    fn parses_whisper_csv() {
        let csv = "start,end,text\n0,2000,\" Hi, all.\"\n2000,4000,\" He said \"\"go\"\".\"\n4000,5000,\" \"\n";
        assert_eq!(
            parse_csv(csv),
            [
                (0, 2000, "Hi, all.".into()),
                (2000, 4000, "He said \"go\".".into())
            ]
        );
    }

    #[test]
    fn parses_quotes_at_field_edges() {
        assert_eq!(
            parse_csv("0,1000,\" Say \"\"go\"\"\"\n1000,2000,\"\"\"yes\"\"\"\n"),
            [
                (0, 1000, "Say \"go\"".into()),
                (1000, 2000, "\"yes\"".into())
            ]
        );
    }

    #[test]
    fn speaker_comes_from_track_name() {
        assert_eq!(Speaker::from_path("call.mic.wav"), Speaker::Me);
        assert_eq!(Speaker::from_path("call.system.wav"), Speaker::Them);
        assert_eq!(Speaker::from_path("call.m4a"), Speaker::Unknown);
    }

    #[test]
    fn voices_number_from_two_on_a_call_and_from_one_alone() {
        assert_eq!(Speaker::Them.voice(0), Speaker::Voice(2));
        assert_eq!(Speaker::Unknown.voice(0), Speaker::Voice(1));
        assert_eq!(Speaker::Voice(3).label(), "Speaker 3: ");
    }

    #[test]
    fn merge_orders_by_start_time_across_tracks() {
        let seg = |start_ms, speaker, text: &str| Segment {
            start_ms,
            end_ms: start_ms + 1000,
            microphone: speaker == Speaker::Me,
            system: speaker == Speaker::Them,
            speaker,
            text: text.into(),
        };
        let merged = merge(vec![
            seg(3_170, Speaker::Me, "I can take it."),
            seg(65_000, Speaker::Me, "Done."),
            seg(0, Speaker::Them, "Who can take the bug?"),
            seg(3_170, Speaker::Them, "Same moment."),
            seg(0, Speaker::Unknown, "No label."),
            seg(3_170, Speaker::Voice(2), "Another voice."),
        ]);
        assert_eq!(
            merged,
            "[00:00] Them: Who can take the bug?\n\
             [00:00] No label.\n\
             [00:03] Me: I can take it.\n\
             [00:03] Them: Same moment.\n\
             [00:03] Speaker 2: Another voice.\n\
             [01:05] Me: Done.\n"
        );
    }
}
