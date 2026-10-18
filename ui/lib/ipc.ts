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
