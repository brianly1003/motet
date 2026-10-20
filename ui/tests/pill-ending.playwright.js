// The idle stop counts down in the Pill (SPEC 4.2): once the call's app lets go of the mic it reads
// "Call over? Stopping in N s" with Keep recording and Stop; Keep recording calls keep_recording.
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 460, height: 200 });
  await p.addInitScript(() => {
    let next = 0; const callbacks = {}, listeners = {};
    let state = { state: { type: 'Recording' }, meeting_id: 'm1', app: 'Microsoft Teams', elapsed_ms: 56000, started_by: { type: 'App', app: 'Microsoft Teams' }, ask_on_call: true, problem: null, more_problems: 0, quit_pending: false, dnd_active: false, onboarding_done: true, sharing: null, ask_off_pending: null, stops_at_ms: null,
      companion: { visible: true, size: 'Pill', pinned: false, peeking: false, display_id: '' } };
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
        if (c === 'keep_recording') { window.fixtureState({ stops_at_ms: null }); return null; }
        return null; } };
  });
  await p.goto('http://127.0.0.1:3000/companion/');
  await p.getByRole('button', { name: 'Stop' }).waitFor();
  if (await p.getByText('Call over?').count()) fails.push('countdown shown while people talk');
  await p.evaluate(() => window.fixtureState({ stops_at_ms: 56000 + 23000 }));
  const msg = p.locator('.pillmsg');
  await msg.waitFor();
  if ((await msg.innerText()).replace(/\s+/g, ' ').trim() !== 'Call over? Stopping in 23 s') fails.push('pill text: ' + await msg.innerText());
  if (!(await msg.getAttribute('title'))?.startsWith('Microsoft Teams stopped using the microphone')) fails.push('no reason on hover');
  if (!(await p.getByRole('button', { name: 'Stop' }).isVisible())) fails.push('Stop missing during the countdown');
  // The bar fits the 460 px Pill without wrapping or clipping.
  const fit = await p.locator('.pillmsg').evaluate(e => { const bar = e.parentElement; return bar.scrollWidth <= bar.clientWidth && e.getBoundingClientRect().height < 30; });
  if (!fit) fails.push('countdown does not fit the Pill');
  await p.evaluate(() => window.fixtureState({ elapsed_ms: 70000 }));
  if (!(await msg.innerText()).includes('9 s')) fails.push('countdown does not follow the clock: ' + await msg.innerText());
  await p.getByRole('button', { name: 'Keep recording' }).click();
  await p.waitForTimeout(200);
  if (!(await p.evaluate(() => window.calls.includes('keep_recording')))) fails.push('keep_recording not called');
  if (await p.getByText('Call over?').count()) fails.push('countdown still shown after Keep recording');
  await p.close();
  if (fails.length) throw Error(JSON.stringify(fails));
  return 'ok';
}
