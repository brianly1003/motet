import type { Meeting } from "./bindings/Meeting";
import type { MeetingSummary } from "./bindings/MeetingSummary";

export function desktopAvailable() { return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window; }
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}
export const listMeetings = () => invoke<MeetingSummary[]>("list_meetings");
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

export const setCompanionSize = (size: "Pill" | "Dot") => invoke<void>("set_companion_size", { size });
export const companionReady = () => invoke<void>("companion_ready");
export const resumeRemainingMs = () => invoke<number>("resume_remaining_ms");
export const notificationPermission = (request = false) => invoke<boolean>("notification_permission", { request });
export async function dragCompanion() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().startDragging();
}

export async function onOpenMeeting(callback: (id: string) => void) {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("motet://open-meeting", event => callback(event.payload));
}
