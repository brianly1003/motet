//! Pure desktop recording rules. Time, readiness and worker outcomes are explicit inputs.

use crate::events::*;

const GRACE_MS: u64 = 10_000;
const ASK_MS: u64 = 60_000;
const GB: u64 = 1_000_000_000;

/// No clock or I/O is read by the reducer. `since` is a caller-supplied wall-clock timestamp.
#[derive(Debug, Clone)]
pub struct Input {
    pub now_ms: u64,
    pub since: String,
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Emit(Event),
    NotifyRecording {
        reason: StartReason,
    },
    NotifyAsk {
        call: Call,
    },
    NotifyProblem {
        id: u64,
    },
    NotifyShort {
        seconds: u32,
    },
    StartCapture {
        meeting_id: String,
        resume: bool,
    },
    StopCapture,
    Finish {
        meeting_id: String,
        notes_only: bool,
    },
    Discard {
        meeting_id: String,
    },
    AddApp {
        meeting_id: String,
        app_id: String,
    },
    SetAppMode {
        app_id: String,
        mode: Mode,
    },
    AudioPaused {
        meeting_id: String,
        at_ms: u64,
    },
    Retry {
        problem: ProblemInfo,
    },
    OfferRecovery {
        meeting_ids: Vec<String>,
    },
    MarkRecoverable {
        meeting_id: String,
    },
    PauseDownload,
    Exit,
}

#[derive(Debug, Clone)]
struct PendingAsk {
    call: Call,
    meeting_id: String,
    until_ms: u64,
}

/// Reducer state, including private timers and the full problem queue behind `AppState`.
#[derive(Debug, Clone)]
pub struct Session {
    pub app: AppState,
    pub discard_short_s: u32,
    pub idle_stop_s: u32,
    readiness: Readiness,
    ask: Option<PendingAsk>,
    call: Option<Call>,
    apps: Vec<String>,
    problems: Vec<ProblemInfo>,
    next_problem_id: u64,
    last_ms: u64,
    last_emit_ms: u64,
    idle_since: Option<u64>,
}

impl Session {
    pub fn new(readiness: Readiness) -> Self {
        Self {
            app: AppState::default(),
            discard_short_s: 20,
            idle_stop_s: 30,
            readiness,
            ask: None,
            call: None,
            apps: Vec::new(),
            problems: Vec::new(),
            next_problem_id: 1,
            last_ms: 0,
            last_emit_ms: 0,
            idle_since: None,
        }
    }

    fn audio(&self) -> bool {
        self.app.started_by != Some(StartReason::NotesOnly)
    }

    fn idle(&mut self) {
        self.app.state = SessionState::Idle;
        self.app.meeting_id = None;
        self.app.app = None;
        self.app.elapsed_ms = 0;
        self.app.started_by = None;
        self.app.never_pending = None;
        self.ask = None;
        self.call = None;
        self.apps.clear();
        self.idle_since = None;
    }

    fn problem(&mut self, problem: Problem, since: &str) -> u64 {
        if let Some(existing) = self
            .problems
            .iter_mut()
            .find(|p| same_cause(&p.problem, &problem))
        {
            existing.problem = problem;
            return existing.id;
        }
        let id = self.next_problem_id;
        self.next_problem_id = self.next_problem_id.saturating_add(1);
        self.problems.push(ProblemInfo {
            id,
            problem,
            since: since.into(),
        });
        id
    }

