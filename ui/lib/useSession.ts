"use client";
import { useEffect, useRef, useState } from "react";
import * as ipc from "@/lib/ipc";
import type { AppState } from "@/lib/bindings/AppState";
import type { Stage } from "@/lib/bindings/Stage";

export function useSession() {
  const [state, setState] = useState<AppState | null>(null);
  const [stage, setStage] = useState<Stage | null>(null);
  const [notice, setNotice] = useState("");
  const [pending, setPending] = useState(false);
  const meetingId = useRef<string | null>(null);
  useEffect(() => {
    if (!ipc.desktopAvailable()) return;
    let active = true;
    let version = 0;
    const cleanups: Array<() => void> = [];
    const retain = (off: () => void) => active ? cleanups.push(off) : off();
    (async () => {
      retain(await ipc.onSessionEvent(event => {
        if (!active) return;
        if (event.type === "StateChanged") { version++; meetingId.current = event.meeting_id; setState(event); if (event.state.type !== "Finishing") setStage(null); }
        else if (event.type === "Stage" && event.meeting_id === meetingId.current) setStage(event.stage);
      }));
      retain(await ipc.onNotice(text => { if (active) setNotice(text); }));
      const before = version;
      const current = await ipc.getState();
      if (active && version === before) { meetingId.current = current.meeting_id; setState(current); }
    })().catch(e => { if (active) setNotice(String(e)); });
    return () => { active = false; cleanups.forEach(off => off()); };
  }, []);
  async function act(command: () => Promise<void>) {
    setPending(true); setNotice("");
    try { await command(); } catch (e) { setNotice(String(e)); }
    finally { setPending(false); }
  }
  return { state, stage, notice, setNotice, pending, act };
}
