async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 340, height: 560 });
  await p.addInitScript(() => {
    let next = 0; const callbacks = {}, listeners = {};
    let state = { state: { type: 'Recording' }, meeting_id: 'm1', app: 'Microsoft Teams', elapsed_ms: 56000, started_by: { type: 'App', app: 'Microsoft Teams' }, ask_on_call: true, problem: null, more_problems: 0, quit_pending: false, dnd_active: false, onboarding_done: true, sharing: null, ask_off_pending: null, stops_at_ms: null,
      companion: { visible: true, size: 'Sidecar', pinned: true, peeking: false, display_id: '' } };
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
        if (c === 'live_transcript') return ['m1', '[00:44] Me: Hello.', '']; if (c === 'get_my_notes') return { meeting_id: 'm1', text: '', version: 1 };
        if (c === 'save_my_notes') return {meeting_id:'m1',text:a.text,version:2};
        if (c === 'pause_recording') { window.fixtureState({ state: { type: 'Paused' } }); return null; }
        if (c === 'resume_recording') { window.fixtureState({ state: { type: 'Recording' } }); return null; }
        return null; } };
  });
  await p.goto('http://127.0.0.1:3000/companion/');
  await p.locator('.line.pending').waitFor();
  for (const theme of ['light', 'dark']) {
    await p.emulateMedia({ colorScheme: theme });
    const handle = p.getByRole('separator');
    const before = await p.locator('.live-pane').boundingBox();
    const h = await handle.boundingBox();
    await p.mouse.move(h.x + h.width/2, h.y + h.height/2);
    await p.mouse.down(); await p.mouse.move(h.x + h.width/2, h.y + h.height/2 + 55, { steps: 5 }); await p.mouse.up();
    const after = await p.locator('.live-pane').boundingBox();
    if (after.height < before.height + 45) throw Error('Drag did not resize Live');
    await handle.focus(); await p.keyboard.press('Home');
    if (await handle.getAttribute('aria-valuenow') !== '20') throw Error('Home minimum');
    await p.keyboard.press('End'); await p.keyboard.press('ArrowDown');
    if (await handle.getAttribute('aria-valuenow') !== '80') throw Error('Maximum not clamped');
    await p.keyboard.press('ArrowUp');
    if (await handle.getAttribute('aria-valuenow') !== '75') throw Error('ArrowUp');
    const live = p.getByRole('button', { name: 'Live click a line to quote it' });
    await live.click();
    if (await handle.isVisible()) throw Error('Handle visible with Live collapsed');
    await live.click();
    if (await handle.getAttribute('aria-valuenow') !== '75') throw Error('Split lost after toggle');
    await handle.focus(); await p.keyboard.press('Home');
    await p.locator('.editor').fill('[] Share the design');
    await p.waitForTimeout(400);
    if (!(await p.evaluate(() => window.calls.includes('save_my_notes')))) throw Error('Note not saved');
    const layout = await p.evaluate(() => {
      const r = e => e.getBoundingClientRect();
      const real = r(document.querySelector('.line:not(.pending) .ts')), pending = r(document.querySelector('.pending .ts'));
      const editor = r(document.querySelector('.editor'));
      return { aligned: Math.abs(real.x-pending.x)<.1 && Math.abs(real.width-pending.width)<12,
        dot: getComputedStyle(document.querySelector('.dots i')).width,
        centered: [...document.querySelectorAll('.hints code')].every(e=>getComputedStyle(e).placeItems==='center'),
        fits: editor.height>30 && document.querySelector('.pill').scrollHeight <= 560 };
    });
    if (!layout.aligned || layout.dot !== '4px' || !layout.centered || !layout.fits) throw Error(JSON.stringify(layout));
    await p.locator('.pill').screenshot({path: `.playwright-out/sidecar-refined-${theme}.png`});
  }
  await p.setViewportSize({width:340,height:400});
  if ((await p.locator('.editor').boundingBox()).height < 30) throw Error('Small window editor unusable');
  await p.emulateMedia({ reducedMotion: 'reduce' });
  if (await p.locator('.dots i').first().evaluate(e=>getComputedStyle(e).animationName) !== 'none') throw Error('Reduce motion');
  const mock = await page.context().newPage();
  await mock.route('https://**/*', r=>r.abort());
  await mock.setViewportSize({width:1400,height:1000});
  await mock.goto('http://127.0.0.1:3001/motetto-app-mockup.html');
  await mock.getByRole('button',{name:'macOS',exact:true}).click();
  await mock.getByRole('button',{name:'During a call',exact:true}).click();
  await mock.locator('#caption').click();
  const handle = mock.getByRole('separator'); await handle.waitFor();
  await handle.focus(); await mock.keyboard.press('End');
  if (await handle.getAttribute('aria-valuenow') !== '80') throw Error('Mockup keyboard resize');
  const h=await handle.boundingBox(); await mock.mouse.move(h.x+h.width/2,h.y+h.height/2); await mock.mouse.down();
  await mock.mouse.move(h.x+h.width/2,h.y+h.height/2-50,{steps:5}); await mock.mouse.up();
  if (Number(await handle.getAttribute('aria-valuenow')) >= 80) throw Error('Mockup drag');
  await mock.locator('#liveHead').focus();
  for (const theme of ['light','dark']) {
    await mock.evaluate(t=>document.documentElement.dataset.theme=t,theme);
    await p.emulateMedia({colorScheme:theme});
    for (const selector of ['.pane-resizer', '.hints code']) {
      const styles = el => { const s=getComputedStyle(el); return [s.height,s.display,s.placeItems,s.borderRadius,s.backgroundColor,s.color,s.padding]; };
      const actual=await p.locator(selector).first().evaluate(styles), reference=await mock.locator(selector).first().evaluate(styles);
      if (JSON.stringify(actual)!==JSON.stringify(reference)) throw Error(`Parity ${selector}: ${JSON.stringify({actual,reference})}`);
    }
    await mock.locator('#pill').screenshot({path:`.playwright-out/sidecar-mockup-refined-${theme}.png`});
  }
  await mock.getByRole('button',{name:'After the call',exact:true}).click();
  await mock.getByText('Action items', {exact:true}).first().click();
  const banner = mock.locator('.banner').first(); await banner.waitFor();
  if (await banner.evaluate(e=>getComputedStyle(e).borderLeftWidth) !== '1px') throw Error('Old alert stripe');
  await banner.screenshot({path:'.playwright-out/alert-refined.png'});
  await mock.close(); await p.close();
  if (fails.length) throw Error(JSON.stringify(fails));
  return 'PASS: drag, keyboard bounds, Live collapse, notes save, listening alignment, badges, small viewport, reduced motion, mockup parity and alerts';
}
