//! Capture stage: system audio + mic from one ScreenCaptureKit stream, one WAV per track.

use crate::events::Stop;
use crate::mics::Meters;
use screencapturekit::prelude::*;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

type Wav = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

struct Track {
    path: String,
    wav: Wav,
    errors: mpsc::Sender<Result<(), String>>,
    level: Option<Arc<AtomicU32>>,
    /// Mic only: clipped samples since the desktop last looked (`mics::too_loud`).
    clips: Option<Arc<AtomicU32>>,
}

impl SCStreamOutputTrait for Track {
    fn did_output_sample_buffer(&self, buf: CMSampleBuffer, _: SCStreamOutputType) {
        if let Err(error) = self.write_buffer(buf) {
            // The receiver may already be gone while capture is stopping.
            let _ = self.errors.send(Err(format!(
                "cannot save {}: {error}; check free disk space and folder permissions",
                self.path
            )));
        }
    }
}

impl Track {
    fn write_buffer(&self, buf: CMSampleBuffer) -> Result<(), Box<dyn Error>> {
        let frames = buf.num_samples() as usize;
        let Ok(list) = buf.audio_buffer_list() else {
            return Ok(());
        };
        let Some(first) = list.get(0) else {
            return Ok(());
        };
        if frames == 0 {
            return Ok(());
        }
        // Built-in mics deliver f32; a Bluetooth headset mic in call mode delivers 16-bit integers.
        let format = buf.format_description();
        let float = format.as_ref().is_none_or(|f| f.audio_is_float());
        let bits = format
            .as_ref()
            .and_then(|f| f.audio_bits_per_channel())
            .unwrap_or(32);
        let samples = to_f32(first.data(), float, bits)
            .ok_or_else(|| format!("unsupported audio format ({bits}-bit, float: {float})"))?;
        // Non-interleaved: buffer 0 is channel 0. Interleaved: take every Nth sample.
        let stride = (samples.len() / frames).max(1);

        if let Some(level) = &self.level {
            level.store(
                rms(samples.iter().step_by(stride).copied()).to_bits(),
                Ordering::Relaxed,
            );
        }
        if let Some(clips) = &self.clips {
            let n = samples
                .iter()
                .step_by(stride)
                .filter(|s| s.abs() >= crate::mics::CLIP)
                .count();
            clips.fetch_add(n as u32, Ordering::Relaxed);
        }
        let mut wav = self.wav.lock().map_err(|_| "audio writer lock poisoned")?;
        if wav.is_none() {
            let rate = buf
                .format_description()
                .and_then(|f| f.audio_sample_rate())
                .unwrap_or(48_000.0) as u32;
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            *wav = Some(hound::WavWriter::create(&self.path, spec)?);
        }
        if let Some(writer) = wav.as_mut() {
            write_samples(writer, samples.iter().step_by(stride).copied())?;
        }
        Ok(())
    }
}

fn to_f32(data: &[u8], float: bool, bits: u32) -> Option<Vec<f32>> {
    Some(match (float, bits) {
        (true, 32) => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_ne_bytes(*b))
            .collect(),
        (false, 16) => data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| f32::from(i16::from_ne_bytes(*b)) / 32768.0)
            .collect(),
        (false, 32) => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| i32::from_ne_bytes(*b) as f32 / 2_147_483_648.0)
            .collect(),
        _ => return None,
    })
}

fn rms(samples: impl Iterator<Item = f32>) -> f32 {
    let (sum, count) = samples
        .filter(|v| v.is_finite())
        .fold((0.0_f64, 0_u64), |(sum, count), v| {
            (sum + f64::from(v).powi(2), count + 1)
        });
    if count == 0 {
        0.0
    } else {
        ((sum / count as f64).sqrt() as f32).min(1.0)
    }
}

fn write_samples<W: Write + Seek>(
    writer: &mut hound::WavWriter<W>,
    samples: impl Iterator<Item = f32>,
) -> Result<(), hound::Error> {
    for sample in samples {
        writer.write_sample(sample)?;
    }
    Ok(())
}

fn wait_for_stop(
    stop: &Stop,
    errors: &mpsc::Receiver<Result<(), String>>,
    mut flush: impl FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let mut flushed = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        match errors.recv_timeout(Duration::from_millis(50)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if flushed.elapsed() >= Duration::from_secs(1) {
                    flush()?;
                    flushed = Instant::now();
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("capture error channel closed".into());
            }
        }
    }
    Ok(())
}

/// Records both tracks until `stop` is set or writing fails; returns tracks that got audio.
pub fn record(base: &str, stop: Stop) -> Result<Vec<String>, Box<dyn Error>> {
    record_with_levels(base, stop, None, None)
}

