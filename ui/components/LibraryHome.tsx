import { useEffect, useState } from "react";
import { noteSections, taskLine } from "@/lib/document";
import { renderInline } from "@/lib/md";
import * as ipc from "@/lib/ipc";
import { strings as s } from "@/lib/strings";
import type { MeetingSummary } from "@/lib/bindings/MeetingSummary";

type Item = { meeting: MeetingSummary; notes: string; line: number; owner: string; text: string; src: string };

// Unchecked action items across meetings; also counted on the sidebar's Action items row.
export function openItems(open: unknown): Item[] {
  // Anything but a list (an older backend without this command) means nothing open.
  return (Array.isArray(open) ? open as [MeetingSummary, string][] : []).flatMap(([meeting, notes]) => noteSections(notes).flatMap(section => section.lines.flatMap(line => {
    const task = taskLine(line.text);
    if (!task || task.done) return [];
    const src = /\[(\d+:\d{2})\]\s*$/.exec(task.text)?.[1] ?? "";
    return [{meeting, notes, line: line.line, owner: task.owner === "Me" ? "You" : task.owner, text: task.text.replace(/\s*\[\d+:\d{2}\]\s*$/, ""), src}];
  })));
}

// Library home (mockup renderHome): unchecked action items across meetings, then the import drop zone.
export default function LibraryHome({version, onOpen, onImport, onDone, onError, onCount, disabled, disabledReason}: {version: number; disabledReason?: string; onCount: (n: number) => void; onOpen: (id: string) => void; onImport: () => void; onDone: (undo: () => Promise<void>) => void; onError: (error: string) => void; disabled: boolean}) {
  const [items, setItems] = useState<Item[] | null>(null);
  const [reload, setReload] = useState(0);
  useEffect(() => {
    let active = true;
    ipc.openActionItems().then(open => { if (active) setItems(openItems(open)); }).catch(error => { if (active) onError(String(error)); });
    return () => { active = false; };
  }, [version, reload, onError]);
  async function done(item: Item) {
    try {
      const notes = await ipc.setActionItemDone(item.meeting.id, item.line, true, item.notes);
      setReload(n => n + 1);
      onDone(async () => { await ipc.setActionItemDone(item.meeting.id, item.line, false, notes); setReload(n => n + 1); });
    } catch (error) { onError(String(error)); }
  }
  useEffect(() => { if (items) onCount(items.length); }, [items, onCount]);
  const meetings = new Set(items?.map(item => item.meeting.id)).size;
  return <div className="home">
    <div className="titlerow"><h1>{s.openItemsTitle}</h1>{items && <span className="tip">{s.openItemsCount.replace("{n}", String(items.length)).replace("{m}", String(meetings))}</span>}</div>
    {items && (items.length ? <div className="openitems">{items.map(item => <div className="oi" key={`${item.meeting.id}-${item.line}`}>
      <input type="checkbox" aria-label={s.markDone} onChange={() => void done(item)}/>
      <span>{item.owner && <span className={`owner${item.owner === "You" ? " me" : ""}`}>{item.owner}</span>}<span dangerouslySetInnerHTML={{__html: renderInline(item.text)}}/></span>
      <button type="button" className="from" onClick={() => onOpen(item.meeting.id)}>{item.meeting.title}{item.src && ` · ${item.src}`}</button>
    </div>)}</div> : <p className="tip">{s.noOpenItems}</p>)}
    <div className="drop"><span>{s.dropHint}</span><button type="button" className="doc-action" onClick={onImport} disabled={disabled} title={disabledReason || undefined}>{s.import}</button></div>
  </div>;
}
