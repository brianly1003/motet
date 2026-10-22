//! CLI stop source: Enter requests cancellation without putting stdin in the core.

use motetto_core::{capture, events::Stop};
use std::error::Error;
use std::sync::{atomic::Ordering, mpsc};

/// Preserve Enter-to-stop behavior while the core accepts cancellation from any caller.
pub(crate) fn record(base: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let stop = Stop::default();
    let input_stop = stop.clone();
    let (sender, receiver) = mpsc::channel();
    // The CLI exits on capture failure, so an outstanding stdin reader cannot outlive it.
    std::thread::spawn(move || {
        let result = std::io::stdin()
            .read_line(&mut String::new())
            .map(|_| ())
            .map_err(|e| format!("cannot read the stop key: {e}"));
        // Capture may have already failed and the receiver may be gone.
        let _ = sender.send(result);
        input_stop.store(true, Ordering::Relaxed);
    });
    let files = capture::record(base, stop)?;
    if let Ok(result) = receiver.try_recv() {
        result?;
    }
    Ok(files)
}
