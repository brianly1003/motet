// Run as a Playwright page function. These are UI fixtures, not native capture tests.
async (page) => {
  const results = [];
  const reference = await page.context().newPage();
  await reference.route('https://**/*', route => route.abort());
  await reference.setViewportSize({width: 1400, height: 1000});
  await reference.goto('http://127.0.0.1:3001/motet-app-mockup.html');
  await reference.getByRole('button', {name: 'macOS', exact: true}).click();
  await reference.getByRole('button', {name: 'During a call', exact: true}).click();
  for (const theme of ['light', 'dark']) {
    const p = await page.context().newPage();
    const errors = [];
    p.on('pageerror', error => errors.push(error.message));
    await p.emulateMedia({colorScheme: theme, reducedMotion: 'reduce'});
    await reference.emulateMedia({colorScheme: theme, reducedMotion: 'reduce'});
    await reference.evaluate(theme => document.documentElement.setAttribute('data-theme', theme), theme);
    await p.setViewportSize({width: 420, height: 560});
    await p.addInitScript(() => {
      let next = 0;
      const callbacks = {}, listeners = {};
      let state = {state: {type: 'Recording'}, meeting_id: 'fixture', app: null, elapsed_ms: 319000,
        started_by: {type: 'Manual'}, auto_start: false, problem: null, more_problems: 0, quit_pending: false,
        dnd_active: false, onboarding_done: true, sharing: null, never_pending: null,
        companion: {visible: true, size: 'Pill', pinned: false, peeking: false, display_id: null}};
      let notes = {meeting_id: 'fixture', text: '[05:00] [] Share the design', version: 1};
      window.testCalls = [];
      window.fixtureTranscript = "[05:00] Me: Sure, I'll share the table design before Thursday.";
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener: () => {}};
      const publish = () => callbacks[listeners['motet://event']]?.({payload: {type: 'StateChanged', ...state}});
      window.fixtureState = value => { state = {...state, ...value}; publish(); };
      window.fixtureEmit = (event, payload) => { if (!listeners[event]) throw Error(`No listener for ${event}`); callbacks[listeners[event]]({event, id: 0, payload}); };
      window.__TAURI_INTERNALS__ = {
        metadata: {currentWindow: {label: 'companion'}, currentWebview: {windowLabel: 'companion', label: 'companion'}},
        transformCallback: callback => { callbacks[++next] = callback; return next; },
        unregisterCallback: id => delete callbacks[id],
        invoke: async (command, args) => {
          window.testCalls.push({command, args});
          if (command === 'plugin:event|listen') { listeners[args.event] = args.handler; return next; }
          if (command === 'get_state') return state;
          if (command === 'get_levels') return [0, 0];
          if (command === 'live_transcript') return ['fixture', window.fixtureTranscript, window.fixtureLiveError || ''];
          if (command === 'get_my_notes') return notes;
          if (command === 'save_my_notes') { notes = {...notes, text: args.text, version: notes.version + 1}; return notes; }
          if (command === 'mark_moment') return state.elapsed_ms;
          if (command === 'resume_remaining_ms') return 8000;
          if (command === 'set_companion_size') { state.companion = {...state.companion, size: args.size}; publish(); }
          if (command === 'pin_companion') { state.companion = {...state.companion, pinned: args.pinned}; publish(); }
          if (command === 'stop_recording') { state.state = {type: 'Stopped', resume_until_ms: 10000}; state.companion.size = 'Pill'; publish(); }
          if (command === 'resume_recording') { state.state = {type: 'Recording'}; publish(); }
          if (command === 'quit') { state.quit_pending = false; publish(); }
          return null;
        }
      };
    });
    await p.goto('http://127.0.0.1:3000/companion/');
    await p.getByRole('button', {name: 'Open my notes', exact: true}).waitFor();
    await p.waitForTimeout(1100);
    const selectors = ['.pill', '.pillbar', '.duet', '.duet svg', '.caption', '.stop'];
    const properties = ['width', 'height', 'padding', 'gap', 'fontFamily', 'fontSize', 'lineHeight', 'borderRadius', 'backgroundColor', 'color'];
    const styles = async (target) => Object.fromEntries(await Promise.all(selectors.map(async selector => [selector, await target.locator(selector).first().evaluate((element, properties) => {
      const style = getComputedStyle(element); return Object.fromEntries(properties.map(property => [property, style[property]]));
    }, properties)])));
    const actual = await styles(p), expected = await styles(reference);
    const differences = selectors.flatMap(selector => properties.filter(key => actual[selector][key] !== expected[selector][key]).map(key => ({selector, key, actual: actual[selector][key], expected: expected[selector][key]})));
    await p.locator('.pill').screenshot({path: `.playwright-out/parity-pill-${theme}.png`});
    await reference.locator('#pill').screenshot({path: `.playwright-out/reference-pill-${theme}.png`});
    await p.getByRole('button', {name: 'Mark this moment', exact: true}).click();
    if (await p.getByRole('button', {name: 'Mark this moment', exact: true}).getAttribute('aria-pressed') !== 'true') throw Error('Mark feedback missing');
    for (const selector of ['.t', '.duet svg', '.pillbar > .dot']) {
      const dragRegion = await p.locator(selector).evaluate(element => {
        const rect = element.getBoundingClientRect();
        return document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2)?.hasAttribute('data-tauri-drag-region');
      });
      if (!dragRegion) throw Error(`Dragging is blocked over ${selector}`);
    }
    await p.getByRole('button', {name: 'Change size'}).click();
    const menuBounds = await p.getByRole('menu').boundingBox();
    const pillBounds = await p.locator('.pill').boundingBox();
    if (menuBounds.y < pillBounds.y + pillBounds.height + 6 || menuBounds.y + menuBounds.height > 300) throw Error('Menu overlaps the Pill or exceeds its native window');
    await p.screenshot({path: `.playwright-out/parity-size-menu-${theme}.png`});
    // A press outside the menu closes it and shrinks the window: on the clear area, then on the Pill.
    for (const [x, y] of [[40, 500], [30, 22]]) {
      const before = await p.evaluate(() => window.testCalls.length);
      await p.mouse.click(x, y);
      if (await p.getByRole('menu').count()) throw Error(`Size menu stayed open after a click at ${x},${y}`);
      if (!await p.evaluate(n => window.testCalls.slice(n).some(c => c.command === 'companion_menu' && c.args.open === false), before)) throw Error('Window did not shrink after closing the menu');
      await p.getByRole('button', {name: 'Change size'}).click();
    }
    await p.getByRole('menu').click({position: {x: 5, y: 5}});
    if (!await p.getByRole('menu').count()) throw Error('A click inside the menu closed it');
    await p.getByRole('menuitemradio', {name: 'Notes and live transcript'}).click();
    const editor = p.getByRole('textbox', {name: 'My notes. Each line keeps the time you wrote it.'});
    await editor.waitFor();
    await editor.click(); await p.keyboard.press('End'); await p.keyboard.type(' tomorrow');
    await p.waitForTimeout(450);
    if (!await p.evaluate(() => window.testCalls.some(call => call.command === 'save_my_notes' && call.args.text.includes('tomorrow')))) throw Error('My notes did not save');
    await p.locator('.pill').screenshot({path: `.playwright-out/parity-sidecar-${theme}.png`});
    // Owner's direction 2026-10-20: focus leaving keeps the Sidecar open; Esc tucks it unless pinned.
    const sizedBefore = await p.evaluate(() => window.testCalls.length);
    await p.evaluate(() => window.fixtureEmit('tauri://blur'));
    await p.waitForTimeout(200);
    if (await p.evaluate(n => window.testCalls.slice(n).some(c => c.command === 'set_companion_size'), sizedBefore)) throw Error('Sidecar collapsed when focus left');
    await p.getByRole('button', {name: 'Keep open'}).click();
    await editor.click(); await p.keyboard.press('Escape'); await p.waitForTimeout(200);
    if (await p.evaluate(n => window.testCalls.slice(n).some(c => c.command === 'set_companion_size'), sizedBefore)) throw Error('Pinned Sidecar collapsed on Esc');
    await p.getByRole('button', {name: 'Keep open'}).click();
    await editor.click(); await p.keyboard.press('Escape');
    await p.waitForFunction(() => window.testCalls.some(c => c.command === 'set_companion_size' && c.args.size === 'Tucked'));
    await p.evaluate(() => window.fixtureState({companion: {visible: true, size: 'Sidecar', pinned: false, peeking: false, display_id: null}}));
    await editor.waitFor();
    await p.evaluate(() => { window.fixtureTranscript = Array.from({length:30},(_,i)=>`[05:${String(i).padStart(2,'0')}] Me: Live message ${i}`).join('\n'); });
    await p.waitForFunction(() => { const box=document.querySelector('.lines'); return box?.querySelectorAll('button.line').length===30 && box.scrollTop>0 && Math.abs(box.scrollHeight-box.clientHeight-box.scrollTop)<2; });
    await editor.focus();
    await p.evaluate(() => { document.querySelector('.lines').scrollTop=0; });
    const focusSession = await p.context().newCDPSession(p);
    await focusSession.send("Emulation.setFocusEmulationEnabled", {enabled:false});
    await reference.bringToFront();
    await p.evaluate(() => { window.fixtureTranscript += '\n[05:31] Them: Newest background message'; });
    await p.waitForFunction(() => { const box=document.querySelector('.lines'); return [...(box?.querySelectorAll('button.line')||[])].at(-1)?.textContent.includes('Newest background message') && Math.abs(box.scrollHeight-box.clientHeight-box.scrollTop)<2; });
    if (await p.evaluate(() => document.hasFocus())) throw Error('Background fixture unexpectedly has window focus');
    if (!await editor.evaluate(e => e===document.activeElement)) throw Error('Auto-scroll stole editor focus');
    await p.screenshot({path: `.playwright-out/live-autoscroll-${theme}.png`});
    if (await p.locator('.lines > .line.pending:last-child').innerText() !== '··:··\n…\nlistening') throw Error('Listening row missing after the newest line');
    await p.bringToFront();
    await p.locator('.panehead[aria-expanded]').click();
    await p.evaluate(() => { window.fixtureTranscript += '\n[05:32] Me: Arrived while collapsed'; });
    await p.waitForTimeout(1200);
    await p.locator('.panehead[aria-expanded]').click();
    await p.waitForFunction(() => { const box=document.querySelector('.lines'); return [...(box?.querySelectorAll('button.line')||[])].at(-1)?.textContent.includes('Arrived while collapsed') && Math.abs(box.scrollHeight-box.clientHeight-box.scrollTop)<2; });

    await p.evaluate(() => { window.fixtureTranscript = ''; window.fixtureLiveError = 'Live captions are unavailable. The full transcript will be written after the call.'; });
    await p.locator('.lines [role="status"]').waitFor();
    if ((await p.locator('.lines').innerText()).includes('Listening')) throw Error('Live error hidden by listening placeholder');
    await p.screenshot({path: `.playwright-out/live-error-${theme}.png`});
    await p.evaluate(() => { window.fixtureLiveError = ''; window.fixtureTranscript = '[05:34] Me: Captions recovered'; });
    await p.waitForFunction(() => document.querySelector('.lines')?.textContent.includes('Captions recovered') && !document.querySelector('.lines [role="status"]'));

    await p.getByRole('button', {name: 'Tuck to the screen edge'}).click();
    await p.getByRole('button', {name: 'Recording. Open notes and live transcript'}).click();
    await p.getByRole('button', {name: 'Stop', exact: true}).click();
    await p.getByRole('button', {name: 'Resume', exact: true}).waitFor();
    if (await p.locator('.line.pending').count()) throw Error('Listening row shown while stopped');
    if (!await p.locator('.caption').isVisible()) throw Error('Stopped Pill lost its caption row (mockup keeps it)');
    await p.locator('.pill').screenshot({path: `.playwright-out/parity-stopped-${theme}.png`});
    await p.getByRole('button', {name: 'Resume', exact: true}).click();
    await p.evaluate(() => window.fixtureState({quit_pending: true}));
    await p.getByRole('button', {name: 'Keep recording', exact: true}).waitFor();
    if (!await p.locator('.caption').isVisible()) throw Error('Quit question lost its caption row (mockup keeps it)');
    await p.locator('.pill').screenshot({path: `.playwright-out/parity-quit-${theme}.png`});
    results.push({theme, differences, errors});
    await p.close();
  }
  await reference.close();
  return results;
}
