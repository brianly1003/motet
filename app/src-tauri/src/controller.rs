//! Desktop session actor: one ordered command stream, independent of capture and model work.
use motet_core::{
    events::*,
    session::{self, Effect, Input, Session},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub enum Action {
    Start,
    Stop,
    StopMeeting(String),
    Pause,
    Resume,
    Size(CompanionSize),
    Pin(bool),
    Sharing(bool),
    RequestQuit,
    Quit(bool),
    Dismiss(u64),
}
pub enum Message {
    Mark(mpsc::Sender<Result<u64, String>>),
    Action(Action, mpsc::Sender<Result<(), String>>),
    CaptureFailed {
        id: String,
        reason: String,
    },
    Finished {
        id: String,
        result: Result<String, String>,
    },
}
pub enum Job {
    Exit,
    Capture {
        id: String,
        offset_ms: u64,
        stop: Stop,
    },
    Finish {
        id: String,
    },
    Discard {
        id: String,
    },
}

/// Native effects are injected so controller ordering can be tested without opening a microphone.
pub trait Host: Send + 'static {
    fn mark(&self, id: &str, at_ms: u64) -> Result<(), String>;
    fn readiness(&self) -> Result<Readiness, String>;
    fn event(&self, event: Event) -> Result<(), String>;
    fn announce(&self) -> Result<(), String>;
    fn job(&self, job: Job) -> Result<(), String>;
    fn notice(&self, message: &str);
    fn exit(&self);
}

pub struct Controller {
    origin: Instant,
    sender: mpsc::Sender<Message>,
    state: Arc<Mutex<AppState>>,
}
impl Controller {
    pub fn spawn(
        host: impl Host,
        sender: mpsc::Sender<Message>,
        receiver: mpsc::Receiver<Message>,
        busy: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let origin = Instant::now();
        let state = Arc::new(Mutex::new(AppState::default()));
        let shared = state.clone();
        std::thread::Builder::new()
            .name("motet-session".into())
            .spawn(move || {
                let mut engine = Engine::new(host, busy, shared);
                engine.clock = Some(origin);
                loop {
                    let message = receiver.recv_timeout(Duration::from_millis(100));
                    let now = origin.elapsed().as_millis() as u64;
                    // Advance deadlines even under a continuous stream of UI commands.
                    engine.apply(Command::Tick, now);
                    match message {
                        Ok(Message::Mark(reply)) => {
                            let _ = reply.send(engine.mark());
                        }
                        Ok(Message::Action(action, reply)) => {
                            let _ = reply.send(engine.action(action, now));
                        }
                        Ok(Message::CaptureFailed { id, reason })
                            if engine.session.app.meeting_id.as_ref() == Some(&id) =>
                        {
                            engine.capture_failed = true;
                            engine.apply(
                                Command::ProblemRaised(Problem::ToolFailed {
                                    tool: "Audio capture".into(),
                                    stderr: reason,
                                }),
                                now,
                            );
                        }
                        Ok(Message::Finished { id, result }) => engine.apply(
                            match result {
                                Ok(title) => Command::Finished {
                                    meeting_id: id,
                                    title,
                                    action_items: vec![],
                                },
                                Err(reason) => Command::FinishFailed {
                                    meeting_id: id,
                                    reason,
                                },
                            },
                            now,
                        ),
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            engine.stop();
                            break;
                        }
                        _ => {}
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            sender,
            state,
            origin,
        })
    }
    pub fn mark(&self) -> Result<u64, String> {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(Message::Mark(tx))
            .map_err(|_| "Session controller stopped")?;
        rx.recv()
            .map_err(|_| "Session controller stopped".to_string())?
    }
    pub fn resume_remaining_ms(&self) -> Result<u64, String> {
        Ok(match self.state()?.state {
            SessionState::Stopped { resume_until_ms } => {
                resume_until_ms.saturating_sub(self.origin.elapsed().as_millis() as u64)
            }
            _ => 0,
        })
    }
    pub fn state(&self) -> Result<AppState, String> {
        self.state
            .lock()
            .map(|s| s.clone())
            .map_err(|_| "Session state is unavailable".into())
    }
    pub fn action(&self, action: Action) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(Message::Action(action, tx))
            .map_err(|_| "Session controller stopped")?;
        rx.recv()
            .map_err(|_| "Session controller stopped".to_string())?
    }
}

