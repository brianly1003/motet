use super::*;
#[derive(Clone)]
struct Fake {
    log: Arc<Mutex<Vec<String>>>,
    stops: Arc<Mutex<Vec<Stop>>>,
    allowed: Arc<AtomicBool>,
    announce_ok: bool,
}
impl Host for Fake {
    fn readiness(&self) -> Result<Readiness, String> {
        Ok(Readiness {
            microphone: self.allowed.load(Ordering::SeqCst),
            system_audio: true,
            notifications: false,
            speech_model: true,
            free_bytes: 10_000_000_000,
        })
    }
    fn event(&self, event: Event) -> Result<(), String> {
        self.log.lock().unwrap().push(match event {
            Event::StateChanged(s) => format!("state:{:?}", s.state),
            Event::NotesReady { .. } => "ready".into(),
            _ => "event".into(),
        });
        Ok(())
    }
    fn announce(&self) -> Result<(), String> {
        self.log.lock().unwrap().push("announce".into());
        if self.announce_ok {
            Ok(())
        } else {
            Err("no visible window".into())
        }
    }
    fn job(&self, job: Job) -> Result<(), String> {
        let value = match job {
            Job::Capture {
                stop, offset_ms, ..
            } => {
                self.stops.lock().unwrap().push(stop);
                format!("capture:{offset_ms}")
            }
            Job::Finish { .. } => "finish".into(),
            Job::Discard { .. } => "discard".into(),
            Job::Exit => "exit-job".into(),
        };
        self.log.lock().unwrap().push(value);
        Ok(())
    }
    fn notice(&self, _: &str) {}
    fn exit(&self) {
        self.log.lock().unwrap().push("exit".into());
    }
}
fn engine() -> Engine<Fake> {
    Engine::new(
        Fake {
            log: Arc::default(),
            stops: Arc::default(),
            allowed: Arc::new(AtomicBool::new(true)),
            announce_ok: true,
        },
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(AppState::default())),
    )
}
#[test]
fn publishes_and_announces_before_capture_and_rejects_overlap() {
    let mut e = engine();
    e.action(Action::Start, 0).unwrap();
    assert_eq!(
        &*e.host.log.lock().unwrap(),
        &["state:Recording", "announce", "capture:0"]
    );
    assert!(e.action(Action::Start, 10).is_err());
    assert!(e.busy.load(Ordering::SeqCst));
}
#[test]
fn resume_has_fresh_stop_and_excludes_pause_from_offset() {
    let mut e = engine();
    e.action(Action::Start, 0).unwrap();
    e.action(Action::Stop, 25_000).unwrap();
    assert!(e.host.stops.lock().unwrap()[0].load(Ordering::SeqCst));
    e.action(Action::Resume, 29_000).unwrap();
    let stops = e.host.stops.lock().unwrap();
    assert_eq!(stops.len(), 2);
    assert!(!stops[1].load(Ordering::SeqCst));
    assert!(!Arc::ptr_eq(&stops[0], &stops[1]));
    assert!(
        e.host
            .log
            .lock()
            .unwrap()
            .iter()
            .any(|s| s == "capture:25000")
    );
}
#[test]
fn deadline_wins_over_resume_and_processing_only_runs_once() {
    let mut e = engine();
    e.action(Action::Start, 0).unwrap();
    e.action(Action::Stop, 25_000).unwrap();
    e.action(Action::Resume, 35_000).unwrap();
    e.apply(Command::Tick, 36_000);
    assert!(matches!(
        e.session.app.state,
        SessionState::Finishing { .. }
    ));
    assert_eq!(
        e.host
            .log
            .lock()
            .unwrap()
            .iter()
            .filter(|s| *s == "finish")
            .count(),
        1
    );
}
#[test]
fn short_recording_discards_after_grace_and_releases_import_gate() {
    let mut e = engine();
    e.action(Action::Start, 0).unwrap();
    e.action(Action::Stop, 2_000).unwrap();
    e.apply(Command::Tick, 11_999);
    assert!(e.busy.load(Ordering::SeqCst));
    e.apply(Command::Tick, 12_000);
    assert!(!e.busy.load(Ordering::SeqCst));
    assert!(e.host.log.lock().unwrap().contains(&"discard".into()));
}
#[test]
fn blocked_permission_and_failed_announcement_never_capture() {
    let mut e = engine();
    e.host.allowed.store(false, Ordering::SeqCst);
    e.action(Action::Start, 0).unwrap();
    assert!(e.host.stops.lock().unwrap().is_empty());
    assert!(!e.busy.load(Ordering::SeqCst));
    e.host.allowed.store(true, Ordering::SeqCst);
    e.host.announce_ok = false;
    e.action(Action::Start, 1).unwrap();
    assert!(e.host.stops.lock().unwrap().is_empty());
    assert!(e.session.app.problem.is_some());
}
#[test]
fn quitting_waits_for_processing_and_failure_keeps_window_open() {
    let mut e = engine();
    e.action(Action::Start, 0).unwrap();
    let id = e.session.app.meeting_id.clone().unwrap();
    e.action(Action::RequestQuit, 100).unwrap();
    e.action(Action::Quit(false), 101).unwrap();
    assert!(!e.session.app.quit_pending);
    e.action(Action::RequestQuit, 102).unwrap();
    e.action(Action::Quit(true), 103).unwrap();
    assert!(e.host.stops.lock().unwrap()[0].load(Ordering::SeqCst));
    assert!(!e.host.log.lock().unwrap().contains(&"exit".into()));
    e.apply(
        Command::Finished {
            meeting_id: "stale".into(),
            title: "wrong".into(),
            action_items: vec![],
        },
        104,
    );
    assert!(matches!(
        e.session.app.state,
        SessionState::Finishing { .. }
    ));
    e.apply(
        Command::FinishFailed {
            meeting_id: id,
            reason: "disk full".into(),
        },
        105,
    );
    assert_eq!(e.session.app.state, SessionState::Idle);
    assert!(!e.session.app.quit_pending);
    assert!(!e.host.log.lock().unwrap().contains(&"exit".into()));
}
#[test]
fn successful_finish_emits_notes_before_quit() {
    let mut e = engine();
    e.action(Action::Start, 0).unwrap();
    let id = e.session.app.meeting_id.clone().unwrap();
    e.action(Action::RequestQuit, 100).unwrap();
    e.action(Action::Quit(true), 101).unwrap();
    e.apply(
        Command::Finished {
            meeting_id: id,
            title: "Meeting".into(),
            action_items: vec![],
        },
        102,
    );
    let log = e.host.log.lock().unwrap();
    assert!(
        log.iter().position(|s| s == "ready").unwrap()
            < log.iter().position(|s| s == "exit").unwrap()
    );
}

