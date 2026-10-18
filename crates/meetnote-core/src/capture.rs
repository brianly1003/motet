//! Capture stage: system audio + mic from one ScreenCaptureKit stream, one WAV per track.

use crate::events::Stop;
use screencapturekit::prelude::*;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

type Wav = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

struct Track {
    path: String,
    wav: Wav,
    errors: mpsc::Sender<Result<(), String>>,
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
        let samples: Vec<f32> = first
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_ne_bytes(*b))
            .collect();
        // Non-interleaved: buffer 0 is channel 0. Interleaved: take every Nth sample.
        let stride = (samples.len() / frames).max(1);

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
                if flushed.elapsed() >= Duration::from_secs(5) {
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
