export function meetingGroup(time: number, now = new Date()) {
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const week = new Date(today);
  week.setDate(today.getDate() - (today.getDay() + 6) % 7);
  return time >= today.getTime() ? 'Today' : time >= week.getTime() ? 'This week' : 'Earlier';
}
export function meetingDate(time: number, now = new Date()) {
  const date = new Date(time);
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today); yesterday.setDate(today.getDate() - 1);
  const day = time >= today.getTime() ? 'Today' : time >= yesterday.getTime() ? 'Yesterday' : new Intl.DateTimeFormat(undefined, {month:'short', day:'numeric'}).format(date);
  return `${day} · ${new Intl.DateTimeFormat(undefined, {hour:'2-digit', minute:'2-digit', hour12:false}).format(date)}`;
}
export function timeLabel(ms: number) {
  const seconds = Math.floor(ms / 1000);
  return `${String(Math.floor(seconds / 60)).padStart(2,'0')}:${String(seconds % 60).padStart(2,'0')}`;
}
export function fileSize(bytes: number) {
  return bytes >= 1e9 ? `${(bytes/1e9).toFixed(1)} GB` : bytes >= 1e6 ? `${(bytes/1e6).toFixed(1)} MB` : `${(bytes/1e3).toFixed(1)} KB`;
}
export function noteSections(markdown: string) {
  const sections: {title:string; lines:{text:string; line:number}[]}[] = [];
  let section = {title:'', lines:[] as {text:string; line:number}[]};
  let fence = false;
  markdown.split('<details><summary>Transcript</summary>')[0].split('\n').forEach((text, line) => {
    if (/^\s*(```|~~~)/.test(text)) fence = !fence;
    const heading = !fence && /^#{1,6}\s+(.+?)\s*#*\s*$/.exec(text);
    if (heading) { if(section.title || section.lines.length) sections.push(section); section = {title:heading[1],lines:[]}; }
    else section.lines.push({text,line});
  });
  if(section.title || section.lines.length) sections.push(section);
  return sections.filter(s => s.title || s.lines.some(l=>l.text.trim()));
}
export function taskLine(text: string) {
  const match = /^\s*[-*] \[([ xX])\] (.*)$/.exec(text);
  if (!match) return null;
  const owner = /^(?:\*\*)?([^:*\[\]\n]{1,40})(?:\*\*)?:\s+(.*)$/.exec(match[2]);
  return {done: match[1] !== ' ', owner: owner?.[1] ?? '', text: owner?.[2] ?? match[2]};
}
