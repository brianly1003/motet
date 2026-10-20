// Tucked keeps Pause (Resume while paused) and Stop one click away (DESIGN 4.3, owner 2026-10-20):
// a 28 x 104 capsule, the handle on top, nothing clipped, each button calls its command.
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 200, height: 260 });
  await p.addInitScript(() => {
    let next = 0; const callbacks = {}, listeners = {};
    let state = { state: { type: 'Recording' }, meeting_id: 'm1', app: 'Microsoft Teams', elapsed_ms: 56000, started_by: { type: 'App', app: 'Microsoft Teams' }, ask_on_call: true, problem: null, more_problems: 0, quit_pending: false, dnd_active: false, onboarding_done: true, sharing: null, ask_off_pending: null, stops_at_ms: null,
      companion: { visible: true, size: 'Tucked', pinned: false, peeking: false, display_id: '' } };
    window.calls = [];
    window.fixtureState = value => { state = { ...state, ...value }; callbacks[listeners['motet://event']]?.({ payload: { type: 'StateChanged', ...state } }); };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'companion' }, currentWebview: { windowLabel: 'companion', label: 'companion' } },
      transformCallback: cb => { callbacks[++next] = cb; return next; }, unregisterCallback: id => delete callbacks[id],
      invoke: async (c, a) => {
        window.calls.push(c);
        if (c === 'plugin:event|listen') { listeners[a.event] = a.handler; return a.handler; }
        if (c === 'get_state') return state; if (c === 'get_levels') return [0, 0];
        if (c === 'live_transcript') return ['m1', '', '']; if (c === 'get_my_notes') return { meeting_id: 'm1', text: '', version: 1 };
        if (c === 'pause_recording') { window.fixtureState({ state: { type: 'Paused' } }); return null; }
        if (c === 'resume_recording') { window.fixtureState({ state: { type: 'Recording' } }); return null; }
        return null; } };
  });
  await p.goto('http://127.0.0.1:3000/companion/');
  const cap = p.locator('.pill[data-size="tucked"]');
  await cap.waitFor();
  const box = await cap.boundingBox();
  if (Math.round(box.width) !== 28 || Math.round(box.height) !== 104) fails.push(`capsule ${box.width}x${box.height}`);
  const clipped = await cap.evaluate(e => [...e.querySelectorAll('.handle, .tuckctl button')].some(b => { const r = b.getBoundingClientRect(), c = e.getBoundingClientRect(); return r.top < c.top || r.bottom > c.bottom || r.left < c.left - 0.5 || r.right > c.right + 0.5; }));
  if (clipped) fails.push('a control sticks out of the capsule');
  await p.getByRole('button', { name: 'Pause recording' }).click();
  await p.getByRole('button', { name: 'Resume' }).waitFor();
  if (!(await cap.locator('.handle .dot.stopped').count())) fails.push('dot does not grey while paused');
  if (!(await p.locator('.pill[data-size="tucked"]').count())) fails.push('pausing left Tucked');
  await p.getByRole('button', { name: 'Resume' }).click();
  await p.getByRole('button', { name: 'Pause recording' }).waitFor();
  await p.getByRole('button', { name: 'Stop', exact: true }).click();
  await p.waitForTimeout(200);
  const calls = await p.evaluate(() => window.calls);
  for (const c of ['pause_recording', 'resume_recording', 'stop_recording']) if (!calls.includes(c)) fails.push(c + ' not called');
  await p.close();
  if (fails.length) throw Error(JSON.stringify(fails));
  return 'ok';
}
