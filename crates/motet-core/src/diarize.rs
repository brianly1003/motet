//! Diarize stage (optional): split one track into speakers with sherpa-onnx, on this computer.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config;

const TOOL: &str = "sherpa-onnx-offline-speaker-diarization";

/// One speaker turn: diarizer cluster `speaker` talked from `start_ms` to `end_ms`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Turn {
    start_ms: u64,
    end_ms: u64,
    speaker: usize,
}

/// A model path setting; unset and empty both mean off.
fn model(key: &str) -> Option<String> {
    config::var(key).filter(|v| !v.is_empty())
}

/// Speaker turns in the decoder's 16 kHz mono WAV, or `None` when diarization is off (both
/// models unset).
pub(crate) fn turns(wav: &Path) -> Result<Option<Vec<Turn>>, Box<dyn Error>> {
    let (seg, emb) = match (
        model("DIARIZE_SEGMENTATION_MODEL"),
        model("DIARIZE_EMBEDDING_MODEL"),
    ) {
        (Some(seg), Some(emb)) => (seg, emb),
        (None, None) => return Ok(None),
        _ => {
            return Err(
                "set both DIARIZE_SEGMENTATION_MODEL and DIARIZE_EMBEDDING_MODEL, or neither"
                    .into(),
            );
        }
    };
    // The decoder writes 32-bit float for whisper; the diarizer reads only 16-bit PCM.
    let pcm = wav.with_extension("pcm16.wav");
    let result = to_pcm16(wav, &pcm).and_then(|()| run(&pcm, &seg, &emb));
    let _ = fs::remove_file(&pcm);
    result.map(Some)
}

fn run(wav: &Path, seg: &str, emb: &str) -> Result<Vec<Turn>, Box<dyn Error>> {
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(4));
    // 0.5 is the tool's default; 0.4-0.8 gave the same speakers on our synthetic calls.
    // no speaker-count hint; add --clustering.num-clusters if people ask for one.
    let out = Command::new(TOOL)
        .arg(format!("--segmentation.pyannote-model={seg}"))
        .arg(format!("--embedding.model={emb}"))
        .arg(format!("--segmentation.num-threads={threads}"))
        .arg(format!("--embedding.num-threads={threads}"))
        .arg("--clustering.cluster-threshold=0.5")
        .arg(wav)
        .output()
        .map_err(|e| format!("couldn't run {TOOL} ({e}); put its bin folder on PATH"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let last = stderr.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("");
        return Err(format!("{TOOL} failed ({}): {last}", out.status).into());
    }
    Ok(parse(&String::from_utf8_lossy(&out.stdout)))
}

/// Copies a float WAV to 16-bit PCM, sample by sample, so a long call never sits in memory.
fn to_pcm16(src: &Path, dst: &Path) -> Result<(), Box<dyn Error>> {
    let mut reader = hound::WavReader::open(src)?;
    let spec = hound::WavSpec {
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
        ..reader.spec()
    };
    let mut writer = hound::WavWriter::create(dst, spec)?;
    for sample in reader.samples::<f32>() {
        writer.write_sample(pcm16(sample?))?;
    }
    writer.finalize()?;
    Ok(())
}

fn pcm16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

/// Diarizer stdout rows `1.583 -- 3.406 speaker_00`. Other lines (the echoed config, which also
/// says "speaker", and progress) are skipped.
fn parse(stdout: &str) -> Vec<Turn> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            let start_ms = millis(cols.next()?)?;
            (cols.next()? == "--").then_some(())?;
            let end_ms = millis(cols.next()?)?;
            let speaker = cols.next()?.strip_prefix("speaker_")?.parse().ok()?;
            (cols.next().is_none() && end_ms > start_ms).then_some(Turn {
                start_ms,
                end_ms,
                speaker,
            })
        })
        .collect()
}

fn millis(secs: &str) -> Option<u64> {
    let s: f64 = secs.parse().ok()?;
    (s.is_finite() && s >= 0.0).then(|| (s * 1000.0).round() as u64)
}

/// For each transcript span (start, end in ms, in time order), the speaker who overlaps it most,
/// or the nearest turn when none overlaps. Speakers are renumbered by first appearance, so the
/// first voice heard is 0. Returns the numbers and how many distinct voices were heard, or
/// `None` when the diarizer found no turns.
pub(crate) fn assign(turns: &[Turn], spans: &[(u64, u64)]) -> Option<(Vec<usize>, usize)> {
    if turns.is_empty() {
        return None;
    }
    let mut order: Vec<usize> = vec![];
    let ids = spans
        .iter()
        .map(|&(start, end)| {
            let best = turns
                .iter()
                .max_by_key(|t| {
                    let overlap = t.end_ms.min(end).saturating_sub(t.start_ms.max(start));
                    let gap = t.start_ms.saturating_sub(end) + start.saturating_sub(t.end_ms);
                    (overlap, std::cmp::Reverse(gap))
                })
                .map_or(0, |t| t.speaker);
            order.iter().position(|&s| s == best).unwrap_or_else(|| {
                order.push(best);
                order.len() - 1
            })
        })
        .collect();
    Some((ids, order.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(start_ms: u64, end_ms: u64, speaker: usize) -> Turn {
        Turn {
            start_ms,
            end_ms,
            speaker,
        }
    }

    #[test]
    fn pcm16_scales_and_clips() {
        assert_eq!(pcm16(0.0), 0);
        assert_eq!(pcm16(0.5), 16384);
        assert_eq!(pcm16(-2.0), -i16::MAX);
        assert_eq!(pcm16(f32::NAN), 0);
    }

    #[test]
    fn parses_turns_and_skips_config_and_progress() {
        let out = "OfflineSpeakerDiarizationConfig(embedding=speaker.onnx)\nStarted\n\
                   1.583 -- 3.406 speaker_00\n9.346 -- 11.472 speaker_01\nprogress 92.86%\n\
                   2.0 -- 1.0 speaker_02\n4.0 -- 5.0 speaker_x\n";
        assert_eq!(parse(out), [turn(1583, 3406, 0), turn(9346, 11472, 1)]);
    }

    #[test]
    fn assign_picks_most_overlap_and_numbers_by_first_appearance() {
        // The diarizer calls the second voice speaker_00; the transcript hears speaker_01 first.
        let turns = [turn(0, 4000, 1), turn(4600, 9000, 0), turn(9500, 12000, 1)];
        let (ids, count) = assign(
            &turns,
            &[(100, 3900), (4500, 8800), (3800, 9600), (9600, 11000)],
        )
        .unwrap();
        assert_eq!(ids, [0, 1, 1, 0]);
        assert_eq!(count, 2);
    }

    #[test]
    fn assign_uses_nearest_turn_when_nothing_overlaps() {
        let turns = [turn(0, 1000, 0), turn(5000, 6000, 1)];
        assert_eq!(
            assign(&turns, &[(4200, 4800), (1100, 1200)]),
            Some((vec![0, 1], 2))
        );
    }

    #[test]
    fn assign_without_turns_is_none() {
        assert_eq!(assign(&[], &[(0, 1000)]), None);
    }
}
