import { strings as s } from "@/lib/strings";

// The @ picker's menu (SPEC 7.11): each row is the pill the task will get; a name nobody has yet
// says "new". Shared by the notes and the companion's My notes. The caller owns the keys.
export default function PeoplePicker({options, known, index, x, y, onPick}: {options: string[]; known: string[]; index: number; x: number; y: number; onPick: (name: string) => void}) {
  return <div className="mention gmenu" role="listbox" aria-label={s.assignTo} style={{left: x, top: y}}>
    {options.map((name, i) => <button type="button" role="option" key={name} aria-selected={i === index} onMouseDown={event => event.preventDefault()} onClick={() => onPick(name)}>
      <span className={`owner ${name === "You" ? "me" : ""}`}>{name}</span>{!known.includes(name) && <span className="tip">{s.newPerson}</span>}</button>)}
    {!options.length && <p className="tip">{s.typeAName}</p>}
  </div>;
}
