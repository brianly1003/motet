"use client";
import { useEffect, useRef, useState } from "react";
import * as ipc from "@/lib/ipc";
import { elapsedLabel } from "@/lib/session";
import { strings as s } from "@/lib/strings";
import { gapLabel, transcriptLines } from "@/lib/transcript";
import { MyNotes } from "./MyNotes";
import type { AppState } from "@/lib/bindings/AppState";
import type { Stage } from "@/lib/bindings/Stage";
import type { Gap } from "@/lib/useSession";

type Session = { state: AppState; gaps: Gap[]; pending: boolean; act: (command: () => Promise<void>) => Promise<void> };

const PauseIcon = () => <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><rect x="4" y="3" width="2.6" height="10" rx="1"/><rect x="9.4" y="3" width="2.6" height="10" rx="1"/></svg>;

// The meeting page while it records (DESIGN 4.8): a place to work, not a status screen. My notes is
// the companion's my-notes.md; the live transcript is the same preview the companion polls.
export function RecordingPage({ session: { state, gaps, pending, act } }: { session: Session }) {
  const id = state.meeting_id!;
  const paused = state.state.type === "Paused", stopped = state.state.type === "Stopped";
  const [tab, setTab] = useState<"notes" | "live">("notes");
  const [live, setLive] = useState("");
  const [liveError, setLiveError] = useState("");
  const [notesKey, setNotesKey] = useState(0);
  const [marked, setMarked] = useState<number | null>(null);
  const [remaining, setRemaining] = useState<number | null>(null);
  const flush = useRef<() => Promise<void>>(async () => {});
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let active = true;
    const poll = () => ipc.liveTranscript().then(([meeting, text, error]) => { if (active && meeting === id) { setLive(text); setLiveError(error || ""); } }).catch(() => {});
    poll(); const timer = setInterval(poll, 1000);
    return () => { active = false; clearInterval(timer); };
  }, [id]);
  useEffect(() => {
    if (!stopped) return;
    let active = true;
    const tick = () => ipc.resumeRemainingMs().then(ms => { if (active) setRemaining(Math.ceil(ms / 1000)); }).catch(() => {});
    tick(); const timer = setInterval(tick, 250);
    return () => { active = false; clearInterval(timer); };
  }, [stopped]);
  useEffect(() => { if (marked === null) return; const t = setTimeout(() => setMarked(null), 2000); return () => clearTimeout(t); }, [marked]);
  const lines = transcriptLines(live, []);
  useEffect(() => { box.current?.lastElementChild?.scrollIntoView({ block: "nearest" }); }, [live, tab]);
  const gapRows = (index: number) => gaps.filter(gap => lines.findIndex(line => line.ms !== null && line.ms >= gap.at_ms) === index)
    .map(gap => <div className="line gap" key={`gap-${gap.at_ms}`}><span className="ts">{elapsedLabel(gap.at_ms)}</span><span className="who"/><span className="tx">{s.pausedFor} {gapLabel(gap.ms)}</span></div>);
  const count = lines.length;
  return <>
    <div className="recbar" role="status">
      <span className={`dot${paused || stopped ? " off" : " pulse"}`} aria-hidden="true"/>
      <b>{stopped ? s.chipStopped : paused ? s.paused : s.chipRecording}</b>
      <time className="t">{elapsedLabel(state.elapsed_ms)}</time>
      <span className="tip">{stopped ? <>The notes start in {remaining ?? "…"} s</> : paused ? "Not recording. Resume when you're ready." : marked !== null ? `${s.marked} ${elapsedLabel(marked)}.` : `Both sides · ${count} line${count === 1 ? "" : "s"}`}</span>
      <span className="sp"/>
      {stopped || paused
        ? <><button className="doc-action primary" disabled={pending || (stopped && remaining === 0)} onClick={() => act(ipc.resumeRecording)}>{s.resume}</button>
          {stopped && <button className="doc-action ghost" disabled={pending || remaining === 0} title={s.pillDiscardTitle} onClick={() => act(ipc.discardRecording)}>{s.pillDiscard}</button>}
          {paused && <button className="doc-action danger" disabled={pending} onClick={() => act(async () => { await flush.current(); await ipc.stopRecording(); })}>{s.pillStop}</button>}</>
        : <><button className="doc-action ghost" disabled={pending} title={`${s.markMoment} (⌃⌥M)`} onClick={() => act(async () => setMarked(await ipc.markMoment()))}>⚑ Mark</button>
          <button className="doc-action ghost" disabled={pending} title={s.pauseTitle} onClick={() => act(async () => { await flush.current(); await ipc.pauseRecording(); })}><PauseIcon/> {s.pause}</button>
          <button className="doc-action danger" disabled={pending} onClick={() => act(async () => { await flush.current(); await ipc.stopRecording(); })}>{s.pillStop}</button></>}
    </div>
    <div className="tabs" role="tablist" aria-label={s.meetingViews}>
      {(["notes", "live"] as const).map(key => <button key={key} type="button" role="tab" aria-selected={tab === key} onClick={() => { void flush.current(); setTab(key); }}>{key === "notes" ? "My notes" : "Live transcript"}</button>)}
    </div>
    <div className="recbody" role="tabpanel">
      {tab === "notes" ? <>
        <p className="tip">Each line keeps the time you wrote it. Start a line with <code>[]</code> action, <code>!</code> decision, <code>?</code> question or <code>#</code> topic. The companion shows the same notes.</p>
        <MyNotes key={`${id}-${notesKey}`} meetingId={id} elapsed={state.elapsed_ms} registerFlush={f => { flush.current = f; }} hints={false}/>
      </> : <>
        <div ref={box} className="lines reclines" aria-live="polite">
          {liveError && <p className="tip" role="status">{liveError}</p>}
          {lines.length ? lines.flatMap((line, i) => [...gapRows(i), <button className="line" key={i} onClick={() => act(async () => { await flush.current(); await ipc.addJot(`> ${line.text} [${line.time}]`); setNotesKey(n => n + 1); setTab("notes"); })}>
            <span className="ts">{line.time}</span><span className={`who ${line.speaker === "You" ? "me" : "them"}`}>{line.speaker}</span><span className="tx">{line.text}</span></button>])
            .concat(gapRows(-1), state.state.type === "Recording" && !liveError ? [<div className="line pending" key="pending" aria-hidden="true"><span className="ts">··:··</span><span className="who them">…</span><span className="tx">listening</span></div>] : [])
            : !liveError && <span className="tip">Listening… lines appear a few seconds after people speak.</span>}
        </div>
        <p className="tip">Click a line to quote it into My notes.</p>
      </>}
    </div>
    <p className="tip recfoot">{stopped ? "Resume keeps it one meeting." : paused ? "Paused time isn't recorded or transcribed. Motet reminds you if a pause runs past 15 minutes." : "After you stop, the transcript and notes are written on this computer. You can close this window; recording continues."}</p>
  </>;
}

// After Stop: honest steps while the local models write the notes (mockup procHtml).
export function ProcessingPage({ stage, durationMs }: { stage: Stage | null; durationMs: number }) {
  const at = stage?.type === "Saving" ? 3 : stage?.type === "Summarizing" || stage?.type === "Merging" ? 2 : 1;
  const steps: [string, string][] = [["Recorded both sides", durationMs ? elapsedLabel(durationMs) : ""], ["Transcribed", ""], [stage?.type === "Summarizing" && stage.of > 1 ? `Summarizing part ${stage.part} of ${stage.of}` : "Summarizing", ""], ["Saving", ""]];
  return <div className="writing">
    <h4>Writing the notes</h4>
    <div className="steps">{steps.map(([label, detail], i) => <div key={label} className={`step${i < at ? " done" : i === at ? " now" : ""}`}><span className="b"/><span>{label}</span><small>{detail}</small></div>)}</div>
    <div className="procbar"><i style={{ width: `${[0, 35, 65, 90][at]}%` }}/></div>
    <p className="tip">You can close this window. A notification arrives when the notes are ready.</p>
  </div>;
}