    fn can_start(
        &mut self,
        notes_only: bool,
        automatic: bool,
        since: &str,
        effects: &mut Vec<Effect>,
    ) -> bool {
        let mut blocked = Vec::new();
        if self.readiness.free_bytes < GB {
            blocked.push(Problem::DiskFull);
        }
        if !notes_only {
            for (allowed, kind, side) in [
                (
                    self.readiness.microphone,
                    PermissionKind::Microphone,
                    Side::YourMicrophone,
                ),
                (
                    self.readiness.system_audio,
                    PermissionKind::SystemAudio,
                    Side::TheCall,
                ),
            ] {
                if !allowed {
                    blocked.push(Problem::PermissionMissing { kind, side });
                }
            }
            if !self.readiness.speech_model {
                blocked.push(Problem::ModelMissing {
                    kind: ModelKind::Speech,
                });
            }
            if automatic && !self.readiness.notifications {
                self.app.auto_start = false;
                blocked.push(Problem::PermissionMissing {
                    kind: PermissionKind::Notifications,
                    side: Side::TheCall,
                });
            }
        }
        if !blocked.is_empty() {
            for problem in blocked {
                let id = self.problem(problem, since);
                effects.push(Effect::NotifyProblem { id });
            }
            return false;
        }
        if self.readiness.free_bytes < 5 * GB {
            self.problem(
                Problem::DiskLow {
                    free_bytes: self.readiness.free_bytes,
                },
                since,
            );
        }
        true
    }

    fn start(&mut self, meeting_id: String, reason: StartReason, effects: &mut Vec<Effect>) {
        self.app.state = SessionState::Recording;
        self.app.meeting_id = Some(meeting_id.clone());
        self.app.elapsed_ms = 0;
        self.app.app = self.call.as_ref().map(|c| c.app.clone());
        self.app.started_by = Some(reason.clone());
        self.app.companion.size = CompanionSize::Pill;
        self.idle_since = None;
        if self.audio() {
            effects.push(Effect::NotifyRecording { reason });
            effects.push(Effect::StartCapture {
                meeting_id,
                resume: false,
            });
        }
    }

    fn finish(&mut self, effects: &mut Vec<Effect>) {
        if let Some(meeting_id) = self.app.meeting_id.clone() {
            self.app.state = SessionState::Finishing {
                meeting_id: meeting_id.clone(),
            };
            self.app.never_pending = None;
            effects.push(Effect::Finish {
                meeting_id,
                notes_only: !self.audio(),
            });
        }
    }

    fn stop(&mut self, now: u64, effects: &mut Vec<Effect>) {
        if self.app.state != SessionState::Recording {
            return;
        }
        self.idle_since = None;
        if self.audio() {
            effects.push(Effect::StopCapture);
            self.app.state = SessionState::Stopped {
                resume_until_ms: now.saturating_add(GRACE_MS),
            };
        } else {
            self.finish(effects);
        }
    }

    fn resume(&mut self, since: &str, effects: &mut Vec<Effect>) {
        if self.can_start(false, false, since, effects) {
            self.app.state = SessionState::Recording;
            self.idle_since = None;
            if let (Some(meeting_id), Some(reason)) =
                (self.app.meeting_id.clone(), self.app.started_by.clone())
            {
                effects.push(Effect::NotifyRecording { reason });
                effects.push(Effect::StartCapture {
                    meeting_id,
                    resume: true,
                });
            }
        }
    }

    fn advance(&mut self, now: u64, effects: &mut Vec<Effect>) {
        if self.app.state == SessionState::Recording {
            self.app.elapsed_ms = self.app.elapsed_ms.saturating_add(now - self.last_ms);
            if self.audio()
                && self
                    .idle_since
                    .is_some_and(|t| now.saturating_sub(t) >= u64::from(self.idle_stop_s) * 1000)
            {
                self.stop(now, effects);
            }
        }
        self.last_ms = now;
        if self.ask.as_ref().is_some_and(|a| now >= a.until_ms) {
            self.idle();
        }
        if let SessionState::Stopped { resume_until_ms } = self.app.state
            && now >= resume_until_ms
        {
            let never = self.app.never_pending.is_some();
            if never || self.app.elapsed_ms < u64::from(self.discard_short_s) * 1000 {
                if let Some(meeting_id) = self.app.meeting_id.clone() {
                    effects.push(Effect::Discard { meeting_id });
                }
                if !never {
                    effects.push(Effect::NotifyShort {
                        seconds: self.discard_short_s,
                    });
                }
                self.idle();
                if self.app.quit_pending {
                    effects.push(Effect::PauseDownload);
                    effects.push(Effect::Exit);
                }
            } else {
                self.finish(effects);
            }
        }
    }
}

