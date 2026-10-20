// People for the @ picker and the owner pill (SPEC 7.11). Kept import-free for the node tests.

// Names that match the query on any word's start, case-insensitive. The typed name is offered as a
// new person only when nothing matches, so "@Ke" lists Kevin and nothing else.
export function matchPeople(names: string[], query: string) {
  const unique = [...new Set(names)], q = query.trim().toLowerCase();
  const hits = unique.filter(name => !q || name.toLowerCase().split(/\s+/).some(word => word.startsWith(q)) || name.toLowerCase().startsWith(q));
  const typed = query.replace(/[:[\]*`<\n_]/g, " ").replace(/\s+/g, " ").trim().slice(0, 40);
  return hits.length || !typed ? hits : [typed];
}

// A correction looks or sounds like what was there ("Kenvin" → "Kevin", "Jen Lee Ang" →
// "Jianliang"); a different person ("Kevin" → "Linh") doesn't. Same rule as Fix all.
export function alike(a: string, b: string) {
  const g = (x: string) => x.toLowerCase().replace(/[^\p{L}\p{N}]/gu, ""), x = g(a), y = g(b);
  const d = Array.from({length: y.length + 1}, (_, j) => j);
  for (let i = 1; i <= x.length; i++) {
    let prev = d[0]; d[0] = i;
    for (let j = 1; j <= y.length; j++) { const t = d[j]; d[j] = Math.min(d[j] + 1, d[j - 1] + 1, prev + (x[i - 1] === y[j - 1] ? 0 : 1)); prev = t; }
  }
  const skel = (z: string) => z.normalize("NFD").replace(/[aeiouỳ-ͯ]/g, "").replace(/(.)\1+/g, "$1");
  return d[y.length] <= 0.4 * Math.max(x.length, y.length) || (skel(x).length > 1 && skel(x) === skel(y));
}

// My notes keep an owner as one word: "@Linh_Tran" (summarize.rs reads "_" as a space).
export const mentionWord = (name: string) => `@${name.trim().replace(/\s+/g, "_")}`;
