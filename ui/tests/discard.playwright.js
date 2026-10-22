// Discard in the Stopped Pill (owner 2026-10-20): no notes, the meeting goes to Recently deleted, and
// the Pill offers Undo, which brings it back and writes the notes after all.
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 460, height: 200 });
  await p.addInitScript(() => {
    let next = 0; const callbacks = {}, listeners = {};
    let state = { state: { type: 'Recording' }, meeting_id: 'm1', app: 'Microsoft Teams', elapsed_ms: 56000, started_by: { type: 'App', app: 'Microsoft Teams' }, ask_on_call: true, problem: null, more_problems: 0, quit_pending: false, dnd_active: false, onboarding_done: true, sharing: null, ask_off_pending: null, stops_at_ms: null, discarded: null,
      companion: { visible: true, size: 'Pill', pinned: false, peeking: false, display_id: '' } };
    window.calls = [];
    window.fixtureState = value => { state = { ...state, ...value }; callbacks[listeners['motetto://event']]?.({ payload: { type: 'StateChanged', ...state } }); };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'companion' }, currentWebview: { windowLabel: 'companion', label: 'companion' } },
      transformCallback: cb => { callbacks[++next] = cb; return next; }, unregisterCallback: id => delete callbacks[id],
      invoke: async (c, a) => {
        window.calls.push(c);
        if (c === 'plugin:event|listen') { listeners[a.event] = a.handler; return a.handler; }
        if (c === 'get_state') return state; if (c === 'get_levels') return [0, 0];
        if (c === 'live_transcript') return ['m1', '', '']; if (c === 'get_my_notes') return { meeting_id: 'm1', text: '', version: 1 };
        if (c === 'resume_remaining_ms') return 8000;
        if (c === 'discard_recording') { window.fixtureState({ state: { type: 'Idle' }, meeting_id: null, discarded: { meeting_id: 'm1', until_ms: 99999 } }); return null; }
        if (c === 'undo_discard') { window.undone = a.id; window.fixtureState({ discarded: null, companion: { visible: false, size: 'Pill', pinned: false, peeking: false, display_id: '' } }); return null; }
        return null; } };
  });
  await p.goto('http://127.0.0.1:3000/companion/');
  await p.evaluate(() => window.fixtureState({ state: { type: 'Stopped', resume_until_ms: 999999 } }));
  const discard = p.getByRole('button', { name: 'Discard' });
  await discard.waitFor();
  if (!(await discard.getAttribute('title'))?.includes('Recently deleted')) fails.push('Discard does not say where the recording goes');
  if (!(await p.getByRole('button', { name: 'Resume' }).isVisible())) fails.push('Resume missing beside Discard');
  // One line (owner 2026-10-20: "7 / s" wrapped in the mockup's Sidecar). In the app a Stop always
  // shows the 460 px Pill (session.rs); the seconds never truncate.
  const m = await p.locator('.pillbar').evaluate(bar => { const msg = bar.querySelector('.pillmsg'), num = msg.querySelector('.num'), n = num.getBoundingClientRect(), r = msg.getBoundingClientRect();
    return { lines: Math.round(r.height / parseFloat(getComputedStyle(msg).lineHeight || '18')), num: num.textContent, numWhole: n.right <= r.right + 0.5 && n.width > 10, fits: bar.scrollWidth <= bar.clientWidth };
  });
  if (m.lines > 1 || !m.numWhole || !m.fits || !/\d s$/.test(m.num)) fails.push(`Stopped bar: ${JSON.stringify(m)}`);
  await discard.click();
  await p.getByText("Recording discarded. It's in Recently deleted.").waitFor();
  await p.getByRole('button', { name: 'Undo' }).click();
  await p.waitForTimeout(200);
  const calls = await p.evaluate(() => [window.calls, window.undone]);
  if (!calls[0].includes('discard_recording')) fails.push('discard_recording not called');
  if (calls[1] !== 'm1') fails.push('undo_discard not called with the meeting: ' + calls[1]);
  await p.close();
  if (fails.length) throw Error(JSON.stringify(fails));
  return 'ok';
}
