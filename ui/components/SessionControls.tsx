"use client";
import { useSession } from "@/lib/useSession";
import * as ipc from "@/lib/ipc";
import { elapsedLabel, problemMessage } from "@/lib/session";
import { strings as s } from "@/lib/strings";

// hideActive: on any meeting page; the live meeting's page shows its bar and the sidebar chip
// (Recording, Paused, Writing notes) already says what the live one is doing (owner, 2026-10-20).
export default function SessionControls({ hideActive = false }: { hideActive?: boolean }) {
  const { state, stage, notice, setNotice, pending, act } = useSession();
  const recording = state?.state.type === "Recording";
  const paused = state?.state.type === "Paused";
  const stopped = state?.state.type === "Stopped";
  const finishing = state?.state.type === "Finishing";
  return <>
    {(recording || paused || stopped || finishing) && !hideActive && <div className={`banner session-banner${recording ? " is-recording" : ""}`} role="status">
      <span>{recording ? s.recording : paused ? s.pillPaused : stopped ? s.stopped : stage?.type === "Transcribing" ? s.transcribing : s.processing} {state && !finishing && <time>{elapsedLabel(state.elapsed_ms)}</time>}</span>
      {recording && <button className="quiet" disabled={pending} onClick={() => act(ipc.pauseRecording)}>{s.pause}</button>}
      {paused && <button className="quiet" disabled={pending} onClick={() => act(ipc.resumeRecording)}>{s.resume}</button>}
      {(recording || paused) && <button className="quiet" disabled={pending} onClick={() => act(ipc.stopRecording)}>{s.stop}</button>}
      {stopped && <button className="quiet" disabled={pending} onClick={() => act(ipc.resumeRecording)}>{s.resume}</button>}
    </div>}
    {state?.quit_pending && <div className="banner" role="alert">
      <p>{finishing ? s.quitPending : s.quitQuestion}</p>
      {!finishing && <button className="quiet" disabled={pending} onClick={() => act(() => ipc.quitRecording(true))}>{s.stopAndSave}</button>}
      <button className="quiet" disabled={pending} onClick={() => act(() => ipc.quitRecording(false))}>{recording ? s.keepRecording : s.stayOpen}</button>
    </div>}
    {state?.problem && <div className="banner" role="alert"><p>{problemMessage(state.problem.problem)}</p><button className="quiet" disabled={pending} onClick={() => act(() => ipc.dismissProblem(state.problem!.id))}>{s.dismiss}</button></div>}
    {notice && <div className="banner" role="status"><p>{notice}</p><button className="quiet" onClick={() => setNotice("")}>{s.dismiss}</button></div>}
  </>;
}
