// A time cited in the notes opens the Transcript with that line at the top, so the lines that play
// next show below it (owner 2026-10-21: the 03:49 line was at the bottom edge, behind the player).
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 1200, height: 720 });
  await p.addInitScript(() => {
    const summary = { id: '1', title: 'Seek', created_ms: Date.now(), status: 'Ready', marks_ms: [], duration_ms: 300000, notes_ms: 8000, spellings: [], locked: false, title_source: 'placeholder' };
    const time = s => `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
    const transcript = Array.from({ length: 40 }, (_, i) => `[${time(i * 7)}] Line number ${i}`).join('\n');
    const meeting = { summary, transcript, transcript_heard: '', notes_md: `### Action items\n- None\n### Topic\n- Late point [${time(30 * 7)}]\n` };
    let next = 0;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = { convertFileSrc: x => x, transformCallback: () => ++next, unregisterCallback: () => {}, invoke: async (c) => {
      if (c === 'plugin:event|listen') return next; if (c === 'get_state') return { state: { type: 'Idle' }, meeting_id: null, companion: { visible: false, size: 'Pill' }, elapsed_ms: 0, quit_pending: false };
      if (c === 'list_meetings') return [summary]; if (c === 'deleted_meetings') return []; if (c === 'get_meeting') return meeting;
      if (c === 'meeting_media') return { tracks: [], duration_ms: 300000, folder: 'x' }; return false; } };
  });
  await p.goto('http://127.0.0.1:3000/main/');
  await p.getByRole('button', { name: /Seek/ }).first().click();
  await p.getByText('03:30', { exact: true }).first().click();
  const line = p.locator('.tline.playing');
  await line.waitFor();
  const text = await line.innerText();
  if (!/Line number 30/.test(text)) fails.push('cited time highlights the wrong line: ' + text);
  // The player appears with the jump; give layout a moment to settle before measuring.
  let lineBox, viewBox;
  // One line (34 px) still shows above it, so it is clear there is more to scroll back to.
  const atTop = () => lineBox && viewBox && lineBox.y - viewBox.y >= 34 && lineBox.y - viewBox.y <= 60;
  for (let i = 0; i < 20 && !atTop(); i++) {
    await p.waitForTimeout(100);
    lineBox = await line.boundingBox(); viewBox = await p.locator('.docbody').boundingBox();
  }
  if (!atTop()) fails.push(`cited line not at the top: line ${JSON.stringify(lineBox)}, view ${JSON.stringify(viewBox)}`);
  const prev = await p.locator('.tline[data-line="29"]').boundingBox();
  if (!prev || prev.y < viewBox.y - 1) fails.push('the line before the cited one is not fully visible');
  const next = await p.locator('.tline[data-line="33"]').boundingBox(), player = await p.locator('.player').boundingBox();
  if (!next || next.y + next.height > player.y) fails.push('the lines that play next are not visible below the cited one');
  // A line inside the view does not move when clicked.
  const row = p.locator('.tline[data-line="32"]'), before = (await row.boundingBox()).y;
  await row.locator('button.ts').click(); await p.waitForTimeout(300);
  if (Math.abs((await row.boundingBox()).y - before) > 1) fails.push('clicking a visible line scrolled the transcript');
  await p.close();
  return fails.length ? fails : 'PASS';
}