/// Apply one input and return work for the adapter. Late Resume/Undo cannot beat expired timers.
pub fn next(mut session: Session, input: Input) -> (Session, Vec<Effect>) {
    let before = session.app.clone();
    let now = input.now_ms.max(session.last_ms);
    let mut effects = Vec::new();
    session.advance(now, &mut effects);
    let command = if effects.contains(&Effect::Exit) {
        Command::Tick
    } else {
        input.command
    };
    match command {
        Command::StartManual {
            meeting_id,
            notes_only,
        } if session.app.state == SessionState::Idle => {
            if session.can_start(notes_only, false, &input.since, &mut effects) {
                session.start(
                    meeting_id,
                    if notes_only {
                        StartReason::NotesOnly
                    } else {
                        StartReason::Manual
                    },
                    &mut effects,
                );
            }
        }
        Command::CallDetected { call, meeting_id } => {
            if session.app.state == SessionState::Recording && !session.apps.contains(&call.app_id)
            {
                session.apps.push(call.app_id.clone());
                if let Some(id) = session.app.meeting_id.clone() {
                    effects.push(Effect::AddApp {
                        meeting_id: id,
                        app_id: call.app_id,
                    });
                }
            } else if session.app.state == SessionState::Idle
                && session.app.auto_start
                && call.mode != Mode::Off
                && session.can_start(false, true, &input.since, &mut effects)
            {
                effects.push(Effect::Emit(Event::CallDetected {
                    call_id: call.call_id.clone(),
                    app: call.app.clone(),
                    app_id: call.app_id.clone(),
                    mode: call.mode,
                }));
                if call.mode == Mode::Ask {
                    session.app.state = SessionState::Asking {
                        call_id: call.call_id.clone(),
                        app: call.app.clone(),
                    };
                    session.ask = Some(PendingAsk {
                        call: call.clone(),
                        meeting_id,
                        until_ms: now.saturating_add(ASK_MS),
                    });
                    effects.push(Effect::NotifyAsk { call });
                } else {
                    session.apps.push(call.app_id.clone());
                    session.call = Some(call.clone());
                    session.start(meeting_id, StartReason::App { app: call.app }, &mut effects);
                }
            }
        }
        Command::AnswerAsk { call_id, record } => {
            if session
                .ask
                .as_ref()
                .is_some_and(|a| a.call.call_id == call_id)
            {
                if record && session.can_start(false, false, &input.since, &mut effects) {
                    if let Some(ask) = session.ask.take() {
                        session.apps.push(ask.call.app_id.clone());
                        session.call = Some(ask.call.clone());
                        session.start(
                            ask.meeting_id,
                            StartReason::App { app: ask.call.app },
                            &mut effects,
                        );
                    }
                } else if !record {
                    session.idle();
                }
            }
        }
        Command::StopRecording => session.stop(now, &mut effects),
        Command::MicActivity { active }
            if session.app.state == SessionState::Recording && session.audio() =>
        {
            if active {
                session.idle_since = None;
            } else if session.idle_since.is_none() {
                session.idle_since = Some(now);
            }
        }
        Command::AppQuit { app_id }
            if session.call.as_ref().is_some_and(|c| c.app_id == app_id) =>
        {
            session.stop(now, &mut effects)
        }
        Command::ResumeRecording
            if matches!(session.app.state, SessionState::Stopped { .. })
                && session.app.never_pending.is_none() =>
        {
            session.resume(&input.since, &mut effects)
        }
        Command::NeverForApp { call_id }
            if session.app.state == SessionState::Recording
                && session.call.as_ref().is_some_and(|c| c.call_id == call_id) =>
        {
            if let Some(call) = session.call.clone() {
                session.stop(now, &mut effects);
                session.app.never_pending = Some(NeverPending {
                    app_id: call.app_id.clone(),
                    app: call.app,
                    previous: call.mode,
                    until_ms: now.saturating_add(GRACE_MS),
                });
                effects.push(Effect::SetAppMode {
                    app_id: call.app_id,
                    mode: Mode::Off,
                });
            }
        }
        Command::UndoNever { app_id } => {
            if session
                .app
                .never_pending
                .as_ref()
                .is_some_and(|n| n.app_id == app_id)
                && let Some(pending) = session.app.never_pending.take()
            {
                effects.push(Effect::SetAppMode {
                    app_id,
                    mode: pending.previous,
                });
                session.resume(&input.since, &mut effects);
            }
        }
        Command::RequestQuit => match session.app.state {
            SessionState::Recording
            | SessionState::Stopped { .. }
            | SessionState::Finishing { .. } => {
                session.app.quit_pending = true;
            }
            _ => {
                effects.push(Effect::PauseDownload);
                effects.push(Effect::Exit);
            }
        },
        Command::Quit { stop_and_save } if session.app.quit_pending => {
            if !stop_and_save {
                session.app.quit_pending = false;
            } else if matches!(
                session.app.state,
                SessionState::Recording | SessionState::Stopped { .. }
            ) {
                if session.app.state == SessionState::Recording && session.audio() {
                    effects.push(Effect::StopCapture);
                }
                session.finish(&mut effects);
            }
        }
        Command::QuitNow
            if matches!(session.app.state, SessionState::Finishing { .. })
                && session.app.quit_pending =>
        {
            if let Some(meeting_id) = session.app.meeting_id.clone() {
                effects.push(Effect::MarkRecoverable { meeting_id });
            }
            effects.push(Effect::PauseDownload);
            effects.push(Effect::Exit);
        }
        Command::ReadinessChanged(readiness) => {
            session
                .problems
                .retain(|p| !resolved(&p.problem, &readiness));
            session.readiness = readiness;
            if !session.readiness.notifications {
                session.app.auto_start = false;
            }
            if session.app.state == SessionState::Recording
                && !session.can_start(!session.audio(), false, &input.since, &mut effects)
            {
                session.stop(now, &mut effects);
            }
        }
        Command::ProblemRaised(problem) => {
            match &problem {
                Problem::DiskFull => session.readiness.free_bytes = 0,
                Problem::DiskLow { free_bytes } => session.readiness.free_bytes = *free_bytes,
                Problem::ModelMissing {
                    kind: ModelKind::Speech,
                } => session.readiness.speech_model = false,
                Problem::PermissionMissing { kind, .. } => match kind {
                    PermissionKind::Microphone => session.readiness.microphone = false,
                    PermissionKind::SystemAudio => session.readiness.system_audio = false,
                    PermissionKind::Notifications => {
                        session.readiness.notifications = false;
                        session.app.auto_start = false;
                    }
                },
                _ => {}
            }
            let fatal = matches!(
                problem,
                Problem::DiskFull
                    | Problem::PermissionMissing {
                        kind: PermissionKind::Microphone | PermissionKind::SystemAudio,
                        ..
                    }
                    | Problem::ToolFailed { .. }
            );
            session.problem(problem, &input.since);
            if fatal && session.app.state == SessionState::Recording && session.audio() {
                session.stop(now, &mut effects);
            }
        }
        Command::ProblemResolved { id } | Command::DismissProblem { id } => {
            session.problems.retain(|p| p.id != id)
        }
        Command::RetryProblem { id } => {
            if let Some(index) = session.problems.iter().position(|p| p.id == id) {
                effects.push(Effect::Retry {
                    problem: session.problems.remove(index),
                });
            }
        }
        Command::Finished {
            meeting_id,
            title,
            action_items,
        } if session.app.state
            == (SessionState::Finishing {
                meeting_id: meeting_id.clone(),
            }) =>
        {
            session.problems.retain(|p| !matches!(p.problem, Problem::Slow) && !matches!(&p.problem, Problem::NotesFailed { meeting_id: id, .. } if id == &meeting_id));
            effects.push(Effect::Emit(Event::NotesReady {
                meeting_id,
                title,
                action_items,
            }));
            session.idle();
            if session.app.quit_pending {
                effects.push(Effect::PauseDownload);
                effects.push(Effect::Exit);
            }
        }
        Command::FinishFailed { meeting_id, reason }
            if session.app.state
                == (SessionState::Finishing {
                    meeting_id: meeting_id.clone(),
                }) =>
        {
            session.problem(Problem::NotesFailed { meeting_id, reason }, &input.since);
            session.idle();
            session
                .problems
                .retain(|p| !matches!(p.problem, Problem::Slow));
            // Keep the failure visible instead of silently exiting with unfinished notes.
            session.app.quit_pending = false;
        }
        Command::AudioInterrupted
            if session.app.state == SessionState::Recording && session.audio() =>
        {
            if let Some(meeting_id) = session.app.meeting_id.clone() {
                effects.push(Effect::StopCapture);
                effects.push(Effect::AudioPaused {
                    meeting_id: meeting_id.clone(),
                    at_ms: session.app.elapsed_ms,
                });
                effects.push(Effect::StartCapture {
                    meeting_id,
                    resume: true,
                });
            }
        }
        Command::Recoverable { meeting_ids } => effects.push(Effect::OfferRecovery { meeting_ids }),
        _ => {}
    }
    session.app.companion.visible = match session.app.state {
        SessionState::Idle => false,
        SessionState::Asking { .. } => session.app.dnd_active,
        SessionState::Recording | SessionState::Stopped { .. } => true,
        SessionState::Finishing { .. } => session.app.quit_pending,
    };
    session.problems.sort_by_key(|p| priority(&p.problem));
    session.app.problem = session.problems.first().cloned();
    session.app.more_problems = session.problems.len().saturating_sub(1) as u32;
    let mut without_elapsed = session.app.clone();
    without_elapsed.elapsed_ms = before.elapsed_ms;
    if session.app != before
        && (without_elapsed != before || now.saturating_sub(session.last_emit_ms) >= 1000)
    {
        // Publish the visible state before any effect can start audio capture.
        effects.insert(
            0,
            Effect::Emit(Event::StateChanged(Box::new(session.app.clone()))),
        );
        session.last_emit_ms = now;
    }
    (session, effects)
}

