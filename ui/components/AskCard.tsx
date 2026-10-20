"use client";
import { useEffect, useRef, useState } from "react";
import * as ipc from "@/lib/ipc";
import { strings as s } from "@/lib/strings";
import MotetMark from "@/components/MotetMark";

/** The Ask card (DESIGN 4.2): a detected call asks; nothing records until Record. Rust fades it to the Pill after 20 s. */
export function AskCard({ app, callId }: { app: string; callId: string }) {
  const [reminder,setReminder]=useState(true);
  useEffect(()=>{let active=true;ipc.getSettings().then(s=>{if(active)setReminder(s.recording_reminder);}).catch(()=>{});return()=>{active=false;};},[]);
  const [menu, setMenu] = useState(false);
  const [notice, setNotice] = useState("");
  // Leaving restarts the countdown line, like the backend's 20 s hold.
  const [lap, setLap] = useState(0);
  const split = useRef<HTMLDivElement>(null);
  // The open menu hangs below the card: Rust grows the window for it and restores 480x124 on close,
  // item chosen, Esc, click outside or unmount (the cleanup covers all of them).
  useEffect(() => {
    if (!menu) return;
    ipc.companionMenu(true).catch(() => {});
    const outside = (event: PointerEvent) => { if (!split.current?.contains(event.target as Node)) setMenu(false); };
    document.addEventListener("pointerdown", outside);
    return () => { document.removeEventListener("pointerdown", outside); ipc.companionMenu(false).catch(() => {}); };
  }, [menu]);
  // A card that unmounts while hovered must release the hold.
  useEffect(() => () => { ipc.holdAsk(false).catch(() => {}); }, []);
  async function run(command: () => Promise<void>) {
    setNotice("");
    try { await command(); } catch (e) { setNotice(String(e)); }
  }
  const hold = (on: boolean) => { if (!on) setLap(n => n + 1); void run(() => ipc.holdAsk(on)); };
  return <main className="askwrap"><div className="askcard" role="dialog" aria-label={s.askTitle.replace("{App}", app)}
    onMouseEnter={() => hold(true)} onMouseLeave={() => hold(false)}
    onKeyDown={event => { if (event.key === "Escape" && menu) { event.stopPropagation(); setMenu(false); } }}>
    <button type="button" className="x" aria-label={s.askClose} title={s.askNotThisCall} onClick={() => run(() => ipc.answerAsk(callId, false))}>✕</button>
    <div className="ic"><MotetMark className="mark"/></div>
    <div><b>{s.askTitle.replace("{App}", app)}</b><p>{notice || s.askBody}</p></div>
    <div className="split" ref={split}>
      <button type="button" className="primary" onClick={() => run(() => ipc.answerAsk(callId, true))}>{s.askRecord}</button>
      <button type="button" className="primary more" aria-haspopup="menu" aria-expanded={menu} aria-label={s.askMore} onClick={() => setMenu(open => !open)}>▾</button>
      {menu && <div className="dropdown" role="menu">
        <button type="button" role="menuitem" autoFocus onClick={() => run(() => ipc.answerAsk(callId, false))}><span>{s.askNotThisCall}</span></button>
        <button type="button" role="menuitem" onClick={() => run(() => ipc.dontAskForApp(callId))}><span>{s.askDontAsk.replace("{App}", app)}</span></button>
      </div>}
    </div>
    <div className="cline">{reminder?s.askConsent:"Nothing is recorded until you choose Record."}<i className="timer" key={lap}/></div>
  </div></main>;
}
