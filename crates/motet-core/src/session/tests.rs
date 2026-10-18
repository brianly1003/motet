use super::*;

fn ready() -> Readiness {
    Readiness {
        microphone: true,
        system_audio: true,
        notifications: true,
        speech_model: true,
        free_bytes: 10 * GB,
    }
}
fn fresh() -> Session {
    Session::new(ready())
}
fn step(s: Session, ms: u64, command: Command) -> (Session, Vec<Effect>) {
    next(
        s,
        Input {
            now_ms: ms,
            since: "2026-10-18T00:00:00Z".into(),
            command,
        },
    )
}
fn start(s: Session, notes_only: bool) -> Session {
    step(
        s,
        0,
        Command::StartManual {
            meeting_id: "m1".into(),
            notes_only,
        },
    )
    .0
}
fn call(mode: Mode) -> Call {
    Call {
        call_id: "c1".into(),
        app_id: "zoom".into(),
        app: "Zoom".into(),
        mode,
    }
}
fn detect(s: Session, mode: Mode) -> (Session, Vec<Effect>) {
    step(
        s,
        0,
        Command::CallDetected {
            call: call(mode),
            meeting_id: "m1".into(),
        },
    )
}
fn work(effects: Vec<Effect>) -> Vec<Effect> {
    effects
        .into_iter()
        .filter(|e| !matches!(e, Effect::Emit(Event::StateChanged(_))))
        .collect()
}
fn finished() -> Command {
    Command::Finished {
        meeting_id: "m1".into(),
        title: "Notes".into(),
        action_items: vec![],
    }
}

#[test]
fn manual_start_publishes_visible_state_and_notice_before_capture() {
    let (s, effects) = step(
        fresh(),
        0,
        Command::StartManual {
            meeting_id: "m1".into(),
            notes_only: false,
        },
    );
    assert_eq!(s.app.state, SessionState::Recording);
    assert!(s.app.companion.visible);
    assert!(matches!(&effects[0], Effect::Emit(Event::StateChanged(a)) if a.companion.visible));
    assert_eq!(
        work(effects),
        vec![
            Effect::NotifyRecording {
                reason: StartReason::Manual
            },
            Effect::StartCapture {
                meeting_id: "m1".into(),
                resume: false
            }
        ]
    );
}

#[test]
fn automatic_start_and_off_mode() {
    let (s, effects) = detect(fresh(), Mode::Auto);
    assert_eq!(
        s.app.started_by,
        Some(StartReason::App { app: "Zoom".into() })
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::NotifyRecording { .. }))
    );
    let (s, effects) = detect(fresh(), Mode::Off);
    assert_eq!(s.app.state, SessionState::Idle);
    assert!(effects.is_empty());
    let mut s = fresh();
    s.app.auto_start = false;
    assert!(detect(s, Mode::Auto).1.is_empty());
}

#[test]
fn ask_accept_decline_and_stale_answers() {
    let mut initial = fresh();
    initial.app.dnd_active = true;
    let (s, effects) = detect(initial, Mode::Ask);
    assert!(matches!(s.app.state, SessionState::Asking { .. }));
    assert!(s.app.companion.visible);
    assert!(effects.contains(&Effect::NotifyAsk {
        call: call(Mode::Ask)
    }));
    let (unchanged, effects) = step(
        s.clone(),
        1,
        Command::AnswerAsk {
            call_id: "old".into(),
            record: true,
        },
    );
    assert_eq!(unchanged.app, s.app);
    assert!(effects.is_empty());
    let (yes, _) = step(
        s.clone(),
        10,
        Command::AnswerAsk {
            call_id: "c1".into(),
            record: true,
        },
    );
    assert_eq!(yes.app.state, SessionState::Recording);
    let (no, effects) = step(
        s,
        10,
        Command::AnswerAsk {
            call_id: "c1".into(),
            record: false,
        },
    );
    assert_eq!(no.app.state, SessionState::Idle);
    assert!(work(effects).is_empty());
}

#[test]
fn ask_times_out_at_sixty_seconds_even_with_a_late_answer() {
    let (s, _) = detect(fresh(), Mode::Ask);
    assert!(matches!(
        step(s.clone(), 59_999, Command::Tick).0.app.state,
        SessionState::Asking { .. }
    ));
    let (s, effects) = step(
        s,
        60_000,
        Command::AnswerAsk {
            call_id: "c1".into(),
            record: true,
        },
    );
    assert_eq!(s.app.state, SessionState::Idle);
    assert!(work(effects).is_empty());
}