struct Engine<H> {
    host: H,
    session: Session,
    busy: Arc<AtomicBool>,
    shared: Arc<Mutex<AppState>>,
    stop: Option<Stop>,
    owns_busy: bool,
    capture_failed: bool,
    exiting: bool,
    clock: Option<Instant>,
}
impl<H: Host> Engine<H> {
    fn new(host: H, busy: Arc<AtomicBool>, shared: Arc<Mutex<AppState>>) -> Self {
        let mut session = Session::new(Readiness::default());
        session.app.dnd_active = true; // Focus fallback: the companion always announces recording.
        session.app.auto_start = false; // Automatic detection belongs to M6.
        Self {
            host,
            session,
            busy,
            shared,
            stop: None,
            owns_busy: false,
            capture_failed: false,
            exiting: false,
            clock: None,
        }
    }
    fn mark(&self) -> Result<u64, String> {
        if self.session.app.state != SessionState::Recording || self.session.app.quit_pending {
            return Err("Mark a moment while recording".into());
        }
        let id = self
            .session
            .app
            .meeting_id
            .as_deref()
            .ok_or("No active meeting")?;
        let at_ms = self.session.app.elapsed_ms;
        self.host.mark(id, at_ms)?;
        Ok(at_ms)
    }
    fn action(&mut self, action: Action, now: u64) -> Result<(), String> {
        if self.exiting {
            return Err("Motet is shutting down".into());
        }
        let command = match action {
            Action::Start => {
                if self
                    .busy
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    return Err("A meeting is already active or being imported".into());
                }
                self.owns_busy = true;
                self.capture_failed = false;
                let readiness = match self.host.readiness() {
                    Ok(r) => r,
                    Err(e) => {
                        self.release();
                        return Err(e);
                    }
                };
                // Apply readiness without releasing the reservation before StartManual.
                let (session, effects) = session::next(
                    self.session.clone(),
                    Input {
                        now_ms: now,
                        since: timestamp(),
                        command: Command::ReadinessChanged(readiness),
                    },
                );
                self.session = session;
                self.effects(effects, now);
                Command::StartManual {
                    meeting_id: timestamp(),
                    notes_only: false,
                }
            }
            Action::Resume => {
                if self.capture_failed {
                    return Err(
                        "Capture failed. Finish this meeting before starting another recording."
                            .into(),
                    );
                }
                if !matches!(
                    self.session.app.state,
                    SessionState::Stopped { .. } | SessionState::Paused
                ) {
                    return Err("There is no paused or stopped recording to resume".into());
                }
                let readiness = self.host.readiness()?;
                self.apply(Command::ReadinessChanged(readiness), now);
                Command::ResumeRecording
            }
            Action::Size(size) => Command::SetCompanionSize(size),
            Action::Pin(pinned) => Command::SetCompanionPinned(pinned),
            Action::Sharing(active) => Command::SetSharing(active),
            Action::StopMeeting(id) => {
                if self.session.app.meeting_id.as_ref() != Some(&id) {
                    return Ok(());
                }
                Command::StopRecording
            }
            Action::Stop => Command::StopRecording,
            Action::Pause => {
                if self.session.app.state != SessionState::Recording {
                    return Err("There is no recording to pause".into());
                }
                Command::PauseRecording
            }
            Action::RequestQuit => {
                if self.busy.load(Ordering::SeqCst) && !self.owns_busy {
                    return Err(
                        "An import is running. Motet must stay open until it finishes.".into(),
                    );
                }
                Command::RequestQuit
            }
            Action::Quit(stop_and_save) => Command::Quit { stop_and_save },
            Action::Dismiss(id) => Command::DismissProblem { id },
        };
        self.apply(command, now);
        Ok(())
    }
    fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::SeqCst);
        }
    }
    fn release(&mut self) {
        if self.owns_busy && !self.exiting {
            self.busy.store(false, Ordering::SeqCst);
            self.owns_busy = false;
        }
    }
    fn apply(&mut self, command: Command, now: u64) {
        // A native permission check may have waited for a dialog; it cannot extend Resume.
        let now = self
            .clock
            .map_or(now, |origin| (origin.elapsed().as_millis() as u64).max(now));
        let (session, effects) = session::next(
            self.session.clone(),
            Input {
                now_ms: now,
                since: timestamp(),
                command,
            },
        );
        self.session = session;
        if let Ok(mut state) = self.shared.lock() {
            *state = self.session.app.clone();
        }
        self.effects(effects, now);
        if let Ok(mut state) = self.shared.lock() {
            *state = self.session.app.clone();
        }
        if self.session.app.state == SessionState::Idle {
            self.release();
        }
    }
    fn effects(&mut self, effects: Vec<Effect>, now: u64) {
        for effect in effects {
            let result = match effect {
                Effect::Emit(event) => self.host.event(event),
                Effect::NotifyRecording { .. } => self.host.announce(),
                Effect::StartCapture { meeting_id, .. } => {
                    let stop = Stop::default();
                    self.stop = Some(stop.clone());
                    self.host.job(Job::Capture {
                        id: meeting_id,
                        offset_ms: self.session.app.elapsed_ms,
                        stop,
                    })
                }
                Effect::StopCapture => {
                    self.stop();
                    Ok(())
                }
                Effect::Finish { meeting_id, .. } => self.host.job(Job::Finish { id: meeting_id }),
                Effect::Discard { meeting_id } => self.host.job(Job::Discard { id: meeting_id }),
                Effect::NotifyShort { seconds } => {
                    self.host.notice(&format!(
                        "Recording under {seconds} seconds discarded unless capture failed."
                    ));
                    Ok(())
                }
                Effect::NotifyStillPaused => {
                    self.host.notice(STILL_PAUSED);
                    Ok(())
                }
                Effect::NotifyProblem { .. } => {
                    self.host
                        .notice("Recording could not start. Check the problem shown in Motet.");
                    Ok(())
                }
                Effect::Exit => {
                    self.exiting = true;
                    self.busy.store(true, Ordering::SeqCst);
                    self.host.exit();
                    Ok(())
                }
                Effect::PauseDownload => Ok(()), // No downloader runs in this M4 shell.
                _ => Err(
                    "This session effect is not available in the manual recording controller"
                        .into(),
                ),
            };
            if let Err(reason) = result {
                self.stop();
                self.capture_failed = true;
                let command =
                    if let SessionState::Finishing { meeting_id } = &self.session.app.state {
                        Command::FinishFailed {
                            meeting_id: meeting_id.clone(),
                            reason,
                        }
                    } else {
                        Command::ProblemRaised(Problem::ToolFailed {
                            tool: "Desktop controller".into(),
                            stderr: reason,
                        })
                    };
                // Do not execute the rest of a failed start sequence (especially capture).
                let (session, effects) = session::next(
                    self.session.clone(),
                    Input {
                        now_ms: now,
                        since: timestamp(),
                        command,
                    },
                );
                self.session = session;
                for effect in effects {
                    if let Effect::Emit(event) = effect {
                        let _ = self.host.event(event);
                    }
                }
                break;
            }
        }
    }
}
pub(crate) const STILL_PAUSED: &str =
    "Still paused. Nothing has been recorded for 15 minutes. Resume or stop the recording.";
fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

#[cfg(test)]
mod tests;
