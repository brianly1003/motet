// Transcript keyboard (owner 2026-10-20): ↑↓ move between lines, Space plays or pauses, ↩ edits the
// line, ↑↓ at a line's edge carry on into the next line while editing, Esc leaves the line selected.
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 1200, height: 720 });
  await p.addInitScript(() => {
    const summary = { id: '1', title: 'Keys', created_ms: Date.now(), status: 'Ready', marks_ms: [], duration_ms: 60000, notes_ms: 8000, spellings: [], locked: false, title_source: 'placeholder' };
    const meeting = { summary, transcript: '[00:04] Me: Alo Alo Alo\n[00:20] Them: Rồi nghe rồi\n[00:23] Them: Rồi nghe rồi nghe rồi\n[00:40] Me: Okay, thanks', transcript_heard: '', notes_md: '### Action items\n- None\n' };
    let next = 0;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = { convertFileSrc: x => x, transformCallback: () => ++next, unregisterCallback: () => {}, invoke: async (c, a) => {
      if (c === 'plugin:event|listen') return next; if (c === 'get_state') return { state: { type: 'Idle' }, meeting_id: null, companion: { visible: false, size: 'Pill' }, elapsed_ms: 0, quit_pending: false };
      if (c === 'list_meetings') return [summary]; if (c === 'deleted_meetings') return []; if (c === 'get_meeting') return meeting;
      if (c === 'save_transcript_line') return meeting.transcript;
      if (c === 'meeting_media') return { tracks: [], duration_ms: 60000, folder: 'x' }; return false; } };
  });
  await p.goto('http://127.0.0.1:3000/main/');
  await p.getByRole('button', { name: /Keys/ }).first().click();
  await p.getByRole('tab', { name: 'Transcript' }).click();
  await p.locator('.tline').first().waitFor();
  const tab=p.getByRole('tab',{name:'Transcript',exact:true});
  await tab.focus();await p.keyboard.press(' ');
  if(!/Pause/.test(await p.locator('.player .doc-action').innerText()))fails.push('Space on Transcript tab does not play');
  await p.keyboard.press(' ');
  if(!/Play/.test(await p.locator('.player .doc-action').innerText()))fails.push('Space on Transcript tab does not pause');
  const focus=await tab.evaluate(e=>({outline:getComputedStyle(e).outlineStyle,shadow:getComputedStyle(e).boxShadow}));
  if(focus.outline!=='none'||focus.shadow!=='none')fails.push('Transcript tab has a focus box: '+JSON.stringify(focus));
  const on = () => p.evaluate(() => { const a = document.activeElement; return a?.closest('.tline') ? `${a.classList.contains('words') ? 'edit' : 'row'}:${a.closest('.tline').dataset.line}` : a?.tagName; });
  const player = () => p.locator('.player .doc-action').innerText();
  await tab.click();
  await p.keyboard.press('ArrowDown'); if (await on() !== 'row:0') fails.push('↓ from clicked Transcript tab: ' + await on());
  await p.keyboard.press('ArrowDown'); if (await on() !== 'row:1') fails.push('↓ after entering from tab: ' + await on());
  await p.keyboard.press('ArrowUp'); if (await on() !== 'row:0') fails.push('↑ after entering from tab: ' + await on());
  for (let i = 0; i < 6; i++) await p.keyboard.press('ArrowDown');
  if (await on() !== 'row:3') fails.push('↓ past the bottom: ' + await on());
  await tab.click();await p.keyboard.press('ArrowUp');
  if (await on() !== 'row:0') fails.push('↑ from clicked Transcript tab: ' + await on());
  await tab.click();await p.keyboard.press('ArrowRight');
  if (await p.getByRole('tab',{name:'Audio',exact:true}).getAttribute('aria-selected') !== 'true') fails.push('→ no longer switches tabs');
  await p.keyboard.press('ArrowLeft');
  if (await tab.getAttribute('aria-selected') !== 'true') fails.push('← no longer switches tabs');
  await p.evaluate(() => document.activeElement?.blur());
  await p.keyboard.press('ArrowDown'); if (await on() !== 'row:0') fails.push('↓ from nothing: ' + await on());
  await p.keyboard.press('ArrowDown'); await p.keyboard.press('ArrowDown'); if (await on() !== 'row:2') fails.push('↓↓: ' + await on());
  await p.keyboard.press('ArrowUp'); if (await on() !== 'row:1') fails.push('↑: ' + await on());
  for (let i = 0; i < 5; i++) await p.keyboard.press('ArrowUp'); if (await on() !== 'row:0') fails.push('↑ past the top: ' + await on());
  await p.waitForTimeout(200); // the row's background fades in over .12 s
  if (!(await p.locator('.tline[data-line="0"]').evaluate(e => getComputedStyle(e).backgroundColor !== 'rgba(0, 0, 0, 0)'))) fails.push('the keyboard line has no highlight');
  const before = await player();
  await p.keyboard.press(' '); const playing = await player();
  await p.keyboard.press(' '); const paused = await player();
  if (!/Play/.test(before) || !/Pause/.test(playing) || !/Play/.test(paused)) fails.push(`Space: ${before} -> ${playing} -> ${paused}`);
  await p.keyboard.press('Enter'); if (await on() !== 'edit:0') fails.push('↩ edits: ' + await on());
  await p.keyboard.press('ArrowDown'); if (await on() !== 'edit:1') fails.push('↓ at the edge while editing: ' + await on());
  await p.keyboard.press('ArrowUp'); if (await on() !== 'edit:0') fails.push('↑ at the edge while editing: ' + await on());
  // Space while editing types; it never toggles the audio.
  await p.keyboard.press(' '); if (!/Play/.test(await player())) fails.push('Space while editing played the audio');
  await p.keyboard.press('Backspace');
  await p.keyboard.press('Escape'); if (await on() !== 'row:0') fails.push('Esc: ' + await on());
  await p.keyboard.press('ArrowDown'); if (await on() !== 'row:1') fails.push('↓ after Esc: ' + await on());
  await p.getByRole('button',{name:'Play from 00:23',exact:true}).click();
  await tab.click();await p.keyboard.press('ArrowDown');
  if (await on() !== 'row:2') fails.push('↓ from tab did not enter current playback line: ' + await on());
  await p.close();
  if (fails.length) throw Error(JSON.stringify(fails));
  return 'ok';
}
