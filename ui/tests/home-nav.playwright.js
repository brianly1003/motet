// The sidebar's Action items row (SPEC 7.7, DESIGN 4.7): back to Home from a meeting, current only
// on Home, and its count follows ticked items.
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 1200, height: 720 });
  const theme = 'app';
  await p.addInitScript(() => {
    const mk = (id, title, notes) => ({ summary: { id, title, created_ms: Date.now() - +id * 6e5, status: 'Ready', marks_ms: [], duration_ms: 87000, notes_ms: 48000, spellings: [], locked: false }, transcript: '[00:01] Me: hi', transcript_heard: '', notes_md: notes });
    const ms = [mk('1', 'Recorded meeting', '### Action items\n- [ ] Me: Send the budget proposal by **Friday** [02:15]\n- [ ] Anna: Review the draft [02:20]\n'), mk('2', 'Daily Meeting', '### Action items\n- [ ] Me: Brush my teeth [01:00]\n- [x] Me: Done thing\n')];
    let next = 0; window.ms = ms;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = { convertFileSrc: x => x, transformCallback: () => ++next, unregisterCallback: () => {}, invoke: async (c, a) => {
      if (c === 'plugin:event|listen') return next; if (c === 'get_state') return { state: { type: 'Idle' }, meeting_id: null, companion: { visible: false, size: 'Pill' }, elapsed_ms: 0, quit_pending: false };
      if (c === 'list_meetings') return ms.map(m => m.summary); if (c === 'deleted_meetings') return []; if (c === 'get_meeting') return ms.find(m => m.summary.id === a.id);
      if (c === 'open_action_items') return ms.map(m => [m.summary, m.notes_md]);
      if (c === 'set_action_item_done') { const m = ms.find(m => m.summary.id === a.id); const l = m.notes_md.split('\n'); l[a.line] = l[a.line].replace('[ ]', a.done ? '[x]' : '[ ]'); m.notes_md = l.join('\n'); return m.notes_md; }
      if (c === 'meeting_media') return { tracks: [], duration_ms: 87000, folder: 'x' }; if (c === 'library_bytes') return 1000; return false; } };
  });
  await p.goto('http://127.0.0.1:3000/main/');
  const row = p.locator('.homerow'); await row.waitFor();
  const n = async () => (await row.locator('.n').innerText()).trim();
  if (await row.getAttribute('aria-current') !== 'page') fails.push(theme + ': row not current on Home');
  if (await n() !== '3') fails.push(theme + ': count ' + await n());
  await p.getByRole('button', { name: /Daily Meeting/ }).first().click(); await p.locator('.notes').waitFor();
  if (await row.getAttribute('aria-current')) fails.push(theme + ': row still current in a meeting');
  await row.click(); await p.locator('.openitems').waitFor();
  if (await p.locator('.notes').count()) fails.push(theme + ': meeting still open');
  await p.locator('.oi input').first().click(); await p.waitForTimeout(300);
  if (await n() !== '2') fails.push(theme + ': count after tick ' + await n());
  await p.close();
  if (fails.length) throw Error(JSON.stringify(fails));
  return 'ok';
}
