"use client";
import { useEffect, useState } from "react";
import { getLevels } from "@/lib/ipc";

export function Duet() {
  const [levels, setLevels] = useState<[number, number]>([0, 0]);
  const [phase, setPhase] = useState(0);
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function tick() {
      try { const next = await getLevels(); if (active) { setLevels(next); setPhase(matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : performance.now() / 200); } }
      catch { if (active) setLevels([0, 0]); }
      if (active) timer = setTimeout(tick, 100);
    }
    void tick();
    return () => { active = false; clearTimeout(timer); };
  }, []);
  const strand = (level: number, offset: number) => Array.from({ length: 29 }, (_, i) => {
    const x = i * 4;
    const amplitude = Math.min(1, Math.max(0, level) * 5);
    return `${i ? "L" : "M"}${x} ${(13 + amplitude * 11 * Math.sin(x / 9 + phase + offset) * Math.sin(Math.PI * x / 112)).toFixed(1)}`;
  }).join("");
  return <div className="duet" role="img" aria-label={`You: ${levels[0] > .02 ? "speaking" : "quiet"}, Them: ${levels[1] > .02 ? "speaking" : "quiet"}`}><span className="lm">You</span><svg viewBox="0 0 112 26" aria-hidden="true"><path className="strand them" d={strand(levels[1], Math.PI)} /><path className="strand me" d={strand(levels[0], 0)} /></svg><span className="lt">Them</span></div>;
}
