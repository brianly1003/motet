"use client";
import { useEffect, useState } from "react";
import * as ipc from "@/lib/ipc";
import { useSession } from "@/lib/useSession";
import { elapsedLabel } from "@/lib/session";
import { strings as s } from "@/lib/strings";
import "../../styles/companion.css";

export default function Companion() {
  const { state, notice, pending, act } = useSession();
  const [remaining, setRemaining] = useState<number | null>(null);
  const stopped = state?.state.type === "Stopped";
  useEffect(() => {
    if (!state) return;
    // Report readiness after controls have been painted, never merely after the webview loaded.
    const frame = requestAnimationFrame(() => { ipc.companionReady().catch(() => {}); });
    return () => cancelAnimationFrame(frame);
  }, [state]);
  useEffect(() => {
    if (!stopped) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function tick() {
      try { const ms = await ipc.resumeRemainingMs(); if (active) setRemaining(Math.ceil(ms / 1000)); }
      catch { if (active) setRemaining(null); }
      if (active) timer = setTimeout(tick, 250);
    }
    setRemaining(null);
    tick();
    return () => { active = false; clearTimeout(timer); };
  }, [stopped]);
  if (!state) return <div className="companion-loading">{s.loading}</div>;
  const dot = state.companion.size === "Dot" && !state.quit_pending && !stopped;
  return <main className="pill" data-size={dot ? "dot" : "pill"} role="toolbar" aria-label={s.companion}
    onKeyDown={event => { if (event.key === "Escape" && state.state.type === "Recording") act(() => ipc.setCompanionSize("Dot")); }}>
    {dot ? <button className="dotbtn" aria-label={s.showControls} title={`${s.recording} · ${elapsedLabel(state.elapsed_ms)}`} onClick={() => act(() => ipc.setCompanionSize("Pill"))}><span className="dot pulse" /></button> :
      <div className="pillbar">
        {state.quit_pending ? <>
          <span className="pillmsg">{state.state.type === "Finishing" ? s.quitPending : s.quitQuestion}</span>
          {state.state.type !== "Finishing" && <button className="primary" disabled={pending} onClick={() => act(() => ipc.quitRecording(true))}>{s.stopAndSave}</button>}
          <button className="quiet" disabled={pending} onClick={() => act(() => ipc.quitRecording(false))}>{s.stayOpen}</button>
        </> : stopped ? <>
          <span className="dot stopped" /><span className="pillmsg" role="status">{s.pillStopped} <span aria-live="off">{remaining ?? "…"} {s.seconds}</span></span>
          <button className="primary" disabled={pending || remaining === 0} onClick={() => act(ipc.resumeRecording)}>{s.resume}</button>
        </> : <>
          <span className="dot pulse" aria-hidden="true" />
          <time className="t" onPointerDown={event => { if (event.button === 0) ipc.dragCompanion().catch(() => {}); }}>{elapsedLabel(state.elapsed_ms)}</time>
          <span className="pillmsg" title={notice || s.recording}>{notice || s.recordingShort}</span>
          <button className="stop" disabled={pending} onClick={() => act(ipc.stopRecording)}>{s.pillStop}</button>
          <button className="chev" aria-label={s.shrink} title={s.shrink} disabled={pending} onClick={() => act(() => ipc.setCompanionSize("Dot"))}>···</button>
        </>}
      </div>}
    {notice && <span className="sr-only" role="alert">{notice}</span>}
  </main>;
}
