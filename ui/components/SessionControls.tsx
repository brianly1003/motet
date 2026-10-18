"use client";
import { useSession } from "@/lib/useSession";
import * as ipc from "@/lib/ipc";
import { elapsedLabel, problemMessage } from "@/lib/session";
import { strings as s } from "@/lib/strings";

export default function SessionControls() {
  const { state, stage, notice, setNotice, pending, act } = useSession();
  const recording = state?.state.type === "Recording";
  const stopped = state?.state.type === "Stopped";
  const finishing = state?.state.type === "Finishing";
  return <>
    {(recording || stopped || finishing) && <div className={`banner session-banner${recording ? " is-recording" : ""}`} role="status">
      <span>{recording ? s.recording : stopped ? s.stopped : stage?.type === "Transcribing" ? s.transcribing : s.processing} {state && !finishing && <time>{elapsedLabel(state.elapsed_ms)}</time>}</span>
      {recording && <button className="quiet" disabled={pending} onClick={() => act(ipc.stopRecording)}>{s.stop}</button>}
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
