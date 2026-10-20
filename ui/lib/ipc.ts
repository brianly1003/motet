import type { Meeting } from "./bindings/Meeting";
import type { MeetingSummary } from "./bindings/MeetingSummary";

export function desktopAvailable() { return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window; }
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}
export const listMeetings = () => invoke<MeetingSummary[]>("list_meetings");
export const libraryBytes = () => invoke<number>("library_bytes");
export const getMeeting = (id: string) => invoke<Meeting>("get_meeting", { id });
export const importAudio = () => invoke<MeetingSummary | null>("import_audio");
export const importing = () => invoke<boolean>("importing");
export async function subscribe(name: string, callback: () => void) {
  const { listen } = await import("@tauri-apps/api/event");
  return listen(name, callback);
}

import type { AppState } from "./bindings/AppState";
import type { Event as SessionEvent } from "./bindings/Event";
export const getState = () => invoke<AppState>("get_state");
export const startRecording = () => invoke<void>("start_recording");
export const stopRecording = () => invoke<void>("stop_recording");
export const pauseRecording = () => invoke<void>("pause_recording");
export const resumeRecording = () => invoke<void>("resume_recording");
export const quitRecording = (stopAndSave: boolean) => invoke<void>("quit", { stopAndSave });
export const dismissProblem = (id: number) => invoke<void>("dismiss_problem", { id });
export async function onSessionEvent(callback: (event: SessionEvent) => void) {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<SessionEvent>("motet://event", event => callback(event.payload));
}
export async function onNotice(callback: (notice: string) => void) {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("motet://notice", event => callback(event.payload));
}

export const setCompanionSize = (size: "Pill" | "Dot" | "Sidecar" | "Tucked") => invoke<void>("set_companion_size", { size });
export const companionReady = () => invoke<void>("companion_ready");
export const takeNote = () => invoke<void>("take_note");
export const resumeRemainingMs = () => invoke<number>("resume_remaining_ms");
export const notificationPermission = (request = false) => invoke<boolean>("notification_permission", { request });

export async function onOpenMeeting(callback: (id: string) => void) {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("motet://open-meeting", event => callback(event.payload));
}

export const markMoment = () => invoke<number>("mark_moment");

import type { MyNotes } from "./bindings/MyNotes";
export const getMyNotes = (meetingId: string) => invoke<MyNotes>("get_my_notes", { meetingId });
export const saveMyNotes = (meetingId: string, text: string, baseVersion: number) => invoke<MyNotes>("save_my_notes", { meetingId, text, baseVersion });
export const addJot = (text: string) => invoke<MyNotes>("add_jot", { text });
export const companionMenu = (open: boolean) => invoke<void>("companion_menu", { open });
export const getLevels = () => invoke<[number, number]>("get_levels");

export const liveTranscript = () => invoke<[string, string, string]>("live_transcript");

// The native drag returns when the mouse is released; Rust then snaps Tucked to the nearer edge.
export async function dragCompanion() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().startDragging();
  await invoke<void>("snap_companion");
}
export async function onCompanionFocus(callback: (focused: boolean) => void) {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow().onFocusChanged(event => callback(event.payload));
}
export async function onOpenNotes(callback: (mode: string) => void) {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("motet://open-notes", event => callback(event.payload));
}

export const pinCompanion = (pinned: boolean) => invoke<void>("pin_companion", { pinned });
export const sharingMode = (active: boolean) => invoke<void>("sharing_mode", { active });

export const openActionItems = () => invoke<[MeetingSummary, string][]>("open_action_items");
export const searchMeetings = (query: string) => invoke<MeetingSummary[]>("search_meetings", { query });

export const meetingAction = (id: string, action: "reveal" | "regenerate" | "delete" | "restore" | "purge") => invoke<void>("meeting_action", { id, action });
export const deletedMeetings = () => invoke<MeetingSummary[]>("deleted_meetings");
export const renameMeeting = (id: string, title: string) => invoke<MeetingSummary>("rename_meeting", { id, title });

import type { MeetingMedia } from './bindings/MeetingMedia';
export async function meetingMedia(id: string): Promise<MeetingMedia> {
  const media = await invoke<MeetingMedia>('meeting_media', {id});
  const {convertFileSrc} = await import('@tauri-apps/api/core');
  return {...media, tracks:media.tracks.map(track=>({...track,path:convertFileSrc(track.path)}))};
}
export const setActionItemDone = (id:string, line:number, done:boolean, expected:string) => invoke<string>('set_action_item_done',{id,line,done,expected});
