import { useState } from 'react';
import { noteSections, taskLine } from '@/lib/document';
import { renderNotes } from '@/lib/md';
import * as ipc from '@/lib/ipc';

export default function NotesView({id, markdown, onChange, onError, sourceTimes, onSeek}: {id:string; markdown:string; onChange:(text:string)=>void; onError:(error:string)=>void; sourceTimes:number[]; onSeek?:(ms:number)=>void}) {
  const [saving, setSaving] = useState(false);
  async function toggle(line:number, done:boolean) {
    setSaving(true);
    try { onChange(await ipc.setActionItemDone(id,line,done,markdown)); }
    catch(error) { onError(String(error)); }
    finally { setSaving(false); }
  }
  const render = (text:string)=>renderNotes(text,onSeek?sourceTimes:[]);
  return <div className="notes" onClick={event=>{const chip=(event.target as HTMLElement).closest<HTMLButtonElement>('button.src');if(chip){event.preventDefault();onSeek?.(Number(chip.dataset.ms));}}}>{noteSections(markdown).map((section,index)=>{
    const tasks = section.lines.filter(line=>taskLine(line.text));
    const Heading = index < 2 ? 'h3' : 'h4';
    return <section key={index}>{section.title && <Heading>{section.title}</Heading>}
      {tasks.length ? <><ul className="todo">{tasks.map(line=>{
        const task = taskLine(line.text)!;
        return <li key={line.line} className={task.done?'done':''}><input id={`task-${line.line}`} type="checkbox" checked={task.done} disabled={saving} onChange={event=>void toggle(line.line,event.target.checked)}/><label htmlFor={`task-${line.line}`}>{task.owner&&<span className={`owner ${task.owner==='You'||task.owner==='Me'?'me':''}`}>{task.owner==='Me'?'You':task.owner}</span>}<span className="task-text" dangerouslySetInnerHTML={{__html:render(task.text)}}/></label></li>;
      })}</ul><div dangerouslySetInnerHTML={{__html:render(section.lines.filter(line=>!taskLine(line.text)).map(line=>line.text).join('\n'))}}/></> : <div dangerouslySetInnerHTML={{__html:render(section.lines.map(line=>line.text).join('\n'))}}/>}
    </section>;
  })}</div>;
}
