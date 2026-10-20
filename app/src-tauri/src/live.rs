//! Live preview reads flushed audio on a background thread; capture never waits for the model.
use motet_core::{events::Stop, slice};
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};
use tauri::Manager;

#[derive(Default)]
pub struct State {
    pub levels: [Arc<AtomicU32>; 2],
    transcript: Mutex<(String, String, String)>,
}
#[tauri::command]
pub fn get_levels(state: tauri::State<'_, State>) -> [f32; 2] {
    state
        .levels
        .each_ref()
        .map(|level| f32::from_bits(level.swap(0, Ordering::Relaxed)))
}
#[tauri::command]
pub fn live_transcript(state: tauri::State<'_, State>) -> Result<(String, String, String), String> {
    state
        .transcript
        .lock()
        .map(|text| text.clone())
        .map_err(|_| "Live transcript is unavailable".into())
}

pub fn start(
    app: tauri::AppHandle,
    id: String,
    base: String,
    offset: u64,
    stop: Stop,
) -> Result<(), String> {
    // Lines from earlier parts (before a Pause or Stop) stay; this part's lines follow them.
    let earlier = {
        let state = app.state::<State>();
        let mut text = state
            .transcript
            .lock()
            .map_err(|_| "Live transcript is unavailable")?;
        if text.0 != id {
            *text = (id.clone(), String::new(), String::new());
        }
        text.2.clear();
        text.1.clone()
    };
    std::thread::Builder::new()
        .name("motet-live-transcript".into())
        .spawn(move || {
            static PREVIEW: Mutex<()> = Mutex::new(());
            let guard = loop {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                if let Ok(guard) = PREVIEW.try_lock() {
                    break guard;
                }
                std::thread::sleep(Duration::from_millis(100));
            };
            let _guard = guard;
            let mut server = match crate::speech::Server::start(&stop) {
                Ok(server) => server,
                Err(_) => {
                    failed(&app, &id, &stop);
                    return;
                }
            };
            let mut cursor = 0;
            let mut recent = Vec::new();
            while !stop.load(Ordering::SeqCst) {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let directory = Path::new(&base).with_extension("live");
                if fs::create_dir_all(&directory).is_err() {
                    break;
                }
                let result = (|| -> Result<Option<(String, u64)>, Box<dyn std::error::Error>> {
                    let mut duration = u64::MAX;
                    for side in ["mic", "system"] {
                        let source = format!("{base}.{side}.wav");
                        let Ok(reader) = hound::WavReader::open(&source) else {
                            return Ok(None);
                        };
                        duration = duration.min(
                            u64::from(reader.duration()) * 1000
                                / u64::from(reader.spec().sample_rate),
                        );
                    }
                    let Some(end) = preview_end(&base, cursor, duration)? else {
                        return Ok(None);
                    };
                    let segments = transcribe_slice(
                        &mut server,
                        &base,
                        &directory,
                        (cursor, end),
                        offset,
                        &stop,
                    )?;
                    recent.extend(segments);
                    recent.sort_by_key(|segment| segment.0);
                    let rows: Vec<_> = recent
                        .iter()
                        .map(|(start, end, side, text)| {
                            (*start, *end, text.as_str(), *side == "mic")
                        })
                        .collect();
                    let duplicates = motet_core::echo::duplicates(&rows);
                    let text = recent
                        .iter()
                        .zip(duplicates)
                        .filter(|(_, duplicate)| !duplicate)
                        .map(|((ms, _, side, text), _)| {
                            format!(
                                "[{:02}:{:02}] {}: {}\n",
                                ms / 60000,
                                ms / 1000 % 60,
                                if *side == "mic" { "Me" } else { "Them" },
                                text.replace(['\n', '\r'], " ")
                            )
                        })
                        .collect::<String>();
                    // Keep bounded context so echo across adjacent preview slices is also removed.
                    while recent.iter().map(|row| row.3.len()).sum::<usize>() > 32768
                        && recent.len() > 1
                    {
                        recent.remove(0);
                    }
                    Ok(Some((text, end)))
                })();
                let _ = fs::remove_dir_all(&directory);
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                match result {
                    Ok(Some((text, end))) => {
                        cursor = end;
                        let state = app.state::<State>();
                        if let Ok(mut current) = state.transcript.lock()
                            && current.0 == id
                        {
                            current.1 = format!("{earlier}{text}");
                            // Only the recent preview is retained here; final processing keeps the full call.
                            if current.1.len() > 32_768
                                && let Some(end) = current.1.as_bytes()[16_384..]
                                    .iter()
                                    .position(|b| *b == b'\n')
                            {
                                current.1.drain(..16_384 + end + 1);
                            }
                        }
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(250)),
                    Err(_) => {
                        failed(&app, &id, &stop);
                        break;
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}
type Row = (u64, u64, &'static str, String);

/// Transcribes `cursor..end` of both tracks, skipping a track that is digital silence.
fn transcribe_slice(
    server: &mut crate::speech::Server,
    base: &str,
    directory: &Path,
    (cursor, end): (u64, u64),
    offset: u64,
    stop: &Stop,
) -> Result<Vec<Row>, Box<dyn std::error::Error>> {
    let mut segments = Vec::new();
    for side in ["mic", "system"] {
        let file = directory.join(format!("preview.{side}.wav"));
        slice::write(Path::new(&format!("{base}.{side}.wav")), &file, cursor, end)?;
        let decoded = directory.join(format!("preview.{side}.16k.wav"));
        motet_core::decode_audio(&file, &decoded)?;
        let samples = hound::WavReader::open(decoded)?
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()?;
        // a peak check; a live mic's room noise passes it (measured 2026-10-19), but
        // skipping by RMS level saved no request on a real call. Needs a VAD to do better.
        if samples.iter().any(|sample| sample.abs() > 0.005) {
            let prompt = motet_core::config::terms();
            for segment in server.transcribe(&samples, offset + cursor, prompt.as_deref(), stop)? {
                segments.push((segment.start_ms, segment.end_ms, side, segment.text));
            }
        }
    }
    Ok(segments)
}

// Preview 3-10 s at a time, ending in the latest pause both tracks share, so a word or a
// sentence is not split and each slice usually holds one language. When the model is behind,
// the slice grows to 10 s and still ends in a pause, so catch-up never skips audio.
fn preview_end(
    base: &str,
    cursor: u64,
    duration: u64,
) -> Result<Option<u64>, Box<dyn std::error::Error>> {
    if duration.saturating_sub(cursor) < 3000 {
        return Ok(None);
    }
    let end = duration.min(cursor + 10_000);
    let mic = slice::levels(Path::new(&format!("{base}.mic.wav")), cursor, end)?;
    let system = slice::levels(Path::new(&format!("{base}.system.wav")), cursor, end)?;
    let levels: Vec<f32> = mic.iter().zip(&system).map(|(a, b)| a.max(*b)).collect();
    Ok(slice::cut(&levels, 30, 100, false).map(|frames| cursor + frames as u64 * slice::FRAME_MS))
}

fn failed(app: &tauri::AppHandle, id: &str, stop: &Stop) {
    if stop.load(Ordering::SeqCst) {
        return;
    }
    if let Ok(mut text) = app.state::<State>().transcript.lock()
        && text.0 == id
    {
        text.2 =
            "Live captions are unavailable. The full transcript will be written after the call."
                .into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track(path: &Path, pattern: &str) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for c in pattern.chars() {
            for i in 0..4800 {
                let level = if c == '1' { 0.1 } else { 0.0 };
                writer
                    .write_sample(if i % 2 == 0 { level } else { -level })
                    .unwrap();
            }
        }
        writer.finalize().unwrap();
    }
    #[test]
    fn preview_ends_in_a_shared_pause_and_catches_up_without_skipping_audio() {
        let root = std::env::temp_dir().join(format!("motet-live-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let base = root.join("call").to_string_lossy().into_owned();
        // 100 ms per character. Me pauses alone at 4.0-4.5 s; both pause at 6.0-6.5 s, so that
        // is the cut. After it, 11 s of speech with no shared pause.
        let mic = format!(
            "{}00000{}00000{}",
            "1".repeat(40),
            "1".repeat(15),
            "1".repeat(105)
        );
        let system = format!("{}00000{}", "1".repeat(60), "1".repeat(105));
        track(Path::new(&format!("{base}.mic.wav")), &mic);
        track(Path::new(&format!("{base}.system.wav")), &system);
        assert_eq!(preview_end(&base, 0, 2999).unwrap(), None);
        assert_eq!(preview_end(&base, 0, 5000).unwrap(), None);
        assert_eq!(preview_end(&base, 0, 13_000).unwrap(), Some(6200));
        // No pause: wait while under 10 s, then end near 10 s; the next slice starts there.
        assert_eq!(preview_end(&base, 6200, 13_000).unwrap(), None);
        assert_eq!(preview_end(&base, 6200, 17_000).unwrap(), Some(16_000));
        fs::remove_dir_all(root).unwrap();
    }

    /// Replays `MOTET_MEASURE_BASE`.{mic,system}.wav on a clock that runs at audio speed and
    /// waits for each real request, then prints the slices, caption delay and one line per
    /// caption: `<start ms> +<delay ms> <track>:<text>`. See docs/app/live-captions-runbook.md.
    #[test]
    #[ignore = "needs whisper-server, WHISPER_MODEL and MOTET_MEASURE_BASE"]
    fn measure_caption_delay() {
        let base = std::env::var("MOTET_MEASURE_BASE").unwrap();
        let total = ["mic", "system"]
            .map(|side| {
                let reader = hound::WavReader::open(format!("{base}.{side}.wav")).unwrap();
                u64::from(reader.duration()) * 1000 / u64::from(reader.spec().sample_rate)
            })
            .into_iter()
            .min()
            .unwrap();
        let stop = Stop::default();
        let directory = std::env::temp_dir().join(format!("motet-measure-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let mut server = crate::speech::Server::start(&stop).unwrap();
        let (mut clock, mut cursor, mut slices, mut busy) = (0_u64, 0_u64, 0, 0_u64);
        let mut delays = Vec::new();
        let mut lines = String::new();
        loop {
            let Some(end) = preview_end(&base, cursor, clock.min(total)).unwrap() else {
                // A tail without a pause is never cut while recording; Stop ends it.
                if clock >= total {
                    break;
                }
                clock += 250;
                continue;
            };
            let started = std::time::Instant::now();
            let rows =
                transcribe_slice(&mut server, &base, &directory, (cursor, end), 0, &stop).unwrap();
            let took = started.elapsed().as_millis() as u64;
            (slices, busy) = (slices + 1, busy + took);
            clock = clock.max(end) + took;
            for (start, _, side, text) in rows {
                delays.push(clock - start);
                lines.push_str(&format!("{start} +{} {side}:{text}\n", clock - start));
            }
            cursor = end;
        }
        fs::remove_dir_all(directory).unwrap();
        delays.sort_unstable();
        let at = |q: f64| delays[((delays.len() - 1) as f64 * q).round() as usize] as f64 / 1000.0;
        println!(
            "slices={slices} inference={:.1}s lines={} delay median={:.1}s p90={:.1}s max={:.1}s",
            busy as f64 / 1000.0,
            delays.len(),
            at(0.5),
            at(0.9),
            at(1.0),
        );
        print!("{lines}");
    }
}
