"use client";
import { useEffect, useRef, useState } from "react";
import * as ipc from "@/lib/ipc";
import { elapsedLabel } from "@/lib/session";

export function MyNotes({ meetingId, elapsed, registerFlush }: { meetingId: string; elapsed: number; registerFlush: (flush: () => Promise<void>) => void }) {
  const editor = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState("Loading…");
  const [ready, setReady] = useState(false);
  const version = useRef(0);
  const saved = useRef("");
  const draft = useRef("");
  const loaded = useRef(false);
  const finalFlush = useRef<() => Promise<void>>(async () => {});
  const queue = useRef(Promise.resolve());
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  function content() {
    return Array.from(editor.current?.children ?? []).map(row => `[${(row as HTMLElement).dataset.t || elapsedLabel(elapsed)}] ${row.textContent ?? ""}`).join("\n");
  }
  function flush() {
    clearTimeout(timer.current);
    queue.current = queue.current.catch(() => {}).then(async () => {
      if (!loaded.current) return;
      const value = draft.current;
      if (value === saved.current) return;
      setStatus("Saving…");
      try {
        const result = await ipc.saveMyNotes(meetingId, value, version.current);
        version.current = result.version;
        saved.current = value;
        setStatus("Saved");
      } catch (error) { setStatus(String(error)); throw error; }
    });
    return queue.current;
  }
  useEffect(() => { finalFlush.current = flush; registerFlush(flush); });
  useEffect(() => {
    let active = true;
    ipc.getMyNotes(meetingId).then(notes => {
      if (!active || !editor.current) return;
      version.current = notes.version;
      editor.current.replaceChildren(...(notes.text.split("\n").filter(Boolean).length ? notes.text.split("\n").filter(Boolean) : [""]).map(line => {
        const row = document.createElement("div");
        const match = /^\[(\d+:\d+)\] ?(.*)$/.exec(line);
        row.dataset.t = match?.[1] ?? elapsedLabel(elapsed);
        row.textContent = match?.[2] ?? line;
        if (!row.textContent) row.append(document.createElement("br"));
        decorate(row);
        return row;
      }));
      saved.current = content(); draft.current = saved.current; loaded.current = true;
      setReady(true); setStatus("Saved");
    }).catch(error => { if (active) setStatus(String(error)); });
    return () => { active = false; clearTimeout(timer.current); void finalFlush.current().catch(() => {}); };
    // A meeting owns the editor. Clock ticks must not replace an unsaved draft.
  }, [meetingId]);
  function decorate(row: HTMLElement) {
    const text = row.textContent ?? "";
    row.className = text.startsWith("[]") ? "act" : text.startsWith("!") ? "dec" : text.startsWith("?") ? "q" : text.startsWith("#") ? "h" : "";
  }
  function newLine() {
    document.execCommand("insertParagraph");
    const selection = window.getSelection();
    const element = selection?.anchorNode?.nodeType === Node.ELEMENT_NODE ? selection.anchorNode as HTMLElement : selection?.anchorNode?.parentElement;
    const row = element?.closest(".editor > div") as HTMLElement | null;
    if (row) row.dataset.t = elapsedLabel(elapsed);
    changed();
  }
  function changed() {
    for (const child of Array.from(editor.current?.children ?? [])) {
      const row = child as HTMLElement;
      if (!row.dataset.t) row.dataset.t = elapsedLabel(elapsed);
      decorate(row);
    }
    draft.current = content();
    setStatus("Saving…"); clearTimeout(timer.current);
    timer.current = setTimeout(() => { void flush().catch(() => {}); }, 300);
  }
  return <><div className="panehead mine"><span>My notes</span><span className="tip" role="status">{status}</span></div><div ref={editor} className="editor" contentEditable={ready} suppressContentEditableWarning spellCheck={false} role="textbox" aria-multiline="true" aria-label="My notes. Each line keeps the time you wrote it." onInput={changed} onKeyDown={event => { if (event.key === "Enter") { event.preventDefault(); newLine(); } }} onBlur={() => { void flush().catch(() => {}); }} onPaste={event => {
    event.preventDefault();
    event.clipboardData.getData("text/plain").split(/\r?\n/).forEach((line, index) => { if (index) newLine(); document.execCommand("insertText", false, line); });
    changed();
  }} /><div className="hints"><code>[]</code> action <code>!</code> decision <code>?</code> question <code>#</code> topic <span>· <kbd>Esc</kbd> back to the call</span></div></>;
}
