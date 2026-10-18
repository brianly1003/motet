import MarkdownIt from "markdown-it";
const md = new MarkdownIt({ html: false, linkify: false, typographer: false });
// Imported/generated text cannot load remote images or navigate the webview.
md.renderer.rules.image = (tokens, index) => md.utils.escapeHtml(tokens[index].content);
md.renderer.rules.link_open = () => "<span class=\"note-link\">";
md.renderer.rules.link_close = () => "</span>";
export function renderNotes(notes: string) {
  return md.render(notes.split("<details><summary>Transcript</summary>")[0]);
}
// Transcript files label the mic "Me" (CLI contract); people read "You".
export function displayTranscript(text: string) {
  return text.replace(/^(\[[0-9:]+\] )Me:/gm, "$1You:");
}