/// The desktop's recording: optional meters, and `mic` (a `mics::Mic` id) to record instead of
/// the system default input.
pub fn record_with_levels(
    base: &str,
    stop: Stop,
    meters: Option<Meters>,
    mic: Option<&str>,
) -> Result<Vec<String>, Box<dyn Error>> {
    let display = SCShareableContent::get()?
        .displays()
        .into_iter()
        .next()
        .ok_or("no display found")?;
    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build()?;
    // SCK needs a video filter; 2x2 frames at 1fps keeps it ~free.
    let config = SCStreamConfiguration::new()
        .with_width(2)
        .with_height(2)
        .with_minimum_frame_interval(&CMTime::new(1, 1))
        .with_captures_audio(true)
        .with_channel_count(1)
        .with_captures_microphone(true)?;
    let config = match mic {
        Some(id) => config.with_microphone_capture_device_id(id)?,
        None => config,
    };

    let mut stream = SCStream::new(&filter, &config)?;
    let mut tracks = vec![];
    let (errors, stopped) = mpsc::channel();
    for (suffix, kind) in [
        ("system", SCStreamOutputType::Audio),
        ("mic", SCStreamOutputType::Microphone),
    ] {
        let path = format!("{base}.{suffix}.wav");
        let wav: Wav = Arc::default();
        stream.add_output_handler(
            Track {
                path: path.clone(),
                wav: wav.clone(),
                errors: errors.clone(),
                level: meters
                    .as_ref()
                    .map(|m| m.levels[usize::from(suffix == "system")].clone()),
                clips: meters
                    .as_ref()
                    .filter(|_| suffix == "mic")
                    .map(|m| m.mic_clips.clone()),
            },
            kind,
        )?;
        tracks.push((path, wav));
    }

    println!("Recording system audio + mic. Press Enter to stop.");
    stream.start_capture()?;
    let mut result = wait_for_stop(&stop, &stopped, || {
        for (path, wav) in &tracks {
            let mut wav = wav
                .lock()
                .map_err(|_| "audio writer lock poisoned".to_string())?;
            if let Some(writer) = wav.as_mut() {
                writer
                    .flush()
                    .map_err(|e| format!("cannot flush {path}: {e}; check free disk space"))?;
            }
        }
        Ok(())
    });
    let stop_result = stream
        .stop_capture()
        .map_err(|e| format!("cannot stop capture: {e}"));
    // Readers load the last level; a stopped capture has none.
    for level in meters.iter().flat_map(|m| &m.levels) {
        level.store(0, Ordering::Relaxed);
    }
    result = result.and(stop_result);
    for pending in stopped.try_iter() {
        result = result.and(pending);
    }

    let mut files = vec![];
    for (path, wav) in tracks {
        let finalized =
            (|| -> Result<(), String> {
                let mut wav = wav
                    .lock()
                    .map_err(|_| "audio writer lock poisoned".to_string())?;
                if let Some(w) = wav.take() {
                    w.finalize().map_err(|e| format!(
                    "cannot finalize {path}: {e}; check free disk space and folder permissions"
                ))?;
                    files.push(path);
                }
                Ok(())
            })();
        // Attempt both tracks even if the first one failed; keep the first error.
        result = result.and(finalized);
    }
    result?;
    if files.is_empty() {
        return Err(
            "no audio captured - check Screen Recording / Microphone permission for your terminal"
                .into(),
        );
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor, SeekFrom};
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn meter_uses_real_signal_and_rejects_non_finite_samples() {
        assert_eq!(rms([0.0, 0.0].into_iter()), 0.0);
        assert_eq!(rms([0.5, -0.5].into_iter()), 0.5);
        assert_eq!(rms([f32::NAN, f32::INFINITY].into_iter()), 0.0);
        assert_eq!(rms([4.0].into_iter()), 1.0);
    }

    #[test]
    fn integer_mic_buffers_become_float_samples() {
        let int16: Vec<u8> = [16384_i16, -32768]
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        assert_eq!(to_f32(&int16, false, 16), Some(vec![0.5, -1.0]));
        assert_eq!(to_f32(&0.25_f32.to_ne_bytes(), true, 32), Some(vec![0.25]));
        assert_eq!(
            to_f32(&(i32::MIN).to_ne_bytes(), false, 32),
            Some(vec![-1.0])
        );
        assert_eq!(to_f32(&[0; 3], false, 24), None);
    }
    struct FailingWriter {
        bytes: Cursor<Vec<u8>>,
        fail: Arc<AtomicBool>,
    }

    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail.load(Ordering::Relaxed) {
                return Err(io::Error::other("disk full"));
            }
            self.bytes.write(bytes)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Seek for FailingWriter {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.bytes.seek(pos)
        }
    }

    #[test]
    fn stop_signal_and_write_error_end_the_wait() {
        let (sender, receiver) = mpsc::channel();
        let stop = Stop::default();
        sender.send(Err("disk full".into())).unwrap();
        assert_eq!(
            wait_for_stop(&stop, &receiver, || Ok(())),
            Err("disk full".into())
        );
        stop.store(true, Ordering::Relaxed);
        assert_eq!(wait_for_stop(&stop, &receiver, || Ok(())), Ok(()));
    }

    #[test]
    fn periodic_flush_failure_reaches_the_recorder() {
        let (_sender, receiver) = mpsc::channel();
        assert_eq!(
            wait_for_stop(&Stop::default(), &receiver, || Err("flush failed".into())),
            Err("flush failed".into())
        );
    }

    #[test]
    fn sample_write_failure_is_returned() {
        let fail = Arc::new(AtomicBool::new(false));
        let mut writer = hound::WavWriter::new(
            FailingWriter {
                bytes: Cursor::new(Vec::new()),
                fail: fail.clone(),
            },
            hound::WavSpec {
                channels: 1,
                sample_rate: 48000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        write_samples(&mut writer, [0.5].into_iter()).unwrap();
        fail.store(true, Ordering::Relaxed);
        assert!(
            write_samples(&mut writer, [0.5].into_iter())
                .unwrap_err()
                .to_string()
                .contains("disk full")
        );
    }
}
