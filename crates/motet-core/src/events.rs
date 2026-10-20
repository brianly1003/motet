//! Desktop session contracts; effects are executed by the future desktop adapter.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use std::sync::{Arc, atomic::AtomicBool};

/// Shared cancellation flag for capture and live workers. Each recording gets a fresh flag.
pub type Stop = Arc<AtomicBool>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
pub enum SessionState {
    Idle,
    Asking {
        call_id: String,
        app: String,
    },
    Recording,
    /// Capture stopped by the user without ending the meeting; Resume starts a new part.
    Paused,
    Stopped {
        #[ts(type = "number")]
        resume_until_ms: u64,
    },
    Finishing {
        meeting_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AppState {
    pub state: SessionState,
    pub meeting_id: Option<String>,
    pub app: Option<String>,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
    pub started_by: Option<StartReason>,
    pub auto_start: bool,
    pub problem: Option<ProblemInfo>,
    pub more_problems: u32,
    pub quit_pending: bool,
    pub dnd_active: bool,
    pub onboarding_done: bool,
    pub sharing: Option<Sharing>,
    pub companion: CompanionState,
    pub never_pending: Option<NeverPending>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            state: SessionState::Idle,
            meeting_id: None,
            app: None,
            elapsed_ms: 0,
            started_by: None,
            auto_start: true,
            problem: None,
            more_problems: 0,
            quit_pending: false,
            dnd_active: false,
            onboarding_done: false,
            sharing: None,
            companion: CompanionState {
                visible: false,
                size: CompanionSize::Pill,
                pinned: false,
                peeking: false,
                display_id: String::new(),
            },
            never_pending: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CompanionState {
    pub visible: bool,
    pub size: CompanionSize,
    pub pinned: bool,
    pub peeking: bool,
    pub display_id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum CompanionSize {
    Dot,
    Pill,
    Sidecar,
    Tucked,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Auto,
    Ask,
    Off,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct NeverPending {
    pub app_id: String,
    pub app: String,
    pub previous: Mode,
    #[ts(type = "number")]
    pub until_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
pub enum StartReason {
    App { app: String },
    Manual,
    NotesOnly,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ProblemInfo {
    #[ts(type = "number")]
    pub id: u64,
    pub problem: Problem,
    pub since: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
pub enum Problem {
    PermissionMissing {
        kind: PermissionKind,
        side: Side,
    },
    ModelMissing {
        kind: ModelKind,
    },
    DiskLow {
        #[ts(type = "number")]
        free_bytes: u64,
    },
    DiskFull,
    Slow,
    ToolFailed {
        tool: String,
        stderr: String,
    },
    NotesFailed {
        meeting_id: String,
        reason: String,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum PermissionKind {
    Microphone,
    SystemAudio,
    Notifications,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum Side {
    TheCall,
    YourMicrophone,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ModelKind {
    Speech,
    Notes,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Sharing {
    pub hidden_from_capture: Option<bool>,
    pub shared_display: Option<String>,
    pub other_displays: Vec<Display>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Display {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
pub enum Stage {
    Recording,
    Transcribing,
    Summarizing { part: u32, of: u32 },
    Merging,
    Saving,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ActionItem {
    pub index: u32,
    pub owner: Option<String>,
    pub task: String,
    pub due: Option<String>,
    pub done: bool,
    #[ts(type = "number | null")]
    pub source_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
pub enum Event {
    StateChanged(Box<AppState>),
    CallDetected {
        call_id: String,
        app: String,
        app_id: String,
        mode: Mode,
    },
    Stage {
        meeting_id: String,
        stage: Stage,
        percent: u8,
        eta_s: Option<u32>,
    },
    NotesReady {
        meeting_id: String,
        title: String,
        action_items: Vec<ActionItem>,
    },
}

/// A detector-confirmed call; unknown apps are filtered by the detector before this boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub call_id: String,
    pub app_id: String,
    pub app: String,
    pub mode: Mode,
}

/// Fresh platform/model checks, supplied by the adapter; M1 performs no platform queries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Readiness {
    pub microphone: bool,
    pub system_audio: bool,
    pub notifications: bool,
    pub speech_model: bool,
    pub free_bytes: u64,
}

/// Inputs handled by the M1 reducer. Meeting ids are allocated by its caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    StartManual {
        meeting_id: String,
        notes_only: bool,
    },
    CallDetected {
        call: Call,
        meeting_id: String,
    },
    AnswerAsk {
        call_id: String,
        record: bool,
    },
    StopRecording,
    PauseRecording,
    ResumeRecording,
    MicActivity {
        active: bool,
    },
    AppQuit {
        app_id: String,
    },
    NeverForApp {
        call_id: String,
    },
    UndoNever {
        app_id: String,
    },
    SetCompanionSize(CompanionSize),
    SetCompanionPinned(bool),
    SetSharing(bool),
    RequestQuit,
    Quit {
        stop_and_save: bool,
    },
    QuitNow,
    Tick,
    ReadinessChanged(Readiness),
    ProblemRaised(Problem),
    ProblemResolved {
        id: u64,
    },
    RetryProblem {
        id: u64,
    },
    DismissProblem {
        id: u64,
    },
    Finished {
        meeting_id: String,
        title: String,
        action_items: Vec<ActionItem>,
    },
    FinishFailed {
        meeting_id: String,
        reason: String,
    },
    AudioInterrupted,
    Recoverable {
        meeting_ids: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_event_round_trips_with_tagged_state_and_numeric_milliseconds() {
        let state = AppState {
            state: SessionState::Stopped {
                resume_until_ms: 10_000,
            },
            meeting_id: Some("m1".into()),
            ..AppState::default()
        };
        let event = Event::StateChanged(Box::new(state));
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "StateChanged");
        assert_eq!(json["state"]["type"], "Stopped");
        assert_eq!(json["state"]["resume_until_ms"], 10_000);
        assert_eq!(serde_json::from_value::<Event>(json).unwrap(), event);
        assert!(AppState::decl(&ts_rs::Config::default()).contains("elapsed_ms: number"));
        assert!(SessionState::decl(&ts_rs::Config::default()).contains("resume_until_ms: number"));
        assert!(Problem::decl(&ts_rs::Config::default()).contains("free_bytes: number"));
        assert_eq!(serde_json::to_string(&Mode::Auto).unwrap(), "\"auto\"");
    }

    #[test]
    fn stage_and_problem_payloads_keep_their_fields() {
        let event = Event::Stage {
            meeting_id: "m1".into(),
            stage: Stage::Summarizing { part: 2, of: 3 },
            percent: 40,
            eta_s: Some(10),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["stage"]["part"], 2);
        assert_eq!(serde_json::from_value::<Event>(json).unwrap(), event);
        let problem = Problem::NotesFailed {
            meeting_id: "m1".into(),
            reason: "disk full".into(),
        };
        let json = serde_json::to_string(&problem).unwrap();
        assert_eq!(serde_json::from_str::<Problem>(&json).unwrap(), problem);
    }
}
