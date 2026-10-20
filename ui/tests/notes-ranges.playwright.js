// Block ranges (SPEC 7.11): the last bullet before the hidden transcript copy is editable, and an
// edit never rewrites the copy or a neighbouring task that Markdown counts as a trailing blank line.
async (page) => {
  const p = await page.context().newPage(); const errs = []; p.on('pageerror', e => errs.push(e.message));
  await p.setViewportSize({ width: 1046, height: 662 });
  await p.addInitScript(() => {
    const summary = { id: '1', title: 'Teams test call', created_ms: Date.now(), status: 'Ready', marks_ms: [], duration_ms: 45000, notes_ms: 16000, spellings: [], locked: false };
    const meeting = { summary, transcript: '[00:01] Me: hi', transcript_heard: '', notes_md: '### Action items\n- None\n- [ ] Kevin: Keep this task\n\n### Call Quality Test\n- First point.\n- The system mentioned that the user should check their device settings.\n\n<details><summary>Transcript</summary>\n\n[00:01] Me: hi\n\n</details>\n\n### My notes\n\n[00:01] my own line' };
    let next = 0; window.meeting = meeting;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = { convertFileSrc: x => x, transformCallback: () => ++next, unregisterCallback: () => {}, invoke: async (c, a) => {
      if (c === 'plugin:event|listen') return next; if (c === 'get_state') return { state: { type: 'Idle' }, meeting_id: null, companion: { visible: false, size: 'Pill' }, elapsed_ms: 0, quit_pending: false };
      if (c === 'list_meetings') return [summary]; if (c === 'deleted_meetings') return []; if (c === 'get_meeting') return meeting;
      if (c === 'meeting_media') return { tracks: [], duration_ms: 45000, folder: 'x' };
      if (c === 'save_transcript_line') return meeting.transcript = '[00:01] Me: ' + a.text;
      if (c === 'save_notes') { if (a.expected !== meeting.notes_md) throw Error('Stale notes'); meeting.notes_md = a.markdown; return a.markdown; } return false; } };
  });
  await p.goto('http://127.0.0.1:3000/main/');
  await p.getByRole('button', { name: /Teams test call/ }).first().click();
  const failures = [];
  const endOf = sel => p.locator(sel).first().evaluate(e => { const w = document.createTreeWalker(e, NodeFilter.SHOW_TEXT); let n, last; while ((n = w.nextNode())) if (n.nodeValue.trim()) last = n; const i = last.nodeValue.trimEnd().length, r = document.createRange(); r.setStart(last, i - 1); r.setEnd(last, i); const b = r.getBoundingClientRect(); return { x: b.right - 1, y: b.y + b.height / 2 }; });
  const last = '.notes li[data-lines]:has-text("device settings")';
  await p.locator(last).waitFor();
  if (await p.locator(last).getAttribute('data-editable') !== '1') failures.push('last bullet before the transcript copy is read-only');
  let at = await endOf(last); await p.mouse.click(at.x, at.y); await p.keyboard.type(' Done'); await p.keyboard.press('Escape'); await p.waitForTimeout(250);
  let md = await p.evaluate(() => window.meeting.notes_md);
  if (!md.includes('check their device settings. Done\n')) failures.push('last bullet not saved');
  if (!md.includes('<details><summary>Transcript</summary>\n\n[00:01] Me: hi\n\n</details>\n\n### My notes\n\n[00:01] my own line')) failures.push('transcript copy or My notes changed');
  const none = '.notes li[data-lines]:has-text("None")';
  at = await endOf(none); await p.mouse.click(at.x, at.y); await p.keyboard.press('Meta+a'); await p.keyboard.type('Nothing else yet'); await p.keyboard.press('Escape'); await p.waitForTimeout(250);
  md = await p.evaluate(() => window.meeting.notes_md);
  if (!md.startsWith('### Action items\n- Nothing else yet\n- [ ] Kevin: Keep this task\n')) failures.push(`neighbouring task touched: ${JSON.stringify(md.slice(0, 80))}`);
  // Esc hands focus back to the notes without scrolling them (clicking away never did).
  await p.setViewportSize({ width: 1046, height: 420 });
  const scroller = () => p.evaluate(() => { const n = document.querySelector('.notes'); let e = n.parentElement; while (e && e.scrollHeight <= e.clientHeight) e = e.parentElement; return e ? e.scrollTop : document.scrollingElement.scrollTop; });
  await p.locator(last).scrollIntoViewIfNeeded();
  const before = await scroller();
  // WebKit scrolls a focused element's top into view; Chromium doesn't when it is partly visible,
  // so record how the container is focused as well as the scroll position.
  await p.evaluate(() => { window.scrollingFocus = 0; const focus = HTMLElement.prototype.focus; HTMLElement.prototype.focus = function (options) { if (this.matches('.notes, .tlist') && (!options?.preventScroll || getSelection().rangeCount)) window.scrollingFocus++; return focus.call(this, options); }; });
  at = await endOf(last); await p.mouse.click(at.x, at.y); await p.keyboard.type('!'); await p.keyboard.press('Escape'); await p.waitForTimeout(250);
  const after = await scroller();
  if (before === 0 || after !== before) failures.push(`Esc scrolled the notes: ${before} -> ${after}`);
  if (await p.evaluate(() => window.scrollingFocus)) failures.push('Esc focuses the notes in a way that scrolls, or with the old caret still placed (WebKit jumps)');
  // The same for a transcript line.
  await p.getByRole('tab', { name: 'Transcript' }).click(); await p.locator('.tline .words').first().click(); await p.keyboard.type('!'); await p.keyboard.press('Escape'); await p.waitForTimeout(250);
  if (await p.evaluate(() => window.scrollingFocus)) failures.push('Esc focuses the transcript in a way that scrolls');
  if (!(await p.evaluate(() => document.activeElement.matches('.tlist')))) failures.push('Esc did not hand focus to the transcript');
  await p.close();
  if (failures.length || errs.length) throw Error(JSON.stringify({ failures, errs }));
  return 'ok';
}