#[test]
fn stop_closes_capture_and_resume_preserves_meeting_and_active_duration() {
    let (s, effects) = step(start(fresh(), false), 20_000, Command::StopRecording);
    assert_eq!(
        s.app.state,
        SessionState::Stopped {
            resume_until_ms: 30_000
        }
    );
    assert_eq!(work(effects), vec![Effect::StopCapture]);
    let (s, effects) = step(s, 29_999, Command::ResumeRecording);
    assert_eq!(s.app.state, SessionState::Recording);
    assert_eq!(s.app.elapsed_ms, 20_000);
    assert_eq!(s.app.meeting_id.as_deref(), Some("m1"));
    assert!(effects.contains(&Effect::StartCapture {
        meeting_id: "m1".into(),
        resume: true
    }));
    assert_eq!(step(s, 30_999, Command::Tick).0.app.elapsed_ms, 21_000);
}

#[test]
fn grace_expiry_finishes_at_threshold_and_rejects_late_resume() {
    let (s, _) = step(start(fresh(), false), 20_000, Command::StopRecording);
    let (s, effects) = step(s, 30_000, Command::ResumeRecording);
    assert_eq!(
        s.app.state,
        SessionState::Finishing {
            meeting_id: "m1".into()
        }
    );
    assert_eq!(
        work(effects),
        vec![Effect::Finish {
            meeting_id: "m1".into(),
            notes_only: false
        }]
    );
}

#[test]
fn short_recording_is_kept_through_grace_then_discarded_once() {
    let (s, _) = step(start(fresh(), false), 19_999, Command::StopRecording);
    assert!(work(step(s.clone(), 29_998, Command::Tick).1).is_empty());
    let (s, effects) = step(s, 29_999, Command::Tick);
    assert_eq!(s.app.state, SessionState::Idle);
    assert_eq!(
        work(effects),
        vec![
            Effect::Discard {
                meeting_id: "m1".into()
            },
            Effect::NotifyShort { seconds: 20 }
        ]
    );
    assert!(step(s, 30_000, Command::Tick).1.is_empty());
}

#[test]
fn zero_discard_setting_keeps_even_zero_length_recordings() {
    let mut s = fresh();
    s.discard_short_s = 0;
    let (s, _) = step(start(s, false), 0, Command::StopRecording);
    assert!(matches!(
        step(s, 10_000, Command::Tick).0.app.state,
        SessionState::Finishing { .. }
    ));
}

#[test]
fn idle_mic_timeout_resets_on_activity_and_ignores_repeat_idle_signals() {
    let s = start(fresh(), false);
    let s = step(s, 1_000, Command::MicActivity { active: false }).0;
    let s = step(s, 20_000, Command::MicActivity { active: false }).0;
    let s = step(s, 30_000, Command::MicActivity { active: true }).0;
    assert_eq!(
        step(s.clone(), 31_000, Command::Tick).0.app.state,
        SessionState::Recording
    );
    let s = step(s, 40_000, Command::MicActivity { active: false }).0;
    let (s, effects) = step(s, 70_000, Command::Tick);
    assert!(matches!(s.app.state, SessionState::Stopped { .. }));
    assert!(effects.contains(&Effect::StopCapture));
}

#[test]
fn app_quit_stops_only_the_matching_call() {
    let (s, _) = detect(fresh(), Mode::Auto);
    assert_eq!(
        step(
            s.clone(),
            1,
            Command::AppQuit {
                app_id: "other".into()
            }
        )
        .0
        .app
        .state,
        SessionState::Recording
    );
    assert!(matches!(
        step(
            s,
            2,
            Command::AppQuit {
                app_id: "zoom".into()
            }
        )
        .0
        .app
        .state,
        SessionState::Stopped { .. }
    ));
}

