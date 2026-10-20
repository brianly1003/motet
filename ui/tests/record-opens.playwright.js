// Record now from the tray while another meeting is open (owner, 2026-10-20): the new recording's
// meeting opens and is the selected row, even though its folder appears a moment after the state.
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 1200, height: 720 });
  await p.addInitScript(() => {
    const mk = (id, title, status, created) => ({ summary: { id, title, created_ms: created, status, marks_ms: [], duration_ms: 60000, notes_ms: 8000, spellings: [], locked: false, title_source: 'placeholder' }, transcript: '[00:04] Me: Alo', transcript_heard: '', notes_md: '### Action items\n- None\n' });
    const ms = [mk('old', 'Old meeting', 'Ready', Date.now() - 600000)];
    let next = 0, state = { state: { type: 'Idle' }, meeting_id: null, app: null, elapsed_ms: 0, started_by: null, ask_on_call: true, problem: null, more_problems: 0, quit_pending: false, dnd_active: false, onboarding_done: true, sharing: null, ask_off_pending: null, stops_at_ms: null, companion: { visible: false, size: 'Pill', pinned: false, peeking: false, display_id: '' } };
    const callbacks = {}, listeners = {};
    const emit = (event, payload) => (listeners[event] || []).forEach(id => callbacks[id]?.({ event, payload }));
    // The tray's Record now: the state first, then the folder, then library-changed and open-meeting (recording.rs).
    window.trayRecord = () => {
      state = { ...state, state: { type: 'Recording' }, meeting_id: 'new', started_by: { type: 'Manual' } };
      emit('motet://event', { type: 'StateChanged', ...state });
      setTimeout(() => { ms.unshift(mk('new', 'Recorded meeting', 'Recording', Date.now())); emit('motet://library-changed', null); emit('motet://open-meeting', 'new'); }, window.folderDelay ?? 300);
    };
    // Back to idle with the new meeting gone, so the next delay starts from the same place.
    window.resetFixture = () => { if (ms[0].summary.id === 'new') ms.shift(); state = { ...state, state: { type: 'Idle' }, meeting_id: null }; emit('motet://event', { type: 'StateChanged', ...state }); emit('motet://library-changed', null); };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = { convertFileSrc: x => x, transformCallback: cb => { callbacks[++next] = cb; return next; }, unregisterCallback: () => {}, invoke: async (c, a) => {
      // Real IPC takes time; the open-items scan reads every meeting's notes.
      if (c !== 'plugin:event|listen') await new Promise(r => setTimeout(r, c === 'open_action_items' ? 120 : 15 + Math.random() * 30));
      if (c === 'plugin:event|listen') { (listeners[a.event] ||= []).push(a.handler); return a.handler; }
      if (c === 'get_state') return state;
      if (c === 'list_meetings') return ms.map(m => m.summary); if (c === 'deleted_meetings') return [];
      if (c === 'get_meeting') { const m = ms.find(m => m.summary.id === a.id); if (!m) throw 'Meeting not found'; return m; }
      if (c === 'meeting_media') return { tracks: [], duration_ms: 60000, folder: 'x' }; if (c === 'open_action_items') return []; if (c === 'library_bytes') return 1; if (c === 'importing') return null;
      return null; } };
  });
  await p.goto('http://127.0.0.1:3000/main/');
  await p.getByRole('button', { name: /Old meeting/ }).first().click();
  await p.locator('.meeting-row[aria-current="page"]', { hasText: 'Old meeting' }).waitFor();
  // The folder and its library-changed can land within a few ms of the state (the real app).
  for (const delay of [300, 5, 0]) {
    await p.getByRole('button', { name: /Old meeting/ }).first().click();
    await p.locator('.meeting-row[aria-current="page"]', { hasText: 'Old meeting' }).waitFor();
    await p.evaluate(d => { window.folderDelay = d; window.trayRecord(); }, delay);
    await p.waitForTimeout(1200);
    const rows = await p.locator('.meeting-row[aria-current="page"]').allInnerTexts();
    if (rows.length !== 1 || !/Recording/.test(rows[0])) fails.push(`folder after ${delay} ms, selected: ${JSON.stringify(rows)}`);
    await p.evaluate(() => window.resetFixture());
  }
  await p.close();
  if (fails.length) throw Error(JSON.stringify(fails));
  return 'ok';
}
