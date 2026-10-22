"use client";
import { type RefObject, useEffect, useLayoutEffect, useState } from "react";
import { strings as s } from "@/lib/strings";

/** Within this many pixels of the end still counts as reading the newest line. */
const NEAR_END = 48;

/**
 * Live lines follow the newest one only while the reader is at the end: scrolling up stops it and
 * shows Jump to now (DESIGN 4.4). `box` is the scrolling element; showing the pane again follows.
 */
export function useFollow(box: RefObject<HTMLElement | null>, shown: boolean) {
  const [behind, setBehind] = useState(false);
  useEffect(() => { if (shown) setBehind(false); }, [shown]);
  // Every render while following: new lines, a resized pane and a state change all keep the end in view.
  useLayoutEffect(() => { const element = box.current; if (shown && element && !behind) element.scrollTop = element.scrollHeight; });
  const onScroll = () => { const element = box.current; if (element) setBehind(element.scrollHeight - element.scrollTop - element.clientHeight > NEAR_END); };
  return { behind: shown && behind, onScroll, jump: () => setBehind(false) };
}

export function JumpToNow({ show, onJump }: { show: boolean; onJump: () => void }) {
  return show ? <button type="button" className="jumpnow" onClick={onJump}>
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M8 3v9m-4-4 4 4 4-4"/></svg>{s.jumpToNow}
  </button> : null;
}