#[test]
fn never_discards_without_the_short_notification_and_late_undo_is_ignored() {
    let (s, _) = detect(fresh(), Mode::Auto);
    let (s, effects) = step(
        s,
        25_000,
        Command::NeverForApp {
            call_id: "c1".into(),
        },
    );
    assert!(s.app.never_pending.is_some());
    assert!(effects.contains(&Effect::SetAppMode {
        app_id: "zoom".into(),
        mode: Mode::Off
    }));
    assert!(work(step(s.clone(), 25_001, Command::ResumeRecording).1).is_empty());
    let (s, effects) = step(
        s,
        35_000,
        Command::UndoNever {
            app_id: "zoom".into(),
        },
    );
    assert_eq!(s.app.state, SessionState::Idle);
    assert_eq!(
        work(effects),
        vec![Effect::Discard {
            meeting_id: "m1".into()
        }]
    );
}

#[test]
fn never_undo_restores_previous_mode_and_resumes_same_meeting() {
    let (s, _) = detect(fresh(), Mode::Ask);
    let s = step(
        s,
        1,
        Command::AnswerAsk {
            call_id: "c1".into(),
            record: true,
        },
    )
    .0;
    let s = step(
        s,
        25_000,
        Command::NeverForApp {
            call_id: "c1".into(),
        },
    )
    .0;
    let (s, effects) = step(
        s,
        30_000,
        Command::UndoNever {
            app_id: "zoom".into(),
        },
    );
    assert_eq!(s.app.state, SessionState::Recording);
    assert_eq!(s.app.meeting_id.as_deref(), Some("m1"));
    assert!(s.app.never_pending.is_none());
    assert!(effects.contains(&Effect::SetAppMode {
        app_id: "zoom".into(),
        mode: Mode::Ask
    }));
    assert!(effects.contains(&Effect::StartCapture {
        meeting_id: "m1".into(),
        resume: true
    }));
}

#[test]
fn notes_only_needs_no_audio_permissions_ignores_idle_and_skips_grace() {
    let s = Session::new(Readiness {
        free_bytes: 10 * GB,
        ..Readiness::default()
    });
    let (s, effects) = step(
        s,
        0,
        Command::StartManual {
            meeting_id: "m1".into(),
            notes_only: true,
        },
    );
    assert_eq!(s.app.started_by, Some(StartReason::NotesOnly));
    assert!(work(effects).is_empty());
    let s = step(s, 1, Command::MicActivity { active: false }).0;
    assert_eq!(
        step(s.clone(), 40_000, Command::Tick).0.app.state,
        SessionState::Recording
    );
    let (s, effects) = step(s, 2, Command::StopRecording);
    assert!(matches!(s.app.state, SessionState::Finishing { .. }));
    assert_eq!(
        work(effects),
        vec![Effect::Finish {
            meeting_id: "m1".into(),
            notes_only: true
        }]
    );
}

#[test]
fn quit_question_cancel_and_stop_and_save_bypass_short_discard() {
    for stopped in [false, true] {
        let mut s = start(fresh(), false);
        if stopped {
            s = step(s, 100, Command::StopRecording).0;
        }
        let (s, effects) = step(s, 200, Command::RequestQuit);
        assert!(s.app.quit_pending);
        assert!(work(effects).is_empty());
        assert!(
            !step(
                s.clone(),
                300,
                Command::Quit {
                    stop_and_save: false
                }
            )
            .0
            .app
            .quit_pending
        );
        let (s, effects) = step(
            s,
            300,
            Command::Quit {
                stop_and_save: true,
            },
        );
        assert!(matches!(s.app.state, SessionState::Finishing { .. }));
        assert_eq!(effects.contains(&Effect::StopCapture), !stopped);
        assert!(effects.contains(&Effect::Finish {
            meeting_id: "m1".into(),
            notes_only: false
        }));
        let (s, effects) = step(s, 400, finished());
        assert_eq!(s.app.state, SessionState::Idle);
        assert!(effects.contains(&Effect::Exit));
    }
}

#[test]
fn quit_in_finishing_waits_or_marks_recovery_on_quit_now() {
    let s = start(fresh(), true);
    let s = step(s, 1, Command::StopRecording).0;
    let (s, effects) = step(s, 2, Command::RequestQuit);
    assert!(s.app.quit_pending && s.app.companion.visible);
    assert!(!effects.contains(&Effect::Exit));
    let (_, effects) = step(s, 3, Command::QuitNow);
    assert_eq!(
        work(effects),
        vec![
            Effect::MarkRecoverable {
                meeting_id: "m1".into()
            },
            Effect::PauseDownload,
            Effect::Exit
        ]
    );
    assert_eq!(
        work(step(fresh(), 0, Command::RequestQuit).1),
        vec![Effect::PauseDownload, Effect::Exit]
    );
}

