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
    await p.setViewportSize({width: 460, height: 560});
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
          if (command === 'pause_recording') { state.state = {type: 'Paused'}; publish(); }
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
    // No size menu (owner 2026-10-20): ✎ opens the Sidecar.
    if (await p.getByRole('button', {name: 'Change size'}).count()) throw Error('The size menu is still there');
    await p.getByRole('button', {name: 'Open my notes', exact: true}).click();
    // ✎ is the only way in that sends the library to the Dock (owner 2026-10-20).
    await p.waitForFunction(() => window.testCalls.some(c => c.command === 'take_note'));
    const editor = p.getByRole('textbox', {name: 'My notes. Each line keeps the time you wrote it.'});
    await editor.waitFor();
    await editor.click(); await p.keyboard.press('End'); await p.keyboard.type(' tomorrow');
    await p.waitForTimeout(450);
    if (!await p.evaluate(() => window.testCalls.some(call => call.command === 'save_my_notes' && call.args.text.includes('tomorrow')))) throw Error('My notes did not save');
    await p.locator('.pill').screenshot({path: `.playwright-out/parity-sidecar-${theme}.png`});
    if (await p.locator('.duet').count()) { if (await p.locator('.duet').isVisible()) throw Error('Sidecar still shows the Duet (mockup hides it)'); }
    const sideBar = await p.locator('.pillbar').evaluate(e => [e.scrollWidth, e.clientWidth]);
    if (sideBar[0] > sideBar[1]) throw Error(`Sidecar bar overflows: ${sideBar}`);
    // Controls sit at the right edge (owner 2026-10-20), not bunched after the time.
    const edge = await p.locator('.pillbar').evaluate(e => e.getBoundingClientRect().right - e.lastElementChild.getBoundingClientRect().right);
    if (edge > 8) throw Error(`Sidecar controls are not right-aligned: ${edge}px left over`);
    // Owner's direction 2026-10-20: a click outside slides an unpinned Sidecar out and tucks it;
    // Keep open ignores both the click outside and Esc.
    const sizedBefore = await p.evaluate(() => window.testCalls.length);
    const collapsed = n => p.evaluate(n => window.testCalls.slice(n).some(c => c.command === 'set_companion_size'), n);
    await p.getByRole('button', {name: 'Keep open'}).click();
    await p.evaluate(() => window.fixtureEmit('tauri://blur'));
    await p.waitForTimeout(300);
    if (await collapsed(sizedBefore)) throw Error('Pinned Sidecar collapsed when focus left');
    await editor.click(); await p.keyboard.press('Escape'); await p.waitForTimeout(200);
    if (await collapsed(sizedBefore)) throw Error('Pinned Sidecar collapsed on Esc');
    await p.getByRole('button', {name: 'Keep open'}).click();
    await p.emulateMedia({colorScheme: theme, reducedMotion: 'no-preference'});
    await p.evaluate(() => window.fixtureEmit('tauri://blur'));
    await p.locator('.pill[data-leaving]').waitFor();
    if (await p.locator('.pill').evaluate(e => getComputedStyle(e).animationName) !== 'slide-out') throw Error('No slide-out when focus left');
    await p.waitForFunction(() => window.testCalls.some(c => c.command === 'set_companion_size' && c.args.size === 'Tucked'));
    await p.locator('.pill[data-size="tucked"]:not([data-leaving])').waitFor();
    if (await p.locator('.pill').evaluate(e => getComputedStyle(e).animationName) !== 'slide-in') throw Error('Tucked capsule did not slide in');
    await p.emulateMedia({colorScheme: theme, reducedMotion: 'reduce'});
    await p.evaluate(() => window.fixtureState({companion: {visible: true, size: 'Sidecar', pinned: false, peeking: false, display_id: null}}));
    await editor.waitFor();
    const escFrom = await p.evaluate(() => window.testCalls.length);
    await editor.click(); await p.keyboard.press('Escape');
    await p.waitForFunction(n => window.testCalls.slice(n).some(c => c.command === 'set_companion_size' && c.args.size === 'Tucked'), escFrom);
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
    if (await p.locator('.lines > .line.pending:last-child').innerText() !== '··:··\n...\nlistening') throw Error('Listening row missing after the newest line');
    // The three dots brighten in turn, but stay still under Reduce Motion (DESIGN 2.4).
    const dotMotion = () => p.locator('.lines > .line.pending .dots i').evaluateAll(dots => dots.map(d => `${getComputedStyle(d).animationName} ${getComputedStyle(d).animationDelay}`).join(','));
    if (await dotMotion() !== 'none 0s,none 0.2s,none 0.4s') throw Error(`Listening dots move under Reduce Motion: ${await dotMotion()}`);
    await p.emulateMedia({colorScheme: theme, reducedMotion: 'no-preference'});
    if (await dotMotion() !== 'listening-dot 0s,listening-dot 0.2s,listening-dot 0.4s') throw Error(`Listening dots do not animate in turn: ${await dotMotion()}`);
    await p.emulateMedia({colorScheme: theme, reducedMotion: 'reduce'});
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
    // Pause (owner 2026-10-20): not recording, so no red dot, frozen time, Resume and Stop.
    await p.getByRole('button', {name: 'Pause recording', exact: true}).click();
    await p.getByText('Paused · not recording').waitFor();
    if (await p.locator('.pillbar .dot.pulse').count()) throw Error('Paused bar still shows the recording dot');
    if (!await p.getByRole('button', {name: 'Resume', exact: true}).isVisible() || !await p.getByRole('button', {name: 'Stop', exact: true}).isVisible()) throw Error('Paused bar lacks Resume or Stop');
    const [resumeBox, stopBox] = await Promise.all(['Resume', 'Stop'].map(name => p.getByRole('button', {name, exact: true}).boundingBox()));
    if (Math.abs(resumeBox.height - stopBox.height) > 0.5) throw Error(`Resume ${resumeBox.height}px and Stop ${stopBox.height}px differ`);
    if (await p.locator('.line.pending').count()) throw Error('Listening row shown while paused');
    await p.locator('.pill').screenshot({path: `.playwright-out/parity-paused-${theme}.png`});
    await p.getByRole('button', {name: 'Resume', exact: true}).click();
    await p.getByRole('button', {name: 'Pause recording', exact: true}).waitFor();
    const pillBar = await p.locator('.pillbar').evaluate(e => [e.scrollWidth, e.clientWidth]);
    if (pillBar[0] > pillBar[1]) throw Error(`Pill bar overflows with Pause: ${pillBar}`);
    await p.getByRole('button', {name: 'Open my notes', exact: true}).click();
    await p.locator('.line.gap').waitFor();
    if (!(await p.locator('.line.gap').innerText()).includes('Paused for')) throw Error('No gap line after the pause');
    await p.evaluate(() => window.fixtureState({quit_pending: true}));
    await p.getByRole('button', {name: 'Keep recording', exact: true}).waitFor();
    if (!await p.locator('.caption').isVisible()) throw Error('Quit question lost its caption row (mockup keeps it)');
    await p.locator('.pill').screenshot({path: `.playwright-out/parity-quit-${theme}.png`});
    // Ask first (M6): a detected call asks in the Ask card; nothing records until Record.
    const call = name => p.evaluate(name => window.testCalls.filter(c => c.command === name).at(-1)?.args, name);
    const ask = (size, extra = {}) => p.evaluate(([size, extra]) => window.fixtureState({quit_pending: false, meeting_id: null, state: {type: 'Asking', call_id: 'c1', app: 'Zoom'}, companion: {visible: true, size, pinned: false, peeking: false, display_id: null}, ...extra}), [size, extra]);
    await p.setViewportSize({width: 480, height: 124});
    await ask('Ask');
    await p.getByText('Record this Zoom call?', {exact: true}).waitFor();
    await p.getByText('Nothing is recorded until you choose Record.', {exact: true}).waitFor();
    await p.getByText('Tell the people on the call before you record.', {exact: true}).waitFor();
    const cardBox = await p.locator('.askcard').boundingBox();
    if (Math.round(cardBox.width) !== 460) throw Error(`Ask card is ${cardBox.width}px wide`);
    if (await p.locator('.pill').count()) throw Error('Ask card rendered inside the recording Pill');
    await p.mouse.move(1, 1);
    await p.locator('.askcard').hover();
    await p.waitForFunction(() => window.testCalls.some(c => c.command === 'hold_ask' && c.args.hold === true));
    await p.waitForTimeout(200);
    if (await p.locator('.askcard .x').evaluate(e => getComputedStyle(e).opacity) !== '1') throw Error('Close x not shown on hover');
    await p.screenshot({path: `.playwright-out/ask-card-${theme}.png`});
    await p.mouse.move(2, 2);
    await p.waitForFunction(() => window.testCalls.some(c => c.command === 'hold_ask' && c.args.hold === false));
    if (!await p.getByRole('button', {name: 'Record', exact: true}).evaluate(e => { const probe = document.createElement('i'); probe.style.color = 'var(--ink-1)'; document.body.append(probe); const ink = getComputedStyle(probe).color; probe.remove(); return getComputedStyle(e).backgroundColor === ink; })) throw Error('Record must be ink, not red');
    await p.getByRole('button', {name: 'Record', exact: true}).click();
    const yes = await call('answer_ask');
    if (yes?.callId !== 'c1' || yes.record !== true) throw Error(`Record sent ${JSON.stringify(yes)}`);
    await p.getByRole('button', {name: 'Close: not this call'}).click({force: true});
    const no = await call('answer_ask');
    if (no?.callId !== 'c1' || no.record !== false) throw Error(`Close sent ${JSON.stringify(no)}`);
    await p.getByRole('button', {name: 'More choices'}).click();
    await p.getByRole('menuitem', {name: 'Not for this call'}).waitFor();
    // Rust grows the Ask window to 124 + menu height + 8 while the menu is open (ASK_MENU_HEIGHT in companion.rs).
    const menuBox = await p.locator('.askcard .dropdown').boundingBox();
    const menuWindow = 124 + 76 + 8;
    if (Math.round(menuBox.height) !== 76) throw Error(`Menu is ${menuBox.height}px tall, companion.rs assumes 76`);
    if (menuBox.y + menuBox.height > menuWindow) throw Error(`Open menu ends at ${menuBox.y + menuBox.height}px, past the ${menuWindow}px window`);
    if ((await call('companion_menu'))?.open !== true) throw Error('Opening the menu did not call companion_menu(true)');
    await p.setViewportSize({width: 480, height: menuWindow});
    await p.screenshot({path: `.playwright-out/ask-menu-${theme}.png`});
    await p.mouse.click(5, 5);
    await p.locator('.askcard .dropdown').waitFor({state: 'detached'});
    if ((await call('companion_menu'))?.open !== false) throw Error('Closing the menu did not call companion_menu(false)');
    await p.setViewportSize({width: 480, height: 124});
    await p.getByRole('button', {name: 'More choices'}).click();
    await p.getByRole('menuitem', {name: "Don't ask for Zoom"}).waitFor();
    await p.getByRole('menuitem', {name: "Don't ask for Zoom"}).click();
    if ((await call('dont_ask_for_app'))?.callId !== 'c1') throw Error("Don't ask did not send dont_ask_for_app");
    // After 20 s Rust sizes the session to the Pill: amber dot, Record, close, no caption row.
    await p.setViewportSize({width: 460, height: 72});
    await ask('Pill');
    await p.getByText('Zoom call · not recording', {exact: true}).waitFor();
    if (await p.locator('.caption').isVisible()) throw Error('Asking Pill shows the caption row');
    if (await p.locator('.pillbar .dot.pulse').count()) throw Error('Asking Pill shows the red recording dot');
    await p.locator('.pill').screenshot({path: `.playwright-out/ask-pill-${theme}.png`});
    await p.getByRole('button', {name: 'Record', exact: true}).click();
    if ((await call('answer_ask')).record !== true) throw Error('Pill Record did not answer');
    await p.getByRole('button', {name: 'Not for this call', exact: true}).click();
    if ((await call('answer_ask')).record !== false) throw Error('Pill close did not answer false');
    await p.evaluate(() => window.fixtureState({state: {type: 'Idle'}, ask_off_pending: {app_id: 'zoom', app: 'Zoom', previous: 'Ask', until_ms: 10000}}));
    await p.getByText("Motet won't ask about Zoom calls. Turn it back on in Settings › Calls.", {exact: true}).waitFor();
    await p.getByRole('button', {name: 'Undo', exact: true}).click();
    if ((await call('undo_dont_ask'))?.appId !== 'zoom') throw Error('Undo did not send undo_dont_ask');
    // The Sidecar's why line names the choice, not the microphone.
    await p.evaluate(() => window.fixtureState({ask_off_pending: null, meeting_id: 'fixture', state: {type: 'Recording'}, started_by: {type: 'App', app: 'Zoom'}, companion: {visible: true, size: 'Sidecar', pinned: false, peeking: false, display_id: null}}));
    await p.getByText('Recording because you chose Record for this Zoom call.', {exact: true}).waitFor();
    results.push({theme, differences, errors});
    await p.close();
  }
  await reference.close();
  return results;
}
