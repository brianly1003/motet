"use client";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import * as ipc from "@/lib/ipc";
import { useSession } from "@/lib/useSession";
import { elapsedLabel } from "@/lib/session";
import { strings as s } from "@/lib/strings";
import MotetMark from "@/components/MotetMark";
import { Duet } from "@/components/Duet";
import { MyNotes } from "@/components/MyNotes";
import { transcriptLines } from "@/lib/transcript";
import "../../styles/companion.css";

export default function Companion() {
  const { state, notice, pending, act } = useSession();
  const [remaining, setRemaining] = useState<number | null>(null);
  const [markedAt, setMarkedAt] = useState<number | null>(null);
  useEffect(() => {
    if (markedAt === null) return;
    const timeout = setTimeout(() => setMarkedAt(null), 600);
    return () => clearTimeout(timeout);
  }, [markedAt]);
  async function mark() { await act(async () => { setMarkedAt(await ipc.markMoment()); }); }
  const flushNotes = useRef<() => Promise<void>>(async () => {});
  const [menu, setMenu] = useState(false);
  const [liveError, setLiveError] = useState("");
  const [live, setLive] = useState("");
  const [showLive, setShowLive] = useState(true);
  const liveLines = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const box = liveLines.current;
    if (box && showLive) box.scrollTop = box.scrollHeight;
  }, [live, showLive, state?.companion.size, state?.state.type, state?.quit_pending]);
  const pinned = state?.companion.pinned ?? false;
  const [jot, setJot] = useState("");
  const [jotOpen, setJotOpen] = useState(false);
  const [notesKey, setNotesKey] = useState(0);
  const meetingId = state?.meeting_id;
  useEffect(() => {
    let active = true;
    let off: (() => void) | undefined;
    ipc.onCompanionFocus(focused => {
      if (!focused && menu) { setMenu(false); void ipc.companionMenu(false); }
      if (!focused && state?.companion.size === "Sidecar" && !pinned) void act(() => size("Tucked"));
    }).then(cleanup => { if (active) off = cleanup; else cleanup(); }).catch(() => {});
    return () => { active = false; off?.(); };
  }, [state?.companion.size, pinned, menu]);
  useEffect(() => {
    let active = true;
    let off: (() => void) | undefined;
    ipc.onOpenNotes(mode => { if (mode === "jot") { setJotOpen(true); void ipc.companionMenu(true); } })
      .then(cleanup => { if (active) off = cleanup; else cleanup(); }).catch(() => {});
    return () => { active = false; off?.(); };
  }, []);

  useEffect(() => {
    let active = true;
    const timer = setInterval(() => { ipc.liveTranscript().then(([id, text, error]) => { if (active && id === meetingId) { setLive(text); setLiveError(error || ""); } }).catch(() => {}); }, 1000);
    setLive("");
    setLiveError("");
    return () => { active = false; clearInterval(timer); };
  }, [meetingId]);
  async function size(value: "Dot" | "Pill" | "Sidecar" | "Tucked") {
    await flushNotes.current(); await ipc.companionMenu(false); setMenu(false); setJotOpen(false); await ipc.setCompanionSize(value);
  }
  async function stop() { await flushNotes.current(); await ipc.stopRecording(); }
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
  const sidecar = state.companion.size === "Sidecar" && !stopped && !state.quit_pending;
  const tucked = state.companion.size === "Tucked" && !stopped && !state.quit_pending;
  const lines = transcriptLines(live, []);
  const latest = lines.at(-1);
  return <><main className="pill" data-size={dot ? "dot" : tucked ? "tucked" : sidecar ? "sidecar" : "pill"} data-peek={state.companion.peeking} role="toolbar" tabIndex={0} aria-label={s.companion}
    onKeyDown={event => {
      if (event.code === "Space" && event.target === event.currentTarget && state.state.type === "Recording") { event.preventDefault(); void mark(); }
      if (event.key === "Escape" && state.state.type === "Recording") {
        event.preventDefault();
        if (menu || jotOpen) { setMenu(false); setJotOpen(false); void act(() => ipc.companionMenu(false)); }
        else void act(() => size(sidecar ? "Tucked" : "Dot"));
      }
    }}>
    {tucked ? <button className="handle" aria-label="Recording. Open notes and live transcript" onClick={() => act(() => size("Sidecar"))}><span className="dot pulse" /><MotetMark className="mark"/></button> : dot ? <button className="dotbtn" aria-label={s.showControls} title={`${s.recording} · ${elapsedLabel(state.elapsed_ms)}`} onClick={() => act(() => ipc.setCompanionSize("Pill"))}><span className="dot pulse" /></button> :
      <div className="pillbar" data-tauri-drag-region>
        {state.quit_pending ? <>
          <span className="pillmsg">{notice || (state.state.type === "Finishing" ? s.quitPending : s.pillQuitQuestion)}</span>
          <button className="quiet" disabled={pending} onClick={() => act(() => ipc.quitRecording(false))}>{state.state.type === "Recording" ? s.keepRecording : s.stayOpen}</button>
          {state.state.type !== "Finishing" && <button className="primary" disabled={pending} onClick={() => act(async () => { await flushNotes.current(); await ipc.quitRecording(true); })}>{s.stopAndSave}</button>}
        </> : stopped ? <>
          <span className="dot stopped" /><span className="pillmsg" role="status">{notice || <>{s.pillStopped} <span aria-live="off">{remaining ?? "…"} {s.seconds}</span></>}</span>
          <button className="primary" disabled={pending || remaining === 0} onClick={() => act(ipc.resumeRecording)}>{s.resume}</button>
        </> : <>
          <span className="dot pulse" aria-hidden="true" />
          <time className="t">{elapsedLabel(state.elapsed_ms)}</time>
          <Duet />
          <button className="iconbtn" disabled={pending} onClick={mark} aria-label={s.markMoment} title={markedAt === null ? s.markMoment : `${s.marked} ${elapsedLabel(markedAt)}`} aria-pressed={markedAt !== null}>⚑</button>
          <button className="stop" disabled={pending} onClick={() => act(stop)}>{s.pillStop}</button>
          {sidecar ? <><button className="iconbtn" aria-label="Keep open" title="Keep open while I work elsewhere" aria-pressed={pinned} onClick={() => act(() => ipc.pinCompanion(!pinned))}><svg viewBox="0 0 16 16" fill="none" stroke="currentColor"><path d="M6 2h4l-.5 4 2.5 2.5H4L6.5 6zM8 8.5V14"/></svg></button><button className="iconbtn" aria-label="Tuck to the screen edge" onClick={() => act(() => size("Tucked"))}><svg viewBox="0 0 16 16" fill="none" stroke="currentColor"><path d="M3 8h8M8 5l3 3-3 3M13.5 3v10"/></svg></button></> : <button className="iconbtn" aria-label="Open my notes" title="Take a note (⌃⌥N)" onClick={() => act(() => size("Sidecar"))}><svg viewBox="0 0 16 16" fill="none" stroke="currentColor"><path d="M10.5 2.5l3 3L6 13H3v-3z"/></svg></button>}
          <button className="iconbtn" aria-label="Change size" title="Size" aria-haspopup="menu" aria-expanded={menu} disabled={pending} onClick={() => act(async () => { await ipc.companionMenu(!menu); setMenu(!menu); })}><svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true"><circle cx="3.5" cy="8" r="1"/><circle cx="8" cy="8" r="1"/><circle cx="12.5" cy="8" r="1"/></svg></button>
        </>}
      </div>}
    {!dot && !tucked && !sidecar && <button className="caption" aria-label="Latest line. Click to open the live transcript" onClick={() => act(() => size("Sidecar"))}><span className={`who ${latest?.speaker === "You" ? "me" : "them"}`}>{latest?.speaker || "…"}</span><span className="ctext">{notice || liveError || latest?.text || "Listening… lines appear a few seconds after people speak."}</span></button>}
    {jotOpen && <form className="jotbar" onSubmit={event => { event.preventDefault(); void act(async () => { await flushNotes.current(); await ipc.addJot(jot); setJot(""); setNotesKey(key => key + 1); setJotOpen(false); await ipc.companionMenu(false); }); }}><input type={state.sharing ? "password" : "text"} autoFocus value={jot} onChange={event => setJot(event.target.value)} placeholder="Jot a line into My notes…" aria-label="Jot a line into My notes"/><kbd>⌃⌥J</kbd><button type="submit" className="quiet">Add</button></form>}
    {sidecar && meetingId && <div className="pilldrawer">{state.companion.peeking && <div className="sharebadge">Visible to your share</div>}<div className="why"><span>{notice || (state.started_by?.type === "App" ? `Started because ${state.started_by.app} is using your microphone.` : "Started manually.")}</span><button className="quiet" onClick={() => act(() => navigator.clipboard.writeText("I'm recording this call with Motet to take notes. Audio and notes stay on my computer."))}>Copy a recording notice</button></div><button className="panehead" aria-expanded={showLive} onClick={() => setShowLive(!showLive)}><span>Live</span><span className="tip">click a line to quote it</span></button><div ref={liveLines} className="lines" hidden={!showLive}>{liveError && <p className="tip" role="status">{liveError}</p>}{lines.length ? lines.map((line, i) => <button className="line" key={i} onClick={() => act(async () => { await flushNotes.current(); await ipc.addJot(`> ${line.text} [${line.time}]`); setNotesKey(key => key + 1); })}><span className="ts">{line.time}</span><span className={`who ${line.speaker === "You" ? "me" : "them"}`}>{line.speaker}</span><span className="tx">{line.text}</span></button>) : !liveError && <span className="tip">Listening… lines appear a few seconds after people speak.</span>}</div><MyNotes key={`${meetingId}-${notesKey}`} meetingId={meetingId} elapsed={state.elapsed_ms} registerFlush={flush => { flushNotes.current = flush; }}/></div>}
    {markedAt !== null && <span className="sr-only" role="status">{s.marked} {elapsedLabel(markedAt)}</span>}
    {notice && <span className="sr-only" role="alert">{notice}</span>}
  </main>{menu && <div className="dropdown sizemenu" role="menu" aria-label="Companion size" onKeyDown={event => {
    if (event.key === "Escape") { setMenu(false); void act(() => ipc.companionMenu(false)); document.querySelector<HTMLButtonElement>('[aria-label="Change size"]')?.focus(); }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); const buttons=Array.from(event.currentTarget.querySelectorAll("button")); const index=buttons.indexOf(document.activeElement as HTMLButtonElement); buttons[(index+(event.key === "ArrowDown"?1:-1)+buttons.length)%buttons.length]?.focus(); }
  }}>{([["Dot", "Dot"], ["Pill", "Pill"], ["Tucked", "Tucked at the edge"], ["Sidecar", "Notes and live transcript"]] as const).map(([value, label]) => <button role="menuitemradio" aria-checked={state.companion.size === value} key={value} onClick={() => act(() => size(value))}><span>{label}</span><kbd>{value === "Sidecar" ? "⌃⌥N" : value === "Tucked" ? "⌃⌥Space" : ""}</kbd></button>)}<div className="sep"/><button role="menuitemcheckbox" aria-checked={!!state.sharing} onClick={() => act(async () => { await flushNotes.current(); setMenu(false); await ipc.companionMenu(false); await ipc.sharingMode(!state.sharing); })}><span>Sharing mode</span><kbd>⌃⌥H</kbd></button></div>}</>;
}