#[test]
fn completion_and_failure_return_idle_and_ignore_stale_workers() {
    let s = step(start(fresh(), true), 1, Command::StopRecording).0;
    let (unchanged, effects) = step(
        s.clone(),
        2,
        Command::FinishFailed {
            meeting_id: "old".into(),
            reason: "bad".into(),
        },
    );
    assert_eq!(unchanged.app, s.app);
    assert!(effects.is_empty());
    let (done, effects) = step(s.clone(), 2, finished());
    assert_eq!(done.app.state, SessionState::Idle);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Emit(Event::NotesReady { .. })))
    );
    let (failed, effects) = step(
        s,
        2,
        Command::FinishFailed {
            meeting_id: "m1".into(),
            reason: "disk error".into(),
        },
    );
    assert_eq!(failed.app.state, SessionState::Idle);
    assert!(matches!(
        failed.app.problem.unwrap().problem,
        Problem::NotesFailed { .. }
    ));
    assert!(!effects.contains(&Effect::Exit));
}

#[test]
fn extra_calls_are_added_once_without_starting_another_recording() {
    let (s, _) = detect(fresh(), Mode::Auto);
    let mut other = call(Mode::Auto);
    other.app_id = "teams".into();
    let command = Command::CallDetected {
        call: other,
        meeting_id: "m2".into(),
    };
    let (s, effects) = step(s, 1, command.clone());
    assert_eq!(s.app.meeting_id.as_deref(), Some("m1"));
    assert_eq!(
        work(effects),
        vec![Effect::AddApp {
            meeting_id: "m1".into(),
            app_id: "teams".into()
        }]
    );
    assert!(work(step(s, 2, command).1).is_empty());
}

#[test]
fn missing_permissions_models_and_disk_block_start_with_prioritized_problems() {
    let (s, effects) = detect(Session::new(Readiness::default()), Mode::Auto);
    assert_eq!(s.app.state, SessionState::Idle);
    assert_eq!(s.app.problem.unwrap().problem, Problem::DiskFull);
    assert_eq!(s.app.more_problems, 4);
    assert!(!s.app.auto_start);
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::StartCapture { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::NotifyProblem { .. }))
    );
}

#[test]
fn disk_boundaries_warn_then_clear_and_stop_on_full() {
    let mut checks = ready();
    checks.free_bytes = GB;
    let s = start(Session::new(checks), false);
    assert_eq!(s.app.state, SessionState::Recording);
    assert!(matches!(
        s.app.problem.as_ref().unwrap().problem,
        Problem::DiskLow { .. }
    ));
    let s = step(s, 1, Command::ReadinessChanged(ready())).0;
    assert!(s.app.problem.is_none());
    let mut checks = ready();
    checks.free_bytes = GB - 1;
    let (s, effects) = step(s, 2, Command::ReadinessChanged(checks));
    assert!(matches!(s.app.state, SessionState::Stopped { .. }));
    assert_eq!(s.app.problem.unwrap().problem, Problem::DiskFull);
    assert!(effects.contains(&Effect::StopCapture));
}

#[test]
fn problems_are_deduplicated_prioritized_and_clear_without_changing_session() {
    let mut s = start(fresh(), true);
    for problem in [
        Problem::Slow,
        Problem::DiskLow { free_bytes: 2 * GB },
        Problem::NotesFailed {
            meeting_id: "old".into(),
            reason: "failed".into(),
        },
        Problem::ToolFailed {
            tool: "notes".into(),
            stderr: "bad".into(),
        },
        Problem::ModelMissing {
            kind: ModelKind::Notes,
        },
        Problem::PermissionMissing {
            kind: PermissionKind::Notifications,
            side: Side::TheCall,
        },
        Problem::DiskFull,
        Problem::DiskFull,
    ] {
        s = step(s, 0, Command::ProblemRaised(problem)).0;
    }
    assert_eq!(s.app.state, SessionState::Recording);
    assert_eq!(s.app.more_problems, 6);
    for (rank, command_kind) in (0..7).zip([0, 1, 2, 0, 1, 2, 0]) {
        let problem = s.app.problem.clone().unwrap();
        assert_eq!(priority(&problem.problem), rank);
        let command = match command_kind {
            0 => Command::DismissProblem { id: problem.id },
            1 => Command::ProblemResolved { id: problem.id },
            _ => Command::RetryProblem { id: problem.id },
        };
        let (next_s, effects) = step(s, 0, command);
        if command_kind == 2 {
            assert!(effects.contains(&Effect::Retry { problem }));
        }
        s = next_s;
    }
    assert!(s.app.problem.is_none());
    assert_eq!(s.app.more_problems, 0);
}

