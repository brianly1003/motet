import MarkdownIt from "markdown-it";
const md = new MarkdownIt({ html: false, linkify: false, typographer: false });
// Imported/generated text cannot load remote images or navigate the webview.
md.renderer.rules.image = (tokens, index) => md.utils.escapeHtml(tokens[index].content);
md.renderer.rules.link_open = () => "<span class=\"note-link\">";
md.renderer.rules.link_close = () => "</span>";
// Note syntax (SPEC 7.11): any number of [m:ss] per line becomes a chip when it falls within the
// meeting; with no audio (durationMs 0) times stay text.
export function timeMs(m: string, ss: string, durationMs: number) {
  const ms = (Number(m)*60+Number(ss))*1000;
  return Number(ss) < 60 && ms <= durationMs ? ms : -1;
}
export function timeChip(m: string, ss: string, ms: number) {
  const label = `${m.padStart(2, "0")}:${ss}`;
  return `<button type="button" class="src" data-ms="${ms}" aria-label="Play from ${label}" title="Play from ${label}">${label}</button>`;
}
md.renderer.rules.text = (tokens, index, _options, env) => {
  const text = tokens[index].content;
  return text.split(/(\[\d+:\d{2}\])/g).map(part => {
    const match = /^\[(\d+):(\d{2})\]$/.exec(part);
    const ms = match ? timeMs(match[1], match[2], env?.durationMs ?? 0) : -1;
    return match && ms >= 0 ? timeChip(match[1], match[2], ms) : md.utils.escapeHtml(part);
  }).join('');
};
// "[02:05] Point" -> "Point [02:05]": models sometimes lead with the time; the chip goes at the end.
md.core.ruler.push("motetto_time_last", state => {
  for (const token of state.tokens) {
    const kids = token.children;
    const lead = kids?.[0]?.type === "text" ? /^\[\d+:\d{2}\]\s*/.exec(kids[0].content) : null;
    if (!kids || !lead || kids[0].content.length === lead[0].length && kids.length === 1) continue;
    kids[0].content = kids[0].content.slice(lead[0].length);
    const tail = new state.Token("text", "", 0);
    tail.content = ` ${lead[0].trim()}`;
    kids.push(tail);
  }
});
// Editable notes: each block carries the notes.md line range it came from ("start,end", end
// exclusive) so an edit replaces exactly those lines.
md.core.ruler.push("motetto_lines", state => {
  const offset = state.env?.lineOffset;
  if (offset === undefined) return;
  for (const token of state.tokens) {
    if (!token.map || !["paragraph_open", "list_item_open", "heading_open"].includes(token.type)) continue;
    if (token.type === "paragraph_open" && token.level > 0) continue;
    token.attrSet("data-lines", `${token.map[0]+offset},${token.map[1]+offset}`);
  }
});
export function renderNotes(notes: string, durationMs = 0, lineOffset?: number) {
  // Same as document.ts withoutTranscript (kept import-free for the node tests).
  return md.render(notes.replace(/<details><summary>Transcript<\/summary>[\s\S]*?(<\/details>|$)/, ""), {durationMs, lineOffset});
}
// One line (an action item) without a wrapping paragraph; same escaping as the notes.
export function renderInline(text: string, durationMs = 0) {
  return md.renderInline(text, {durationMs});
}
// Transcript files label the mic "Me" (CLI contract); people read "You".
export function displayTranscript(text: string) {
  return text.replace(/^(\[[0-9:]+\] )Me:/gm, "$1You:");
}
