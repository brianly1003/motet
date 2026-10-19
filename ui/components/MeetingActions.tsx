"use client";
import { useEffect, useRef, useState } from "react";
import type { Meeting } from "@/lib/bindings/Meeting";
import { meetingAction } from "@/lib/ipc";
import { renderNotes } from "@/lib/md";
import { strings as s } from "@/lib/strings";

export default function MeetingActions({ meeting, onChanged, onError }: { meeting: Meeting; onChanged: (deleted: boolean) => Promise<void>; onError: (message: string) => void }) {
  const [menu, setMenu] = useState<"copy" | "more" | null>(null);
  const [confirm, setConfirm] = useState<"delete" | "regenerate" | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const close = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) setMenu(null); };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, []);
  useEffect(() => {
    const key = (event: KeyboardEvent) => { if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === "c") { event.preventDefault(); void copy("items"); } };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  }, [meeting.notes_md]);
  async function copy(kind: "items" | "markdown" | "plain") {
    try {
      let text = meeting.notes_md.split("<details><summary>Transcript</summary>")[0].trim();
      if (kind === "items") text = text.match(/^#{1,6}\s+Action items\s*\n([\s\S]*?)(?=^#{1,6}\s|$(?![\s\S]))/im)?.[1]?.trim() ?? "";
      if (kind !== "markdown") {
        const document = new DOMParser().parseFromString(renderNotes(text), "text/html");
        document.querySelectorAll("li, p, h1, h2, h3, h4, br").forEach(node => node.append("\n"));
        text = document.body.textContent?.trim() ?? "";
      }
      await navigator.clipboard.writeText(text); setMessage(s.copied); setMenu(null);
    } catch (error) { onError(String(error)); }
  }
  async function run(action: "reveal" | "regenerate" | "delete") {
    setBusy(true); setMenu(null);
    try { await meetingAction(meeting.summary.id, action); setConfirm(null); if (action !== "reveal") await onChanged(action === "delete"); }
    catch (error) { onError(String(error)); }
    finally { setBusy(false); }
  }
  return <div className="headacts" ref={root} onKeyDown={event => {
    if (event.key === "Escape") { setMenu(null); root.current?.querySelector<HTMLButtonElement>(`[data-menu="${menu}"]`)?.focus(); }
    if (menu && ["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault(); const items = Array.from(root.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? []);
      const index = items.indexOf(document.activeElement as HTMLButtonElement);
      items[event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : (index + (event.key === "ArrowUp" ? -1 : 1) + items.length) % items.length]?.focus();
    }
  }}>
    <button className="doc-action" data-menu="copy" aria-haspopup="menu" aria-expanded={menu === "copy"} disabled={busy} onClick={() => setMenu(menu === "copy" ? null : "copy")}>{s.copyMenu}</button>
    <button className="doc-action ghost" data-menu="more" aria-label={s.moreActions} aria-haspopup="menu" aria-expanded={menu === "more"} disabled={busy} onClick={() => setMenu(menu === "more" ? null : "more")}>•••</button>
    {menu && <div className="doc-dropdown" role="menu" aria-label={menu === "copy" ? "Copy" : s.moreActions}>
      {menu === "copy" ? <><button role="menuitem" onClick={() => copy("items")}><span>{s.copyItems}</span><kbd>⇧⌘C</kbd></button><button role="menuitem" onClick={() => copy("markdown")}>{s.copyMarkdown}</button><button role="menuitem" onClick={() => copy("plain")}>{s.copyPlain}</button></> : <><button role="menuitem" onClick={() => { setConfirm("regenerate"); setMenu(null); }}>{s.regenerate}</button><button role="menuitem" onClick={() => run("reveal")}>{s.reveal}</button><div className="sep"/><button role="menuitem" onClick={() => { setConfirm("delete"); setMenu(null); }}>{s.deleteMeeting}</button></>}
    </div>}
    {confirm && <div className="doc-confirm banner" role="alertdialog" aria-label={confirm === "delete" ? s.deleteMeeting : s.regenerate}><p>{confirm === "delete" ? s.deleteConfirm : s.regenerateConfirm}</p><button className="doc-action" disabled={busy} onClick={() => run(confirm)}>{busy ? s.processing : s.confirm}</button><button className="doc-action ghost" disabled={busy} onClick={() => setConfirm(null)}>{s.cancel}</button></div>}
    <span className="sr-only" role="status">{message}</span>
  </div>;
}