#[test]
fn readiness_resolution_clears_blockers_but_does_not_reenable_auto_start() {
    let (s, _) = detect(Session::new(Readiness::default()), Mode::Auto);
    let s = step(s, 1, Command::ReadinessChanged(ready())).0;
    assert!(s.app.problem.is_none());
    assert!(!s.app.auto_start);
}

#[test]
fn interrupted_audio_keeps_meeting_and_inserts_pause_marker() {
    let (s, effects) = step(start(fresh(), false), 1_000, Command::AudioInterrupted);
    assert_eq!(s.app.state, SessionState::Recording);
    assert_eq!(
        work(effects),
        vec![
            Effect::StopCapture,
            Effect::AudioPaused {
                meeting_id: "m1".into(),
                at_ms: 1000
            },
            Effect::StartCapture {
                meeting_id: "m1".into(),
                resume: true
            }
        ]
    );
}

#[test]
fn recoverable_meetings_are_offered_without_starting_capture() {
    let (s, effects) = step(
        fresh(),
        0,
        Command::Recoverable {
            meeting_ids: vec!["m1".into()],
        },
    );
    assert_eq!(s.app.state, SessionState::Idle);
    assert_eq!(
        effects,
        vec![Effect::OfferRecovery {
            meeting_ids: vec!["m1".into()]
        }]
    );
}

#[test]
fn elapsed_only_updates_are_throttled_and_backwards_time_cannot_underflow() {
    let s = start(fresh(), false);
    let (s, effects) = step(s, 999, Command::Tick);
    assert!(effects.is_empty());
    let (s, effects) = step(s, 1000, Command::Tick);
    assert!(matches!(
        &effects[..],
        [Effect::Emit(Event::StateChanged(_))]
    ));
    let (s, effects) = step(s, 0, Command::Tick);
    assert_eq!(s.app.elapsed_ms, 1000);
    assert!(effects.is_empty());
}

#[test]
fn invalid_commands_do_not_start_or_stop_work() {
    for command in [
        Command::StopRecording,
        Command::ResumeRecording,
        Command::QuitNow,
        Command::UndoNever {
            app_id: "zoom".into(),
        },
        Command::NeverForApp {
            call_id: "old".into(),
        },
        Command::Quit {
            stop_and_save: true,
        },
        finished(),
    ] {
        let (s, effects) = step(fresh(), 0, command);
        assert_eq!(s.app.state, SessionState::Idle);
        assert!(effects.is_empty());
    }
}

#[test]
fn raised_disk_and_permission_failures_cannot_be_bypassed_by_dismiss_then_resume() {
    for problem in [
        Problem::DiskFull,
        Problem::PermissionMissing {
            kind: PermissionKind::Microphone,
            side: Side::YourMicrophone,
        },
    ] {
        let s = start(fresh(), false);
        let s = step(s, 100, Command::ProblemRaised(problem)).0;
        let id = s.app.problem.as_ref().unwrap().id;
        let s = step(s, 101, Command::DismissProblem { id }).0;
        let (s, effects) = step(s, 102, Command::ResumeRecording);
        assert!(matches!(s.app.state, SessionState::Stopped { .. }));
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::StartCapture { .. }))
        );
        let s = step(s, 103, Command::ReadinessChanged(ready())).0;
        assert_eq!(
            step(s, 104, Command::ResumeRecording).0.app.state,
            SessionState::Recording
        );
    }
}

