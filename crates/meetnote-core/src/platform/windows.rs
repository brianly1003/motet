//! WASAPI shared microphone and render-loopback capture on one COM thread.

use super::audio::{frame_at, packet};
use crate::events::Stop;
use std::{
    error::Error,
    fs::File,
    io::BufWriter,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use wasapi::{
    AudioClient, DeviceEnumerator, DeviceEventCallbacks, Direction, Role, SampleType, StreamMode,
    WaveFormat,
};
use windows::Win32::{
    Media::Audio::{
        AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR, IAudioCaptureClient,
    },
    System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        wasapi::deinitialize();
    }
}

struct Stream {
    capture: IAudioCaptureClient,
    client: AudioClient,
    channels: usize,
    rate: u32,
    started: bool,
}

impl Stream {
    fn open(
        enumerator: &DeviceEnumerator,
        direction: Direction,
        role: Role,
        rate: Option<u32>,
    ) -> Result<Self> {
        let device = enumerator.get_default_device_for_role(&direction, &role)?;
        let mut client = device.get_iaudioclient()?;
        let mix = client.get_mixformat()?;
        let rate = rate.unwrap_or(mix.get_samplespersec());
        let channels = mix.get_nchannels();
        if channels == 0 || rate == 0 {
            return Err("device has invalid audio format".into());
        }
        let format = WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            rate as usize,
            channels as usize,
            None,
        );
        client.initialize_client(
            &format,
            &Direction::Capture,
            &StreamMode::PollingShared {
                autoconvert: true,
                buffer_duration_hns: 1_000_000,
            },
        )?;
        // SAFETY: the client is initialized for capture and remains alive with the service.
        let capture = unsafe {
            client
                .as_iaudioclient()
                .GetService::<IAudioCaptureClient>()?
        };
        Ok(Self {
            capture,
            client,
            channels: channels as usize,
            rate,
            started: false,
        })
    }
    fn start(&mut self) -> Result<()> {
        self.client.start_stream()?;
        self.started = true;
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        if self.started {
            self.client.stop_stream()?;
            self.started = false;
        }
        Ok(())
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

struct Track {
    stream: Stream,
    writer: Option<hound::WavWriter<BufWriter<File>>>,
    written: u64,
    path: String,
    direction: Direction,
    role: Role,
    changed: Arc<AtomicBool>,
}

impl Track {
    fn open(
        enumerator: &DeviceEnumerator,
        base: &str,
        suffix: &str,
        direction: Direction,
        role: Role,
        changed: Arc<AtomicBool>,
    ) -> Result<Self> {
        let stream = Stream::open(enumerator, direction, role, None)?;
        let path = format!("{base}.{suffix}.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: stream.rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )?;
        writer.flush()?;
        Ok(Self {
            stream,
            writer: Some(writer),
            written: 0,
            path,
            direction,
            role,
            changed,
        })
    }
    fn reopen(&mut self, enumerator: &DeviceEnumerator) -> Result<()> {
        let rate = self.stream.rate;
        // An unplugged endpoint may reject Stop; still open its replacement.
        let _ = self.stream.stop();
        // Keep the WAV rate fixed if the new default device has a different mix rate.
        let mut stream = Stream::open(enumerator, self.direction, self.role, Some(rate))?;
        stream.start()?;
        self.stream = stream;
        Ok(())
    }
    fn drain(&mut self, epoch: u64, stop: &Stop) -> Result<()> {
        // SAFETY: capture is a valid COM interface owned and used on this thread.
        while !stop.load(Ordering::Relaxed)
            && unsafe { self.stream.capture.GetNextPacketSize()? } > 0
        {
            let mut data = std::ptr::null_mut();
            let mut frames = 0;
            let mut flags = 0;
            let mut timestamp = 0;
            // SAFETY: all output pointers refer to live locals; ReleaseBuffer below pairs this call.
            unsafe {
                self.stream.capture.GetBuffer(
                    &mut data,
                    &mut frames,
                    &mut flags,
                    None,
                    Some(&mut timestamp),
                )?;
            }
            let result = (|| -> Result<()> {
                if frames == 0 {
                    return Ok(());
                }
                if flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0 {
                    return Err("device returned an unreliable audio timestamp".into());
                }
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
                let size = (frames as usize)
                    .checked_mul(self.stream.channels)
                    .and_then(|v| v.checked_mul(4))
                    .ok_or("audio packet too large")?;
                let bytes = if silent {
                    &[][..]
                } else {
                    if data.is_null() {
                        return Err("device returned a null audio buffer".into());
                    }
                    // SAFETY: WASAPI owns frames of the negotiated interleaved f32 format until ReleaseBuffer.
                    unsafe { std::slice::from_raw_parts(data, size) }
                };
                if timestamp > clock()?.saturating_add(10_000_000) {
                    return Err("audio timestamp is ahead of the recording clock".into());
                }
                let start = frame_at(timestamp, epoch, self.stream.rate)?;
                let writer = self.writer.as_mut().ok_or("audio writer already closed")?;
                packet(
                    writer,
                    &mut self.written,
                    start,
                    frames as usize,
                    self.stream.channels,
                    bytes,
                    silent,
                )
            })();
            // SAFETY: release exactly the frames acquired above, even when writing failed.
            let released = unsafe { self.stream.capture.ReleaseBuffer(frames) }.map_err(Into::into);
            result.and(released)?;
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<()> {
        let stopped = self.stream.stop();
        let finalized = match self.writer.take() {
            Some(writer) => writer.finalize().map_err(Into::into),
            None => Ok(()),
        };
        stopped.and(finalized)
    }
}

fn clock() -> Result<u64> {
    let mut counter = 0;
    let mut frequency = 0;
    // SAFETY: both calls receive valid writable i64 pointers; no shared memory is exposed.
    unsafe {
        QueryPerformanceCounter(&mut counter)?;
        QueryPerformanceFrequency(&mut frequency)?;
    }
    if counter < 0 || frequency <= 0 {
        return Err("invalid Windows performance clock".into());
    }
    Ok((counter as u128 * 10_000_000 / frequency as u128).try_into()?)
}

/// Record the communications microphone and default output until Stop; keep all data local.
pub fn record(base: &str, stop: Stop) -> Result<Vec<String>> {
    wasapi::initialize_mta().ok()?;
    let _com = Com;
    let enumerator = DeviceEnumerator::new()?;
    let mic_changed = Arc::new(AtomicBool::new(false));
    let system_changed = Arc::new(AtomicBool::new(false));
    let mut callbacks = DeviceEventCallbacks::new();
    let mic = mic_changed.clone();
    let system = system_changed.clone();
    callbacks.set_default_device_callback(move |direction, role, _| {
        if direction == Direction::Capture && role == Role::Communications {
            mic.store(true, Ordering::Relaxed);
        }
        if direction == Direction::Render && role == Role::Console {
            system.store(true, Ordering::Relaxed);
        }
    });
    let _notifications = enumerator.register_notification_callback(callbacks)?;
    let mut tracks = Vec::new();
    let result = (|| -> Result<()> {
        tracks.push(Track::open(
            &enumerator,
            base,
            "mic",
            Direction::Capture,
            Role::Communications,
            mic_changed,
        )?);
        tracks.push(Track::open(
            &enumerator,
            base,
            "system",
            Direction::Render,
            Role::Console,
            system_changed,
        )?);
        if stop.load(Ordering::Relaxed) {
            return Err("recording stopped before capture started".into());
        }
        println!("Recording system audio + mic. Press Enter to stop.");
        let epoch = clock()?;
        for track in &mut tracks {
            track.stream.start()?;
        }
        let mut flushed = Instant::now();
        while !stop.load(Ordering::Relaxed) {
            for track in &mut tracks {
                if track.changed.swap(false, Ordering::Relaxed) {
                    track.reopen(&enumerator)?;
                }
                track.drain(epoch, &stop)?;
            }
            if flushed.elapsed() >= Duration::from_secs(5) {
                for track in &mut tracks {
                    if let Some(w) = &mut track.writer {
                        w.flush()?;
                    }
                }
                flushed = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let end = clock()?;
        let drain_stop = Stop::default();
        for track in &mut tracks {
            track.stream.stop()?;
            track.drain(epoch, &drain_stop)?;
            let end_frame = frame_at(end, epoch, track.stream.rate)?;
            if let Some(writer) = &mut track.writer {
                packet(writer, &mut track.written, end_frame, 0, 1, &[], true)?;
            }
        }
        Ok(())
    })();
    let mut result = result;
    for track in &mut tracks {
        result = result.and(track.finish());
    }
    result.map_err(|e| format!("Windows capture failed: {e}; check microphone privacy access, default audio devices, free disk space and folder permissions"))?;
    Ok(tracks.into_iter().map(|t| t.path).collect())
}
