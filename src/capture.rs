//! Capture stage: system audio + mic from one ScreenCaptureKit stream, one WAV per track.

use screencapturekit::prelude::*;
use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};

type Wav = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

struct Track {
    path: String,
    wav: Wav,
}

impl SCStreamOutputTrait for Track {
    fn did_output_sample_buffer(&self, buf: CMSampleBuffer, _: SCStreamOutputType) {
        let frames = buf.num_samples() as usize;
        let Ok(list) = buf.audio_buffer_list() else {
            return;
        };
        let Some(first) = list.get(0) else { return };
        if frames == 0 {
            return;
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

        let mut wav = self.wav.lock().unwrap();
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
            match hound::WavWriter::create(&self.path, spec) {
                Ok(w) => *wav = Some(w),
                Err(e) => return eprintln!("cannot create {}: {e}", self.path),
            }
        }
        let w = wav.as_mut().unwrap();
        for s in samples.iter().step_by(stride) {
            let _ = w.write_sample(*s);
        }
    }
}

/// Records "what you hear" + "what you say" into two WAVs; returns the ones that got audio.
pub(crate) fn record(base: &str) -> Result<Vec<String>, Box<dyn Error>> {
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
            },
            kind,
        )?;
        tracks.push((path, wav));
    }

    stream.start_capture()?;
    println!("Recording system audio + mic. Press Enter to stop.");
    std::io::stdin().read_line(&mut String::new())?;
    stream.stop_capture()?;

    let mut files = vec![];
    for (path, wav) in tracks {
        if let Some(w) = wav.lock().unwrap().take() {
            w.finalize()?;
            files.push(path);
        }
    }
    if files.is_empty() {
        return Err(
            "no audio captured - check Screen Recording / Microphone permission for your terminal"
                .into(),
        );
    }
    Ok(files)
}