#[test]
fn import_reservation_prevents_recording_and_early_exit() {
    let mut e = engine();
    e.busy.store(true, Ordering::SeqCst);
    assert!(e.action(Action::Start, 0).is_err());
    assert!(e.action(Action::RequestQuit, 1).is_err());
    assert!(!e.host.log.lock().unwrap().contains(&"exit".into()));
}

#[test]
fn notification_stop_is_scoped_to_its_meeting_and_sizes_are_validated() {
    let mut e = engine();
    e.action(Action::Start, 0).unwrap();
    let id = e.session.app.meeting_id.clone().unwrap();
    assert!(e.session.app.dnd_active);
    e.action(Action::StopMeeting("older-meeting".into()), 25_000)
        .unwrap();
    assert_eq!(e.session.app.state, SessionState::Recording);
    assert!(!e.host.stops.lock().unwrap()[0].load(Ordering::SeqCst));
    assert!(
        e.action(Action::Size(CompanionSize::Sidecar), 25_000)
            .is_err()
    );
    e.action(Action::Size(CompanionSize::Dot), 25_000).unwrap();
    assert_eq!(e.shared.lock().unwrap().companion.size, CompanionSize::Dot);
    e.action(Action::StopMeeting(id), 26_000).unwrap();
    assert!(e.host.stops.lock().unwrap()[0].load(Ordering::SeqCst));
    assert_eq!(e.shared.lock().unwrap().companion.size, CompanionSize::Pill);
}
