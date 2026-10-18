import type { Problem } from "./bindings/Problem";
export function problemMessage(problem: Problem): string {
  switch (problem.type) {
    case "PermissionMissing": return `Allow ${problem.kind === "Microphone" ? "Microphone" : problem.kind === "SystemAudio" ? "Screen & System Audio Recording" : "Notifications"} for Motet in System Settings > Privacy & Security, then try Record now again. You may need to restart Motet.`;
    case "ModelMissing": return "Your speech model is missing. Set WHISPER_MODEL to an installed local model before recording.";
    case "DiskFull": return "Not enough disk space to record. Free at least 1 GB and try again.";
    case "DiskLow": return "Disk space is running low. Free space to keep this recording safe.";
    case "ToolFailed": return `${problem.tool} failed: ${problem.stderr}`;
    case "NotesFailed": return `Couldn't write the notes: ${problem.reason}. Saved audio and transcripts remain on this computer.`;
    case "Slow": return "Your local models are taking longer than expected.";
  }
}
export function elapsedLabel(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
