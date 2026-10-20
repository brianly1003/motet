import MarkdownIt from "markdown-it";
const md = new MarkdownIt({ html: false, linkify: false, typographer: false });
// Imported/generated text cannot load remote images or navigate the webview.
md.renderer.rules.image = (tokens, index) => md.utils.escapeHtml(tokens[index].content);
md.renderer.rules.link_open = () => "<span class=\"note-link\">";
md.renderer.rules.link_close = () => "</span>";
md.renderer.rules.text = (tokens, index, _options, env) => {
  const text = tokens[index].content;
  const times: number[] = env?.sourceTimes ?? [];
  return text.split(/(\[\d+:\d{2}\])/g).map(part => {
    const match = /^\[(\d+):(\d{2})\]$/.exec(part);
    const ms = match ? (Number(match[1])*60+Number(match[2]))*1000 : -1;
    return match && times.includes(ms) ? `<button type="button" class="src" data-ms="${ms}" aria-label="Play from ${match[1]}:${match[2]}" title="Play from ${match[1]}:${match[2]}">${match[1]}:${match[2]}</button>` : md.utils.escapeHtml(part);
  }).join('');
};
export function renderNotes(notes: string, sourceTimes: number[] = []) {
  // Same as document.ts withoutTranscript (kept import-free for the node tests).
  return md.render(notes.replace(/<details><summary>Transcript<\/summary>[\s\S]*?(<\/details>|$)/, ""), {sourceTimes});
}
// One line (an action item) without a wrapping paragraph; same escaping as the notes.
export function renderInline(text: string) {
  return md.renderInline(text, {sourceTimes: []});
}
// Transcript files label the mic "Me" (CLI contract); people read "You".
export function displayTranscript(text: string) {
  return text.replace(/^(\[[0-9:]+\] )Me:/gm, "$1You:");
}