fn priority(problem: &Problem) -> u8 {
    match problem {
        Problem::DiskFull => 0,
        Problem::PermissionMissing { .. } => 1,
        Problem::ModelMissing { .. } => 2,
        Problem::ToolFailed { .. } => 3,
        Problem::NotesFailed { .. } => 4,
        Problem::DiskLow { .. } => 5,
        Problem::Slow => 6,
    }
}

fn same_cause(a: &Problem, b: &Problem) -> bool {
    match (a, b) {
        (
            Problem::PermissionMissing { kind: a, .. },
            Problem::PermissionMissing { kind: b, .. },
        ) => a == b,
        (Problem::ModelMissing { kind: a }, Problem::ModelMissing { kind: b }) => a == b,
        (Problem::ToolFailed { tool: a, .. }, Problem::ToolFailed { tool: b, .. }) => a == b,
        (
            Problem::NotesFailed { meeting_id: a, .. },
            Problem::NotesFailed { meeting_id: b, .. },
        ) => a == b,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}

fn resolved(problem: &Problem, ready: &Readiness) -> bool {
    match problem {
        Problem::DiskFull => ready.free_bytes >= GB,
        Problem::DiskLow { .. } => ready.free_bytes >= 5 * GB,
        Problem::PermissionMissing { kind, .. } => match kind {
            PermissionKind::Microphone => ready.microphone,
            PermissionKind::SystemAudio => ready.system_audio,
            PermissionKind::Notifications => ready.notifications,
        },
        Problem::ModelMissing {
            kind: ModelKind::Speech,
        } => ready.speech_model,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
