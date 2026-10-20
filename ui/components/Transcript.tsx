import { useEffect, useRef, useState } from "react";
import { transcriptLines } from "@/lib/transcript";
import { elapsedLabel } from "@/lib/session";
import { strings as s } from "@/lib/strings";
import * as ipc from "@/lib/ipc";
import { handBack } from "@/lib/focus";

const esc = (text: string) => text.replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]!));
const mac = () => typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

// As in the mockup: the time plays from its line; the words are the line's text, edited in place
// (SPEC 7.11). The speaker and time never change; an emptied line is removed but keeps its place,
// and what was heard stays one click away (Revert).
export default function Transcript({id, text, heard, marks, locked = false, onSeek, onReplay, onChange, onError, position}: {id: string; text: string; heard: string; marks: number[]; locked?: boolean; onSeek?: (ms: number) => void; onReplay?: (start: number, end: number) => void; onChange: (text: string, heard: string) => void; onError: (error: string) => void; position?: number}) {
  const lines = transcriptLines(text, marks);
  const heardLines = heard ? transcriptLines(heard, []) : null;
  const [focused, setFocused] = useState(-1);
  const playing = position === undefined ? -1 : lines.findLastIndex(line => line.ms !== null && line.ms <= position);
  const box = useRef<HTMLDivElement>(null);
  const history = useRef<{index: number; text: string}[]>([]);
  // Playback never pulls the view or the highlight onto the line being edited.
  useEffect(() => { if (playing >= 0 && focused < 0) box.current?.children[playing]?.scrollIntoView({block: "nearest"}); }, [playing, focused]);

  async function save(index: number, value: string, undoable = true) {
    const before = lines[index]?.text ?? "";
    if (value === before) return;
    // What was heard is the transcript before the first edit; once every line matches it again, there is no edit (as transcript_edits.rs).
    try { const next = await ipc.saveTranscriptLine(id, index, value), was = heard || text; onChange(next, next === was ? "" : was); if (undoable) history.current.push({index, text: before}); }
    catch (error) {
      onError(String(error));
      const words = box.current?.querySelector<HTMLElement>(`[data-line="${index}"] .words`); if (words) words.textContent = before;
    }
  }
  const words = (index: number) => box.current?.querySelector<HTMLElement>(`[data-line="${index}"] .words`);
  function caretAt(el: HTMLElement, start: boolean) {
    el.focus(); const r = document.createRange(); r.selectNodeContents(el); r.collapse(start);
    const sel = getSelection(); sel?.removeAllRanges(); sel?.addRange(r);
  }
  // ⌘Z / Ctrl+Z with no caret in a line undoes the last saved line edit.
  useEffect(() => {
    const undo = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() !== "z" || !(e.metaKey || e.ctrlKey) || e.shiftKey || e.altKey || !history.current.length) return;
      const active = document.activeElement as HTMLElement | null;
      if (active?.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(active?.tagName ?? "")) return;
      e.preventDefault(); const last = history.current.pop()!; void save(last.index, last.text, false);
    };
    document.addEventListener("keydown", undo);
    return () => document.removeEventListener("keydown", undo);
  });

  const editable = !locked;
  return <div className="transcript-edit">
    <div ref={box} className="tlist" tabIndex={0} role="document" aria-label={locked ? s.transcriptLockedLabel : s.transcriptLabel}
      onKeyDown={event => { if (event.key === "Enter" && event.target === box.current) { event.preventDefault(); const w = words(0); if (w) caretAt(w, false); } }}>
      {lines.map((line, index) => {
        const side = line.speaker === "You" ? "me" : "them", original = heardLines?.[index]?.text ?? line.text;
        const removed = !line.text.trim() && !!original.trim(), edited = !!heardLines && line.text !== original;
        const next = lines.slice(index + 1).find(l => l.ms !== null)?.ms;
        const seek = onSeek && line.ms !== null ? () => onSeek(line.ms!) : undefined;
        return <div className={`tline ${side}${index === playing && index !== focused ? " playing" : ""}${edited ? " edited" : ""}${removed ? " removed" : ""}`} key={index} data-line={index}>
          <button className="ts" disabled={!seek} onClick={seek} aria-label={`Play from ${line.time}`}>{line.time}</button><span className={`who ${side}`}>{line.speaker}</span>
          <span className="tx">
            {removed
              ? <span className="rmv"><s>{original}</s><span>{s.lineRemoved}</span>{editable && <button className="lnk" onClick={() => void save(index, original)}>{s.revert}</button>}</span>
              : <span className={`words${editable ? "" : " ro"}`} contentEditable={editable ? "plaintext-only" : undefined} suppressContentEditableWarning spellCheck={false}
                  role={editable ? "textbox" : undefined} aria-label={editable ? `${s.lineAt} ${line.time}, ${line.speaker}` : undefined}
                  dangerouslySetInnerHTML={{__html: esc(line.text)}}
                  onFocus={() => setFocused(index)}
                  onBlur={event => { setFocused(-1); void save(index, (event.currentTarget.textContent ?? "").replace(/\s+/g, " ").trim()); }}
                  onPaste={event => { event.preventDefault(); document.execCommand("insertText", false, event.clipboardData.getData("text/plain").replace(/\s+/g, " ")); }}
                  onDrop={event => event.preventDefault()}
                  onKeyDown={event => {
                    const el = event.currentTarget;
                    if (event.key === "Enter" && event.altKey) { event.preventDefault(); if (line.ms !== null) onReplay?.(line.ms, next ?? line.ms + 8000); return; }
                    if (event.key === "Enter") { event.preventDefault(); el.blur(); const w = words(index + 1); if (w) caretAt(w, true); return; }
                    if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); const back = handBack(box.current); el.blur(); back(); }
                  }}/>}
            <span className="tx-end">{line.marks.map(mark => <span className="flag" key={mark} title={`${s.marked} ${elapsedLabel(mark)}`}>⚑ {s.mark}</span>)}
              {edited && !removed && <span className="edw"><button className="edot" aria-label={`${s.lineEdited} ${original}`}/><span className="epop" role="tooltip"><span>{s.lineEdited} <q>{original}</q></span>{editable && <button className="doc-action" onMouseDown={event => event.preventDefault()} onClick={() => void save(index, original)}>{s.revert}</button>}</span></span>}
            </span>
          </span>
        </div>;
      })}
    </div>
    <p className="tip thint">{locked ? s.lockedHint : mac() ? s.transcriptHintMac : s.transcriptHintWin}</p>
  </div>;
}