#[test]
fn readiness_revocation_stops_audio_and_low_disk_warns_during_capture() {
    let mut checks = ready();
    checks.free_bytes = 4 * GB;
    let s = step(
        start(fresh(), false),
        100,
        Command::ReadinessChanged(checks.clone()),
    )
    .0;
    assert!(matches!(
        s.app.problem.unwrap().problem,
        Problem::DiskLow { .. }
    ));
    checks.microphone = false;
    let (s, effects) = step(
        start(fresh(), false),
        100,
        Command::ReadinessChanged(checks),
    );
    assert!(matches!(s.app.state, SessionState::Stopped { .. }));
    assert!(effects.contains(&Effect::StopCapture));
}

#[test]
fn cancelling_quit_during_finishing_hides_companion_again() {
    let s = step(start(fresh(), true), 1, Command::StopRecording).0;
    let s = step(s, 2, Command::RequestQuit).0;
    let s = step(
        s,
        3,
        Command::Quit {
            stop_and_save: false,
        },
    )
    .0;
    assert!(!s.app.quit_pending && !s.app.companion.visible);
}

#[test]
fn quit_pending_short_expiry_discards_and_exits() {
    let s = step(start(fresh(), false), 1, Command::StopRecording).0;
    let s = step(s, 2, Command::RequestQuit).0;
    let (s, effects) = step(s, 10_001, Command::Tick);
    assert_eq!(s.app.state, SessionState::Idle);
    assert!(effects.contains(&Effect::Discard {
        meeting_id: "m1".into()
    }));
    assert!(effects.contains(&Effect::Exit));
}

#[test]
fn finish_failure_cancels_pending_quit_and_clears_slow_warning() {
    let s = step(
        start(fresh(), true),
        0,
        Command::ProblemRaised(Problem::Slow),
    )
    .0;
    let s = step(s, 1, Command::StopRecording).0;
    let s = step(s, 2, Command::RequestQuit).0;
    let (s, effects) = step(
        s,
        3,
        Command::FinishFailed {
            meeting_id: "m1".into(),
            reason: "failed".into(),
        },
    );
    assert!(!s.app.quit_pending);
    assert_eq!(s.app.more_problems, 0);
    assert!(matches!(
        s.app.problem.unwrap().problem,
        Problem::NotesFailed { .. }
    ));
    assert!(!effects.contains(&Effect::Exit));
}

#[test]
fn expiry_that_exits_cannot_start_another_recording() {
    let s = step(start(fresh(), false), 1, Command::StopRecording).0;
    let s = step(s, 2, Command::RequestQuit).0;
    let (s, effects) = step(
        s,
        10_001,
        Command::StartManual {
            meeting_id: "m2".into(),
            notes_only: false,
        },
    );
    assert_eq!(s.app.state, SessionState::Idle);
    assert!(effects.contains(&Effect::Exit));
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::StartCapture { .. }))
    );
}

#[test]
fn companion_sizes_follow_recording_stop_quit_and_deadline() {
    let s = start(fresh(), false);
    let (s, effects) = step(s, 25_000, Command::SetCompanionSize(CompanionSize::Dot));
    assert_eq!(s.app.companion.size, CompanionSize::Dot);
    assert!(s.app.companion.visible);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Emit(Event::StateChanged(_))))
    );
    let (s, _) = step(s, 26_000, Command::StopRecording);
    assert_eq!(s.app.companion.size, CompanionSize::Pill);
    let (s, _) = step(s, 27_000, Command::SetCompanionSize(CompanionSize::Dot));
    assert_eq!(s.app.companion.size, CompanionSize::Pill);
    let (s, _) = step(s, 36_000, Command::Tick);
    assert!(!s.app.companion.visible);
    let (s, _) = step(s, 36_001, Command::RequestQuit);
    assert!(s.app.companion.visible);
    assert_eq!(s.app.companion.size, CompanionSize::Pill);

    let s = start(fresh(), false);
    let (s, _) = step(s, 1, Command::SetCompanionSize(CompanionSize::Sidecar));
    assert_eq!(s.app.companion.size, CompanionSize::Pill);
    let (s, _) = step(s, 2, Command::SetCompanionSize(CompanionSize::Dot));
    let (s, _) = step(s, 3, Command::RequestQuit);
    assert_eq!(s.app.companion.size, CompanionSize::Pill);
}
