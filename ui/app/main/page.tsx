"use client";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import Duet from "@/components/MotetMark";
import RecentlyDeleted from "@/components/RecentlyDeleted";
import MeetingDocument from "@/components/MeetingDocument";
import { notesText, deletable } from "@/components/MeetingActions";
import { Dots, MenuIcon, MenuItem } from "@/components/MenuIcon";
import {meetingDate,meetingGroup,meetingTitle,durationLabel,fileSize} from "@/lib/document";
import { useSession } from "@/lib/useSession";
import CommandPalette from "@/components/CommandPalette";
import NotificationPermission from "@/components/NotificationPermission";
import SessionControls from "@/components/SessionControls";
import LibraryHome from "@/components/LibraryHome";
import * as ipc from "@/lib/ipc";
import { strings as s } from "@/lib/strings";
import type { Meeting } from "@/lib/bindings/Meeting";
import type { MeetingSummary } from "@/lib/bindings/MeetingSummary";

const GROUPS=["Today","This week","Earlier"];

export default function Main(){
  const session=useSession();
  const live=session.state;
  const [deleted,setDeleted]=useState<MeetingSummary[]>([]);
  const [meetings,setMeetings]=useState<MeetingSummary[]>([]);
  const [bytes,setBytes]=useState(0);
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
  // Recently deleted is a page you open, never part of the list (SPEC 7.7).
  const [trashOpen,setTrashOpen]=useState(false);
  const [rowMenu,setRowMenu]=useState<{id:string;left:number;top:number}|null>(null);
  const [rename,setRename]=useState<{id:string;n:number}|null>(null);
  const [version,setVersion]=useState(0);
  const [toast,setToast]=useState<{text:string;undo?:()=>Promise<void>}|null>(null);
  const toastTimer=useRef<ReturnType<typeof setTimeout>>(undefined);
  const menuRef=useRef<HTMLDivElement>(null);
  const selection=useRef(0);
  const openId=useRef<string|null>(null);
  openId.current=selected?.summary.id??null;
  const pendingOpen=useRef<string|null>(null);
  // Search results refresh 150 ms after the list; never show a meeting that has just been deleted.
  const shown=query.trim()?matches.filter(match=>meetings.some(meeting=>meeting.id===match.id)):meetings;
  // Sidebar order, so a deleted meeting is followed by the one below it.
  const order=GROUPS.flatMap(group=>shown.filter(meeting=>meetingGroup(meeting.created_ms)===group).map(meeting=>meeting.id));
  useEffect(()=>{
    if (!query.trim()) return;
    let active=true;
    const timer=setTimeout(()=>{ipc.searchMeetings(query).then(result=>{if(active)setMatches(result);}).catch(error=>{if(active)setError(String(error));});},150);
    return()=>{active=false;clearTimeout(timer);};
  },[query,meetings]);
  useEffect(()=>{
    const key=(event:KeyboardEvent)=>{if((event.metaKey||event.ctrlKey)&&event.key===","){event.preventDefault();void ipc.openSettings().catch(e=>setError(String(e)));}if((event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==="k"){event.preventDefault();setPalette(value=>!value);}};
    document.addEventListener("keydown",key);return()=>document.removeEventListener("keydown",key);
  },[]);
  // ⌘⌫ / Ctrl+Delete deletes the open meeting, never while typing.
  useEffect(()=>{
    const key=(event:KeyboardEvent)=>{
      const typing=(event.target as Element|null)?.closest?.("input, textarea, [contenteditable='true']");
      if(typing||!selected||trashOpen||!deletable(selected.summary.status))return;
      if((event.metaKey&&event.key==="Backspace")||(event.ctrlKey&&event.key==="Delete")){event.preventDefault();void deleteMeeting(selected.summary.id);}
    };
    document.addEventListener("keydown",key);return()=>document.removeEventListener("keydown",key);
  });
  useEffect(()=>{
    if(!rowMenu)return;
    const close=(event:PointerEvent)=>{if(!(event.target as Element|null)?.closest?.(".rowmenu, .rowmore"))setRowMenu(null);};
    document.addEventListener("pointerdown",close);return()=>document.removeEventListener("pointerdown",close);
  },[rowMenu]);
  // Beside the row, top edges aligned; moved up if it would leave the window.
  useLayoutEffect(()=>{
    const menu=menuRef.current;if(!menu||!rowMenu)return;
    const top=Math.max(8,Math.min(rowMenu.top,innerHeight-8-menu.offsetHeight));
    if(top!==rowMenu.top)menu.style.top=`${top}px`;
    menu.querySelector<HTMLButtonElement>("button")?.focus();
  },[rowMenu]);
  function notify(text:string,undo?:()=>Promise<void>){
    clearTimeout(toastTimer.current);setToast({text,undo});
    toastTimer.current=setTimeout(()=>setToast(null),undo?5000:3600);
  }
  async function refresh(){setVersion(n=>n+1);try{setMeetings(await ipc.listMeetings());setBytes(await ipc.libraryBytes());const gone=await ipc.deletedMeetings();setDeleted(gone);if(!gone.length)setTrashOpen(false);setBusy(await ipc.importing());}catch(e){setError(String(e));}}
  async function select(id:string){setRename(null);setTrashOpen(false);const request=++selection.current;try{const meeting=await ipc.getMeeting(id);if(request===selection.current){setSelected(meeting);}}catch(e){if(request===selection.current)setError(String(e));}}
  async function selectForRename(id:string){await select(id);setRename(value=>({id,n:(value?.n??0)+1}));}
  // No confirmation: Undo and 30 days in Recently deleted are the safety net (SPEC 7.7).
  async function deleteMeeting(id:string){
    setRowMenu(null);
    const index=order.indexOf(id), wasOpen=selected?.summary.id===id;
    try{await ipc.meetingAction(id,"delete");}catch(e){setError(String(e));return;}
    await refresh();
    if(wasOpen){const next=order[index+1]??order[index-1];selection.current++;setSelected(null);if(next&&next!==id)await select(next);}
    notify(s.deleted,async()=>{await ipc.meetingAction(id,"restore");await refresh();if(wasOpen)await select(id);});
  }
  async function renameMeeting(id:string,title:string){
    try{const summary=await ipc.renameMeeting(id,title);setSelected(meeting=>meeting&&meeting.summary.id===id?{...meeting,summary}:meeting);await refresh();notify(s.renamed);}
    catch(e){setError(String(e));}
  }
  async function rowAction(action:"open"|"rename"|"copy"|"reveal"|"delete",id:string){
    setRowMenu(null);
    try{
      if(action==="open")await select(id);
      if(action==="rename")await selectForRename(id);
      if(action==="copy"){const meeting=await ipc.getMeeting(id);await navigator.clipboard.writeText(notesText(meeting.notes_md,"items"));notify(s.copied);}
      if(action==="reveal")await ipc.meetingAction(id,"reveal");
      if(action==="delete")await deleteMeeting(id);
    }catch(e){setError(String(e));}
  }
  useEffect(()=>{
    setMac(/Mac/.test(navigator.platform));
    const available=ipc.desktopAvailable();setDesktop(available);
    if(!available){setLoading(false);return;}
    let active=true;const cleanups:Array<()=>void>=[];
    const subscribe=async(name:string,callback:()=>void)=>{const off=await ipc.subscribe(name,callback);if(active)cleanups.push(off);else off();};
    Promise.all([ipc.onOpenMeeting(id=>{if(active){setQuery("");void select(id);}}).then(off=>{if(active)cleanups.push(off);else off();}),refresh(),subscribe("motet://library-changed",()=>{void refresh().then(()=>{if(pendingOpen.current)void tryOpen(pendingOpen.current);else if(openId.current)void reread(openId.current);});}),subscribe("motet://quit-pending",()=>setQuitPending(true))])
      .catch(e=>{if(active)setError(String(e));}).finally(()=>{if(active)setLoading(false);});
    return()=>{active=false;selection.current++;cleanups.forEach(off=>off());};
  },[]);
  // A new recording always opens (owner's direction 2026-10-20, DESIGN 4.8). Its folder can appear
  // a moment after the state, so it opens on the worker's library-changed if not yet there.
  const liveId=live&&["Recording","Paused","Stopped"].includes(live.state.type)?live.meeting_id:null;
  // Re-read the open meeting; a short recording that was discarded simply closes.
  async function reread(id:string){const request=++selection.current;try{const meeting=await ipc.getMeeting(id);if(request===selection.current)setSelected(meeting);}catch{if(request===selection.current&&openId.current===id)setSelected(null);}}
  async function tryOpen(id:string){try{const meeting=await ipc.getMeeting(id);pendingOpen.current=null;setRename(null);setTrashOpen(false);selection.current++;setSelected(meeting);}catch{/* not created yet */}}
  useEffect(()=>{
    if(!liveId)return;
    pendingOpen.current=liveId;
    void refresh().then(()=>tryOpen(liveId));
  },[liveId]);
  // The live meeting's page follows the session: Stopped, then Writing the notes, then Ready.
  useEffect(()=>{if(openId.current&&openId.current===live?.meeting_id)void reread(openId.current);},[live?.state.type]);
  async function recordNow(){setTrashOpen(false);await session.act(ipc.startRecording);}
  async function importRecording(){setError("");setBusy(true);try{const meeting=await ipc.importAudio();await refresh();if(meeting)await select(meeting.id);}catch(e){setError(String(e));await refresh();}finally{setBusy(false);}}
  const menuMeeting=rowMenu?shown.find(meeting=>meeting.id===rowMenu.id):undefined;
  return <>{mac&&<header className="window-titlebar" data-tauri-drag-region><span className="title-brand"><Duet className="mark"/><span>Motet</span></span></header>}<main className="shell">
    <aside className="sidebar" aria-label={s.meetings}>
      <label className="search"><span className="sr-only">{s.search}</span><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5 14 14"/></svg><input placeholder={s.search} value={query} onChange={e=>setQuery(e.target.value)}/><button className="search-key" aria-label="Command palette" onClick={event=>{event.preventDefault();setPalette(true);}}><kbd>⌘K</kbd></button></label>
      <nav className="meeting-list" aria-label={s.meetings}>
        {GROUPS.map(group=>{const items=shown.filter(meeting=>meetingGroup(meeting.created_ms)===group);return items.length?<div key={group}><div className="list-heading">{group}<span>{items.length}</span></div>{items.map(meeting=>{const current=!trashOpen&&selected?.summary.id===meeting.id;return <div key={meeting.id} className={`mrow${rowMenu?.id===meeting.id?" open":""}`}>
          <button className={`meeting-row ${current?"selected":""}`} aria-current={current?"page":undefined} onClick={()=>select(meeting.id)}><strong title={meetingTitle(meeting)}>{meetingTitle(meeting)}</strong><span>{meetingDate(meeting.created_ms)}{meeting.duration_ms!=null?` · ${durationLabel(meeting.duration_ms)}`:""}{(()=>{const type=live?.meeting_id===meeting.id?live.state.type:null;
            const [cls,label]=type==="Recording"?["rec",s.chipRecording]:type==="Paused"?["plain",s.paused]:type==="Stopped"?["plain",s.chipStopped]:meeting.status==="Ready"?["ready",s.ready]:meeting.status==="Finishing"||type==="Finishing"?["attention",s.finishing]:["attention",s.failed];
            return <i className={cls}>{label}</i>;})()}</span></button>
          <button className="rowmore" aria-label={`${s.rowActions} ${meetingTitle(meeting)}`} aria-haspopup="menu" aria-expanded={rowMenu?.id===meeting.id} onClick={event=>{
            if(rowMenu?.id===meeting.id){setRowMenu(null);return;}
            const row=event.currentTarget.parentElement!.getBoundingClientRect();setRowMenu({id:meeting.id,left:row.right+14,top:row.top});
          }}><Dots/></button></div>;})}</div>:null;})}
        {query&&!shown.length&&<p className="list-empty">{s.none}</p>}
      </nav>
      {deleted.length>0&&<button className="trash" aria-current={trashOpen?"page":undefined} onClick={()=>{selection.current++;setSelected(null);setTrashOpen(true);}}><MenuIcon name="trash"/><span>{s.deletedMeetings}</span><span className="n">{deleted.length}</span></button>}
      <div className="privacy"><svg viewBox="0 0 20 20" aria-hidden="true"><rect x="5" y="8" width="10" height="9" rx="2"/><path d="M7 8V5a3 3 0 0 1 6 0v3"/></svg>{(()=>{const text=`${s.privacy} · ${meetings.length} ${meetings.length===1?s.meeting:s.meetings.toLowerCase()}${bytes>0?` · ${fileSize(bytes)}`:""}`;return <span title={text}>{text}</span>;})()}</div>
    </aside>
    <section className="workspace" aria-busy={loading||busy}>
      <NotificationPermission/><SessionControls hideActive={!!selected}/>
      {error&&<div className="banner" role="alert"><strong>{s.error}</strong><p>{error}</p><button className="quiet" onClick={()=>setError("")}>{s.dismiss}</button></div>}
      {quitPending&&<div className="banner" role="status">{s.quitPending}</div>}
      {busy?<div className="empty processing" role="status"><Duet/><h1>{s.processing}</h1><p>{s.processingBody}</p><div className="progress"/></div>:loading?<div className="empty" role="status"><p>{s.loading}</p></div>
        :trashOpen?<RecentlyDeleted deleted={deleted} refresh={refresh} notify={notify} onError={setError}/>
        :selected?<MeetingDocument key={selected.summary.id} session={session} meeting={selected} onChange={setSelected} onError={setError} renameRequest={rename?.id===selected.summary.id?rename.n:0} onRename={title=>renameMeeting(selected.summary.id,title)} onDelete={()=>void deleteMeeting(selected.summary.id)} onChanged={async removed=>{await refresh();if(removed){selection.current++;setSelected(null);}else await select(selected.summary.id);}}/>
        :meetings.length?<LibraryHome version={version} onOpen={id=>void select(id)} onImport={importRecording} onError={setError} disabled={!desktop} onDone={undo=>notify(s.itemDone,undo)}/>
        :<div className="home"><h1>{s.meetings}</h1><div className="empty"><Duet className="mark lg"/><b>{meetings.length?s.libraryTitle:s.emptyTitle}</b><span>{meetings.length?s.libraryBody:s.emptyBody}</span>
          <div className="field">{!meetings.length&&<button className="btn" onClick={()=>void recordNow()} disabled={!desktop||live?.state.type!=="Idle"}>{s.recordNow}</button>}<button className="btn ghost" onClick={importRecording} disabled={!desktop}>{s.import}</button></div>{!desktop&&<span className="hint">{s.browser}</span>}</div></div>}
      {toast&&<div className="toast" role="status"><span>{toast.text}</span>{toast.undo&&<button onClick={()=>{const undo=toast.undo!;clearTimeout(toastTimer.current);setToast(null);void undo().catch(e=>setError(String(e)));}}>{s.undo}</button>}</div>}
    </section>
    {rowMenu&&menuMeeting&&<div ref={menuRef} className="gmenu rowmenu" role="menu" aria-label={`${s.rowActions} ${meetingTitle(menuMeeting)}`} style={{left:rowMenu.left,top:rowMenu.top}} onKeyDown={event=>{
      if(event.key==="Escape"){setRowMenu(null);document.querySelector<HTMLButtonElement>(`.mrow.open .rowmore`)?.focus();}
      if(["ArrowDown","ArrowUp"].includes(event.key)){event.preventDefault();const items=Array.from(menuRef.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')??[]);const index=items.indexOf(document.activeElement as HTMLButtonElement);items[(index+(event.key==="ArrowUp"?-1:1)+items.length)%items.length]?.focus();}
    }}>
      <MenuItem icon="open" label={s.open} onClick={()=>void rowAction("open",menuMeeting.id)}/>
      <MenuItem icon="rename" label={s.rename} onClick={()=>void rowAction("rename",menuMeeting.id)}/>
      <MenuItem icon="copy" label={s.copyActionItems} onClick={()=>void rowAction("copy",menuMeeting.id)}/>
      <MenuItem icon="folder" label={s.reveal} onClick={()=>void rowAction("reveal",menuMeeting.id)}/>
      <div className="sep"/>
      <MenuItem icon="trash" label={s.deleteMeeting} danger disabled={!deletable(menuMeeting.status)} title={deletable(menuMeeting.status)?undefined:s.deleteBusy} onClick={()=>void rowAction("delete",menuMeeting.id)}/>
    </div>}
    {palette&&<CommandPalette onClose={()=>setPalette(false)} onSelect={id=>{setPalette(false);void select(id);}} onImport={()=>{setPalette(false);void importRecording();}}/>}
  </main></>;
}
