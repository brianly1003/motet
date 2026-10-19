import { transcriptLines } from "@/lib/transcript";
import { elapsedLabel } from "@/lib/session";
import { strings as s } from "@/lib/strings";
export default function Transcript({text, marks, onSeek}: {text: string; marks: number[]; onSeek?:(ms:number)=>void}) {
  return <div>{transcriptLines(text, marks).map((line, index) => <div className="tline" key={index}>
    <button className="ts" disabled={!onSeek||line.ms===null} onClick={()=>line.ms!==null&&onSeek?.(line.ms)} aria-label={`Play from ${line.time}`}>{line.time}</button><span className={`who ${line.speaker === "You" ? "me" : "them"}`}>{line.speaker}</span>
    <span>{line.text}{line.marks.map(mark => <span className="flag" key={mark} title={`${s.marked} ${elapsedLabel(mark)}`}> ⚑ {s.mark}</span>)}</span>
  </div>)}</div>;
}
