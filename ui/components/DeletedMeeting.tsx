"use client";
import { useRef, useState } from "react";
import type { MeetingSummary } from "@/lib/bindings/MeetingSummary";
import { meetingAction } from "@/lib/ipc";
import { strings as s } from "@/lib/strings";

export default function DeletedMeeting({meeting, onChanged, onError}: {meeting: MeetingSummary; onChanged: () => Promise<void>; onError: (message: string) => void}) {
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const deleteButton = useRef<HTMLButtonElement>(null);
  function cancel() { setConfirm(false); deleteButton.current?.focus(); }
  async function run(action: "restore" | "purge") {
    setBusy(true);
    try { await meetingAction(meeting.id, action); await onChanged(); }
    catch (error) { onError(String(error)); }
    finally { setBusy(false); }
  }
  return <div className="meeting-row deleted-meeting"><strong>{meeting.title}</strong>
    {confirm ? <div className="delete-confirm" role="alertdialog" aria-label={`Delete “${meeting.title}” permanently?`} onKeyDown={event => { if(event.key === "Escape" && !busy) cancel(); }}>
      <p>Delete “{meeting.title}” permanently? {s.permanentDeleteWarning}</p>
      <button className="doc-action" disabled={busy} onClick={() => run("purge")}>{s.deletePermanently}</button>
      <button className="doc-action ghost" autoFocus disabled={busy} onClick={cancel}>{s.cancel}</button>
    </div> : <div className="deleted-actions"><button className="doc-action" disabled={busy} onClick={() => run("restore")}>{s.restoreMeeting}</button><button ref={deleteButton} className="doc-action ghost" disabled={busy} onClick={() => setConfirm(true)}>{s.deletePermanently}</button></div>}
  </div>;
}
