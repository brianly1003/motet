//! Live preview reads flushed audio on a background thread; capture never waits for the model.
use motet_core::events::Stop;
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
    {
        let state = app.state::<State>();
        let mut text = state
            .transcript
            .lock()
            .map_err(|_| "Live transcript is unavailable")?;
        if text.0 != id {
            *text = (id.clone(), String::new(), String::new());
        }
    }
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
                    let mut segments = Vec::new();
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
                    // Bound each preview to ten seconds even when the model is slower than the call.
                    let Some(end) = preview_end(cursor, duration) else {
                        return Ok(None);
                    };
                    for side in ["mic", "system"] {
                        let file = directory.join(format!("preview.{side}.wav"));
                        slice(Path::new(&format!("{base}.{side}.wav")), &file, cursor, end)?;
                        let decoded = directory.join(format!("preview.{side}.16k.wav"));
                        motet_core::decode_audio(&file, &decoded)?;
                        let samples = hound::WavReader::open(decoded)?
                            .samples::<f32>()
                            .collect::<Result<Vec<_>, _>>()?;
                        if samples.iter().any(|sample| sample.abs() > 0.005) {
                            let prompt = motet_core::config::terms();
                            for segment in server.transcribe(
                                &samples,
                                offset + cursor,
                                prompt.as_deref(),
                                &stop,
                            )? {
                                segments.push((
                                    segment.start_ms,
                                    segment.end_ms,
                                    side,
                                    segment.text,
                                ));
                            }
                        }
                    }
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
                            current.1 = text;
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
// Drain available slices immediately; poll only while waiting for more flushed audio.
fn preview_end(cursor: u64, duration: u64) -> Option<u64> {
    (duration.saturating_sub(cursor) >= 3000).then(|| duration.min(cursor.saturating_add(10_000)))
}

fn slice(
    source: &Path,
    target: &Path,
    start: u64,
    end: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = hound::WavReader::open(source)?;
    let spec = reader.spec();
    reader.seek((start * u64::from(spec.sample_rate) / 1000).try_into()?)?;
    let samples =
        ((end - start) * u64::from(spec.sample_rate) / 1000) as usize * usize::from(spec.channels);
    let mut writer = hound::WavWriter::create(target, spec)?;
    for sample in reader.samples::<f32>().take(samples) {
        writer.write_sample(sample?)?;
    }
    writer.finalize()?;
    Ok(())
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
    #[test]
    fn preview_catches_up_without_skipping_audio() {
        assert_eq!(preview_end(0, 2999), None);
        assert_eq!(preview_end(0, 3000), Some(3000));
        let mut cursor = 0;
        let mut ends = Vec::new();
        while let Some(end) = preview_end(cursor, 36_000) {
            ends.push(end);
            cursor = end;
        }
        assert_eq!(ends, vec![10_000, 20_000, 30_000, 36_000]);
        assert_eq!(preview_end(36_000, 35_000), None);
    }
    #[test]
    fn preview_slice_keeps_rate_and_exact_interval() {
        let root = std::env::temp_dir().join(format!("motet-live-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.wav");
        let target = root.join("slice.wav");
        let mut writer = hound::WavWriter::create(
            &source,
            hound::WavSpec {
                channels: 1,
                sample_rate: 16000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for _ in 0..32000 {
            writer.write_sample(0.25_f32).unwrap();
        }
        writer.finalize().unwrap();
        slice(&source, &target, 500, 1500).unwrap();
        let reader = hound::WavReader::open(target).unwrap();
        assert_eq!(reader.duration(), 16000);
        assert_eq!(reader.spec().sample_rate, 16000);
        fs::remove_dir_all(root).unwrap();
    }
}
