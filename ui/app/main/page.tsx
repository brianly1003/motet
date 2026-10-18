"use client";
import { useEffect, useMemo, useRef, useState } from "react";
import * as ipc from "@/lib/ipc";
import { strings as s } from "@/lib/strings";
import { displayTranscript, renderNotes } from "@/lib/md";
import type { Meeting } from "@/lib/bindings/Meeting";
import type { MeetingSummary } from "@/lib/bindings/MeetingSummary";

function Duet(){return <svg className="duet" viewBox="0 0 52 52" aria-hidden="true"><mask id="duet-k" maskUnits="userSpaceOnUse" x="0" y="0" width="52" height="52"><rect width="52" height="52" fill="#fff"/><path d="M14 10v12c0 12 24 6 24 20" fill="none" stroke="#000" strokeWidth="8"/></mask><path className="them" d="M38 10v12c0 12-24 6-24 20" mask="url(#duet-k)"/><path className="me" d="M14 10v12c0 12 24 6 24 20"/><circle className="me" cx="14" cy="10" r="3.4"/><circle className="them" cx="14" cy="42" r="3.4"/></svg>;}
const date=(time:number)=>new Intl.DateTimeFormat(undefined,{month:"short",day:"numeric",hour:"numeric",minute:"2-digit"}).format(time);

export default function Main(){
  const [meetings,setMeetings]=useState<MeetingSummary[]>([]);
  const [selected,setSelected]=useState<Meeting|null>(null);
  const [query,setQuery]=useState("");
  const [tab,setTab]=useState<"notes"|"transcript">("notes");
  const [loading,setLoading]=useState(true);
  const [busy,setBusy]=useState(false);
  const [desktop,setDesktop]=useState(false);
  const [error,setError]=useState("");
  const [quitPending,setQuitPending]=useState(false);
  const selection=useRef(0);
  const shown=useMemo(()=>meetings.filter(m=>m.title.toLocaleLowerCase().includes(query.toLocaleLowerCase())),[meetings,query]);
  async function refresh(){try{setMeetings(await ipc.listMeetings());setBusy(await ipc.importing());}catch(e){setError(String(e));}}
  async function select(id:string){const request=++selection.current;try{const meeting=await ipc.getMeeting(id);if(request===selection.current){setSelected(meeting);setTab("notes");}}catch(e){if(request===selection.current)setError(String(e));}}
  useEffect(()=>{
    const available=ipc.desktopAvailable();setDesktop(available);
    if(!available){setLoading(false);return;}
    let active=true;const cleanups:Array<()=>void>=[];
    const subscribe=async(name:string,callback:()=>void)=>{const off=await ipc.subscribe(name,callback);if(active)cleanups.push(off);else off();};
    Promise.all([refresh(),subscribe("motet://library-changed",()=>{void refresh();}),subscribe("motet://quit-pending",()=>setQuitPending(true))])
      .catch(e=>{if(active)setError(String(e));}).finally(()=>{if(active)setLoading(false);});
    return()=>{active=false;selection.current++;cleanups.forEach(off=>off());};
  },[]);
  async function importRecording(){setError("");setBusy(true);try{const meeting=await ipc.importAudio();await refresh();if(meeting)await select(meeting.id);}catch(e){setError(String(e));await refresh();}finally{setBusy(false);}}
  const notes=useMemo(()=>renderNotes(selected?.notes_md??""),[selected]);
  return <main className="shell">
    <aside className="sidebar" aria-label={s.meetings}>
      <label className="search"><span className="sr-only">{s.search}</span><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5 14 14"/></svg><input placeholder={s.search} value={query} onChange={e=>setQuery(e.target.value)}/></label>
      <div className="list-heading">{s.meetings}<span>{meetings.length}</span></div>
      <nav className="meeting-list" aria-label={s.meetings}>
        {shown.map(meeting=><button key={meeting.id} className={`meeting-row ${selected?.summary.id===meeting.id?"selected":""}`} aria-current={selected?.summary.id===meeting.id?"page":undefined} onClick={()=>select(meeting.id)}><strong>{meeting.title}</strong><span>{date(meeting.created_ms)}<i className={meeting.status==="Ready"?"ready":"attention"}>{meeting.status==="Ready"?s.ready:meeting.status==="Finishing"?s.finishing:s.failed}</i></span></button>)}
        {query&&!shown.length&&<p className="list-empty">{s.none}</p>}
      </nav>
      <div className="privacy"><svg viewBox="0 0 20 20" aria-hidden="true"><rect x="5" y="8" width="10" height="9" rx="2"/><path d="M7 8V5a3 3 0 0 1 6 0v3"/></svg>{s.privacy} · {meetings.length} {meetings.length===1?s.meeting:s.meetings.toLowerCase()}</div>
    </aside>
    <section className="workspace" aria-busy={loading||busy}>
      {error&&<div className="banner" role="alert"><strong>{s.error}</strong><p>{error}</p><button className="quiet" onClick={()=>setError("")}>{s.dismiss}</button></div>}
      {quitPending&&<div className="banner" role="status">{s.quitPending}</div>}
      {busy?<div className="empty processing" role="status"><Duet/><h1>{s.processing}</h1><p>{s.processingBody}</p><div className="progress"/></div>:loading?<div className="empty" role="status"><p>{s.loading}</p></div>:selected?<article className="document">
        <header className="dochead">
          <div className="titlerow"><h1>{selected.summary.title}</h1><button className="quiet" onClick={()=>{selection.current++;setSelected(null);}}>{s.meetings}</button></div>
          <div className="document-meta">{date(selected.summary.created_ms)}<span>·</span>{s.privacy}</div>
        </header>
        <div className="tabs" aria-label={s.meetingViews}>
          <button aria-pressed={tab==="notes"} onClick={()=>setTab("notes")}>{s.notes}</button>
          {selected.transcript&&<button aria-pressed={tab==="transcript"} onClick={()=>setTab("transcript")}>{s.transcript}</button>}
        </div>
        <div className="docbody">
          {tab==="notes"?<><p className="document-hint">{s.sourceNote}</p>{selected.notes_md?<div className="notes" dangerouslySetInnerHTML={{__html:notes}}/>:<p className="muted">{s.noNotes}</p>}</>:<pre className="transcript">{displayTranscript(selected.transcript)}</pre>}
        </div>
      </article>:<div className="home"><h1>{s.meetings}</h1><div className="empty"><b>{meetings.length?s.libraryTitle:s.emptyTitle}</b><span>{meetings.length?s.libraryBody:s.emptyBody}</span><button className="quiet" onClick={importRecording} disabled={!desktop}>{s.import}</button>{!desktop&&<span className="hint">{s.browser}</span>}</div></div>}

    </section>
  </main>;
}
