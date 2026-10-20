import {useEffect, useRef, useState} from 'react';
import type {Meeting} from '@/lib/bindings/Meeting';
import type {MeetingMedia} from '@/lib/bindings/MeetingMedia';
import MeetingActions, { deletable } from './MeetingActions';
import NotesView from './NotesView';
import Transcript from './Transcript';
import {meetingDate, meetingTitle, timeLabel, fileSize, durationLabel, myNoteTimes, withoutTranscript} from '@/lib/document';
import {RecordingPage, ProcessingPage} from './RecordingPage';
import type {AppState} from '@/lib/bindings/AppState';
import type {Stage} from '@/lib/bindings/Stage';
import type {Gap} from '@/lib/useSession';
import {transcriptLines} from '@/lib/transcript';
import * as ipc from '@/lib/ipc';
import {strings as s} from '@/lib/strings';
import {MenuIcon} from './MenuIcon';

const TITLE_MAX=80;

export type LiveSession = {state:AppState|null; stage:Stage|null; gaps:Gap[]; pending:boolean; act:(command:()=>Promise<void>)=>Promise<void>};

export default function MeetingDocument({meeting, onChange, onChanged, onError, onRename, onDelete, renameRequest=0, session}: {session?:LiveSession; meeting:Meeting; onChange:(meeting:Meeting)=>void; onChanged:(removed:boolean)=>Promise<void>; onError:(error:string)=>void; onRename:(title:string)=>Promise<void>; onDelete:()=>void; renameRequest?:number}) {
  const [tab,setTab]=useState('Notes');
  // While this meeting records it is a working page; after Stop, the notes are being written.
  const liveType=session?.state?.meeting_id===meeting.summary.id?session.state.state.type:null;
  const live=liveType==='Recording'||liveType==='Paused'||liveType==='Stopped';
  const writing=!live&&(liveType==='Finishing'||meeting.summary.status==='Finishing');
  // Rename (SPEC 7.6): one editor, this title. Enter saves, Esc or an empty title keeps the old one.
  const title=useRef<HTMLHeadingElement>(null);
  const [editing,setEditing]=useState(false);
  const [count,setCount]=useState(0);
  const [titleKey,setTitleKey]=useState(0);
  // A ref, not state: a blur that arrives after Enter or Esc must not save a second time.
  const editingNow=useRef(false);
  function startRename(){
    const h=title.current;if(!h||editingNow.current)return;
    editingNow.current=true;setEditing(true);setCount(h.textContent?.length??0);
    requestAnimationFrame(()=>{h.focus();const range=document.createRange();range.selectNodeContents(h);const selection=getSelection();selection?.removeAllRanges();selection?.addRange(range);});
  }
  function capTitle(){
    const h=title.current;if(!h)return;
    if((h.textContent??'').length>TITLE_MAX){h.textContent=(h.textContent??'').slice(0,TITLE_MAX);const range=document.createRange();range.selectNodeContents(h);range.collapse(false);const selection=getSelection();selection?.removeAllRanges();selection?.addRange(range);}
    setCount(h.textContent?.length??0);
  }
  function endRename(save:boolean){
    if(!editingNow.current)return;
    editingNow.current=false;
    const value=(title.current?.textContent??'').replace(/\s+/g,' ').trim();
    setEditing(false);setTitleKey(key=>key+1);
    if(save&&value&&value!==meetingTitle(meeting.summary))void onRename(value);
  }
  useEffect(()=>{if(renameRequest)startRename();},[renameRequest]);
  const [media,setMedia]=useState<MeetingMedia|null>(null);
  const [playing,setPlaying]=useState(false);
  const [position,setPosition]=useState(0);
  const [showPlayer,setShowPlayer]=useState(false);
  const audio=useRef<(HTMLAudioElement|null)[]>([]);
  const cursor=useRef(0);
  const seekRequested=useRef(true);
  useEffect(()=>{
    // The tracks are still being written while it records or its notes are written.
    if(live||writing)return;
    let active=true;
    ipc.meetingMedia(meeting.summary.id).then(value=>{if(active)setMedia(value);}).catch(error=>{if(active)onError(String(error));});
    return()=>{active=false;};
  },[meeting.summary.id,live,writing]);
  useEffect(()=>{
    if(!playing || !media) return;
    let active=true;
    let lastTick=performance.now();
    const positioned=new Set<HTMLAudioElement>();
    const starting=new Set<HTMLAudioElement>();
    let leader: number | undefined;
    const tick=()=>{
      const now=performance.now();
      const explicitSeek=seekRequested.current;
      if(explicitSeek){positioned.clear();leader=undefined;seekRequested.current=false;}
      let time=cursor.current;
      if(leader!==undefined){
        const element=audio.current[leader];
        const track=media.tracks[leader];
        // The playing audio owns the clock. Buffering must not skip recorded speech.
        if(element && positioned.has(element))time=track.offset_ms+element.currentTime*1000;
      }else if(!explicitSeek && !media.tracks.some(track=>time>=track.offset_ms && time<track.offset_ms+track.duration_ms)){
        time+=now-lastTick;
      }
      lastTick=now;
      time=Math.min(media.duration_ms,time);
      cursor.current=time;setPosition(time);
      if(time>=media.duration_ms){setPlaying(false);return;}
      const available=media.tracks.map((track,index)=>({track,index})).filter(({track})=>time>=track.offset_ms && time<track.offset_ms+track.duration_ms);
      // Prefer the longest remaining track so a shorter track ending cannot stall its partner.
      if(!available.some(({index})=>index===leader))leader=available.sort((a,b)=>(b.track.offset_ms+b.track.duration_ms)-(a.track.offset_ms+a.track.duration_ms))[0]?.index;
      audio.current.forEach((element,index)=>{
        const track=media.tracks[index];
        if(!element||!track)return;
        const local=(time-track.offset_ms)/1000;
        if(local<0||local>=track.duration_ms/1000){element.pause();positioned.delete(element);return;}
        if(!element.readyState)return;
        if(!positioned.has(element)){
          element.currentTime=local;positioned.add(element);
        // Re-sync a partner only when it is clearly off: in WebKit a seek restarts the audio
        // pipeline, so a 50 ms limit seeked the audible track every ~180 ms and it stuttered.
        // Left alone, two tracks hold a steady ~85 ms start offset.
        }else if(index!==leader && !element.seeking && !element.paused && !starting.has(element) && Math.abs(element.currentTime-local)>.25){
          element.currentTime=local;
        }
        if(element.paused && !starting.has(element)){
          starting.add(element);
          void element.play().catch(error=>{if(active && error.name!=="AbortError"){setPlaying(false);onError(`Couldn't play this audio: ${String(error)}`);}}).finally(()=>starting.delete(element));
        }
      });
    };
    tick();const timer=setInterval(tick,50);
    return()=>{active=false;clearInterval(timer);audio.current.forEach(element=>element?.pause());};
  },[playing,media]);
  function seek(time:number){
    if(!media?.duration_ms)return;
    cursor.current=Math.max(0,Math.min(time,media.duration_ms));
    seekRequested.current=true;setPosition(cursor.current);setShowPlayer(true);setPlaying(true);
  }
  const sourceTimes=[...transcriptLines(meeting.transcript,[]).flatMap(line=>line.ms===null?[]:[line.ms]),...myNoteTimes(meeting.notes_md)];
  const itemTimes=[...withoutTranscript(meeting.notes_md).matchAll(/\[(\d+):(\d{2})\]/g)].map(match=>(Number(match[1])*60+Number(match[2]))*1000).filter(ms=>sourceTimes.includes(ms));
  const duration=media?.duration_ms??0;
  const tracks=media?.tracks??[];
  // One scale for both sides, set by the meeting's loudest moment, so a quietly recorded call
  // still shows its speech and You and Them stay comparable; the square root lifts quiet words.
  const loudest=Math.max(0,...tracks.flatMap(track=>track.waveform));
  function envelope(track:MeetingMedia['tracks'][number]) {
    const x=(time:number)=>duration?time/duration*1000:0;
    const sign=track.speaker==='You'?-1:1;
    const height=(peak:number)=>loudest>0?Math.sqrt(peak/loudest)*17:0;
    return `M${x(track.offset_ms)} 20 `+track.waveform.map((peak,index)=>`L${x(track.offset_ms+index/track.waveform.length*track.duration_ms)} ${20+sign*height(peak)}`).join(' ')+` L${x(track.offset_ms+track.duration_ms)} 20 Z`;
  }
  return <article className="document">
    <header className={`dochead${editing?' editing':''}`}>
      <div className="titlerow"><div className="tedit"><h1 key={titleKey} ref={title} contentEditable={editing} suppressContentEditableWarning spellCheck={false}
        onClick={()=>{if(!editing)startRename();}} onInput={capTitle} onBlur={()=>endRename(true)}
        onKeyDown={event=>{if(!editingNow.current)return;if(event.key==='Enter'){event.preventDefault();endRename(true);}if(event.key==='Escape'){event.preventDefault();event.stopPropagation();endRename(false);}}}>{meetingTitle(meeting.summary)}</h1>
        <button type="button" className="pen" aria-label={s.rename} title={s.rename} onClick={startRename}><MenuIcon name="rename"/></button></div>
        {/* Copy and ••• have nothing to act on while the meeting records or its notes are written. */}
        {deletable(meeting.summary.status)&&<MeetingActions meeting={meeting} onError={onError} onChanged={onChanged} onRename={startRename} onDelete={onDelete}/>}</div>
      <div className="edithint" aria-live="polite"><span><kbd>Enter</kbd> {s.renameSave}</span><span><kbd>Esc</kbd> {s.renameCancel}</span><span className={`count${count>=72?' near':''}`}>{count>=60?`${count}/${TITLE_MAX}`:''}</span></div>
      {live?<div className="document-meta"><span>{meetingDate(meeting.summary.created_ms)}</span>{session?.state?.app&&<><i className="sep"/><span>{session.state.app}</span></>}</div>:
      <div className="document-meta"><span>{meetingDate(meeting.summary.created_ms)}</span>{duration>0&&<><i className="sep"/><span>{durationLabel(duration)}</span></>}{meeting.summary.notes_ms!=null&&<><i className="sep"/><span>Notes in {Math.round(meeting.summary.notes_ms/1000)} s</span></>}</div>}
      {!live&&!writing&&<div className="tl">
        <div className="timeline-wave"><svg viewBox="0 0 1000 40" preserveAspectRatio="none" aria-label="Recorded audio: You above, Them below">
          {tracks.map(track=><path key={track.path} className={track.speaker==='You'?'tl-me':'tl-them'} d={envelope(track)}/>)}
          <line x1="0" x2="1000" y1="20" y2="20" className="tl-mid"/>
          {duration>0&&meeting.summary.marks_ms.map(mark=><line key={mark} x1={mark/duration*1000} x2={mark/duration*1000} y1="2" y2="38" className="tl-mark"/>)}
          {duration>0&&[...new Set(itemTimes)].map(ms=><circle key={ms} cx={ms/duration*1000} cy="20" r="4" className="tl-item"/>)}
          {showPlayer&&duration>0&&<line x1={position/duration*1000} x2={position/duration*1000} y1="0" y2="40" className="tl-thread"/>}
        </svg><input type="range" aria-label="Play recording from time" min="0" max={duration||1} value={position} disabled={!duration} onChange={event=>{seek(Number(event.target.value));setTab('Transcript');}}/></div>
        <div className="tl-legend"><span className="lm">You</span><span className="lt">Them</span><span>{duration?timeLabel(duration):'No saved audio'}</span></div>
      </div>}
    </header>
    {live&&session?.state?<RecordingPage session={{state:session.state,gaps:session.gaps,pending:session.pending,act:session.act}}/>:writing?<ProcessingPage stage={session?.stage??null} durationMs={meeting.summary.duration_ms??session?.state?.elapsed_ms??0}/>:<>
    <div className="tabs" role="tablist" aria-label={s.meetingViews}>{['Notes','Transcript','Audio'].map((name,index)=><button key={name} id={`tab-${name}`} type="button" role="tab" aria-selected={tab===name} aria-controls="meeting-panel" tabIndex={tab===name?0:-1} onClick={()=>setTab(name)} onKeyDown={event=>{
      const names=['Notes','Transcript','Audio'];let next=index;
      if(event.key==='ArrowRight')next=(index+1)%3;else if(event.key==='ArrowLeft')next=(index+2)%3;else if(event.key==='Home')next=0;else if(event.key==='End')next=2;else return;
      event.preventDefault();setTab(names[next]);document.getElementById(`tab-${names[next]}`)?.focus();
    }}>{name}</button>)}</div>
    <div className="docbody" id="meeting-panel" role="tabpanel" aria-labelledby={`tab-${tab}`}>
      {tab==='Notes'?(meeting.notes_md?<NotesView id={meeting.summary.id} markdown={meeting.notes_md} sourceTimes={sourceTimes} onSeek={duration?ms=>{seek(ms);setTab('Transcript');}:undefined} onChange={notes_md=>onChange({...meeting,notes_md})} onError={onError}/>:<p className="muted">{s.noNotes}</p>):tab==='Transcript'?<Transcript text={meeting.transcript} marks={meeting.summary.marks_ms} onSeek={duration?seek:undefined} position={showPlayer?position:undefined}/>:<div className="notes"><section><h3>Audio</h3>{tracks.length?<div className="audio-table"><table><thead><tr><th>Track</th><th>Who</th><th>Size</th></tr></thead><tbody>{tracks.map(track=><tr key={track.path}><td className="mono"><button className="track-play" onClick={()=>seek(track.offset_ms)}>{track.name}</button></td><td>{track.speaker}</td><td className="mono">{fileSize(track.bytes)}</td></tr>)}</tbody></table></div>:<p className="tip">No saved audio is available. Notes and transcript are kept.</p>}</section></div>}
      <aside className="meeting-aside">
        <div className="panel"><h5>This meeting</h5><dl className="kv"><dt>Length</dt><dd>{duration?timeLabel(duration):'Not recorded'}</dd><dt>Language</dt><dd>Not recorded</dd><dt>Notes in</dt><dd>{meeting.summary.notes_ms!=null?`${Math.round(meeting.summary.notes_ms/1000)} s`:"Not recorded"}</dd><dt>Marks</dt><dd>{meeting.summary.marks_ms.length}</dd></dl></div>
        <div className="panel"><h5>Spellings used</h5>{meeting.summary.spellings?.length?<div className="spelling-chips">{meeting.summary.spellings.map(term=><span key={term} className="chip plain">{term}</span>)}</div>:<p className="tip">No saved spellings</p>}</div>
        <div className="panel"><h5>Stays on this computer</h5><p className="tip">No account, no upload. Folder: <span>{media?.folder??'Loading…'}</span></p></div>
      </aside>
    </div>
    {(showPlayer||tab==='Transcript')&&duration>0&&<div className="player"><button className="doc-action" onClick={()=>{setShowPlayer(true);if(cursor.current>=duration){cursor.current=0;seekRequested.current=true;setPosition(0);}setPlaying(!playing);}}>{playing?'❚❚ Pause':'▶ Play'}</button><span>{timeLabel(position)}</span><div className="bar"><i style={{width:`${position/duration*100}%`}}/></div><span className="tip">{tracks.some(t=>t.speaker==='You')&&tracks.some(t=>t.speaker==='Them')?'Both sides':'Audio'}</span></div>}
    {tracks.map((track,index)=><audio key={track.path} ref={element=>{audio.current[index]=element;}} src={track.path} preload="metadata"/>)}
    </>}
  </article>;
}
