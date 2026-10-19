"use client";
import { useEffect, useRef, useState } from "react";
import Duet from "@/components/MotetMark";
import DeletedMeeting from "@/components/DeletedMeeting";
import MeetingDocument from "@/components/MeetingDocument";
import {meetingDate,meetingGroup} from "@/lib/document";
import CommandPalette from "@/components/CommandPalette";
import NotificationPermission from "@/components/NotificationPermission";
import SessionControls from "@/components/SessionControls";
import * as ipc from "@/lib/ipc";
import { strings as s } from "@/lib/strings";
import type { Meeting } from "@/lib/bindings/Meeting";
import type { MeetingSummary } from "@/lib/bindings/MeetingSummary";




export default function Main(){
  const [deleted,setDeleted]=useState<MeetingSummary[]>([]);
  const [meetings,setMeetings]=useState<MeetingSummary[]>([]);
  const [selected,setSelected]=useState<Meeting|null>(null);
  const [query,setQuery]=useState("");
  const [palette,setPalette]=useState(false);
  const [matches,setMatches]=useState<MeetingSummary[]>([]);
  const [loading,setLoading]=useState(true);
  const [busy,setBusy]=useState(false);
  const [desktop,setDesktop]=useState(false);
  const [mac,setMac]=useState(false);
  const [error,setError]=useState("");
  const [quitPending,setQuitPending]=useState(false);
  const selection=useRef(0);
  const shown=query.trim()?matches:meetings;
  useEffect(()=>{
    if (!query.trim()) return;
    let active=true;
    const timer=setTimeout(()=>{ipc.searchMeetings(query).then(result=>{if(active)setMatches(result);}).catch(error=>{if(active)setError(String(error));});},150);
    return()=>{active=false;clearTimeout(timer);};
  },[query,meetings]);
  useEffect(()=>{
    const key=(event:KeyboardEvent)=>{if((event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==="k"){event.preventDefault();setPalette(value=>!value);}};
    document.addEventListener("keydown",key);return()=>document.removeEventListener("keydown",key);
  },[]);
  async function refresh(){try{setMeetings(await ipc.listMeetings());setDeleted(await ipc.deletedMeetings());setBusy(await ipc.importing());}catch(e){setError(String(e));}}
  async function select(id:string){const request=++selection.current;try{const meeting=await ipc.getMeeting(id);if(request===selection.current){setSelected(meeting);}}catch(e){if(request===selection.current)setError(String(e));}}
  useEffect(()=>{
    setMac(/Mac/.test(navigator.platform));
    const available=ipc.desktopAvailable();setDesktop(available);
    if(!available){setLoading(false);return;}
    let active=true;const cleanups:Array<()=>void>=[];
    const subscribe=async(name:string,callback:()=>void)=>{const off=await ipc.subscribe(name,callback);if(active)cleanups.push(off);else off();};
    Promise.all([ipc.onOpenMeeting(id=>{if(active){setQuery("");void select(id);}}).then(off=>{if(active)cleanups.push(off);else off();}),refresh(),subscribe("motet://library-changed",()=>{void refresh();}),subscribe("motet://quit-pending",()=>setQuitPending(true))])
      .catch(e=>{if(active)setError(String(e));}).finally(()=>{if(active)setLoading(false);});
    return()=>{active=false;selection.current++;cleanups.forEach(off=>off());};
  },[]);
  async function importRecording(){setError("");setBusy(true);try{const meeting=await ipc.importAudio();await refresh();if(meeting)await select(meeting.id);}catch(e){setError(String(e));await refresh();}finally{setBusy(false);}}
  return <>{mac&&<header className="window-titlebar" data-tauri-drag-region><span className="title-brand"><Duet className="mark"/><span>Motet</span></span></header>}<main className="shell">
    <aside className="sidebar" aria-label={s.meetings}>
      <label className="search"><span className="sr-only">{s.search}</span><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5 14 14"/></svg><input placeholder={s.search} value={query} onChange={e=>setQuery(e.target.value)}/><button className="search-key" aria-label="Command palette" onClick={event=>{event.preventDefault();setPalette(true);}}><kbd>⌘K</kbd></button></label>
      <nav className="meeting-list" aria-label={s.meetings}>
        {["Today","This week","Earlier"].map(group=>{const items=shown.filter(meeting=>meetingGroup(meeting.created_ms)===group);return items.length?<div key={group}><div className="list-heading">{group}<span>{items.length}</span></div>{items.map(meeting=><button key={meeting.id} className={`meeting-row ${selected?.summary.id===meeting.id?"selected":""}`} aria-current={selected?.summary.id===meeting.id?"page":undefined} onClick={()=>select(meeting.id)}><strong>{meeting.title}</strong><span>{meetingDate(meeting.created_ms)}{meeting.duration_ms!=null?` · ${Math.floor(meeting.duration_ms/60000)} min`:""}<i className={meeting.status==="Ready"?"ready":"attention"}>{meeting.status==="Ready"?s.ready:meeting.status==="Finishing"?s.finishing:s.failed}</i></span></button>)}</div>:null;})}
        {deleted.length>0&&<><div className="list-heading">{s.deletedMeetings}</div>{deleted.map(meeting=><DeletedMeeting key={meeting.id} meeting={meeting} onChanged={refresh} onError={setError}/>)}</>}
        {query&&!shown.length&&<p className="list-empty">{s.none}</p>}
      </nav>
      <div className="privacy"><svg viewBox="0 0 20 20" aria-hidden="true"><rect x="5" y="8" width="10" height="9" rx="2"/><path d="M7 8V5a3 3 0 0 1 6 0v3"/></svg>{s.privacy} · {meetings.length} {meetings.length===1?s.meeting:s.meetings.toLowerCase()}</div>
    </aside>
    <section className="workspace" aria-busy={loading||busy}>
      <NotificationPermission/><SessionControls/>
      {error&&<div className="banner" role="alert"><strong>{s.error}</strong><p>{error}</p><button className="quiet" onClick={()=>setError("")}>{s.dismiss}</button></div>}
      {quitPending&&<div className="banner" role="status">{s.quitPending}</div>}
      {busy?<div className="empty processing" role="status"><Duet/><h1>{s.processing}</h1><p>{s.processingBody}</p><div className="progress"/></div>:loading?<div className="empty" role="status"><p>{s.loading}</p></div>:selected?<MeetingDocument key={selected.summary.id} meeting={selected} onChange={setSelected} onError={setError} onChanged={async removed=>{await refresh();if(removed){selection.current++;setSelected(null);}else await select(selected.summary.id);}}/>:<div className="home"><h1>{s.meetings}</h1><div className="empty"><b>{meetings.length?s.libraryTitle:s.emptyTitle}</b><span>{meetings.length?s.libraryBody:s.emptyBody}</span><button className="quiet" onClick={importRecording} disabled={!desktop}>{s.import}</button>{!desktop&&<span className="hint">{s.browser}</span>}</div></div>}

    </section>
    {palette&&<CommandPalette onClose={()=>setPalette(false)} onSelect={id=>{setPalette(false);void select(id);}} onImport={()=>{setPalette(false);void importRecording();}}/>}
  </main></>;
}
