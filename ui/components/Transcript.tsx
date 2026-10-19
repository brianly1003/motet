import { useEffect, useRef } from "react";
import { transcriptLines } from "@/lib/transcript";
import { elapsedLabel } from "@/lib/session";
import { strings as s } from "@/lib/strings";
// As in the mockup: click a line to play from it; the line being heard is highlighted and kept in view.
export default function Transcript({text, marks, onSeek, position}: {text: string; marks: number[]; onSeek?:(ms:number)=>void; position?: number}) {
  const lines = transcriptLines(text, marks);
  const playing = position === undefined ? -1 : lines.findLastIndex(line => line.ms !== null && line.ms <= position);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => { if (playing >= 0) box.current?.children[playing]?.scrollIntoView({block: "nearest"}); }, [playing]);
  return <div ref={box}>{lines.map((line, index) => {
    const seek = onSeek && line.ms !== null ? () => onSeek(line.ms!) : undefined;
    return <div className={`tline${index === playing ? " playing" : ""}${seek ? " seekable" : ""}`} key={index} onClick={seek}>
      <button className="ts" disabled={!seek} onClick={event => { event.stopPropagation(); seek?.(); }} aria-label={`Play from ${line.time}`}>{line.time}</button><span className={`who ${line.speaker === "You" ? "me" : "them"}`}>{line.speaker}</span>
      <span>{line.text}{line.marks.map(mark => <span className="flag" key={mark} title={`${s.marked} ${elapsedLabel(mark)}`}> ⚑ {s.mark}</span>)}</span>
    </div>;
  })}</div>;
}
