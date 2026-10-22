// Run as a Playwright page function. IPC is mocked only in this browser fixture.
// A long row title fades out instead of ending in "…"; pointing at the row slides the fade left so
// the words vanish behind the "⋮" (owner 2026-10-23, DESIGN 4.x Layout). Set APP/MOCK if not 3000/3001.
async (page) => {
  const APP = globalThis.APP || 'http://127.0.0.1:3000', MOCK = globalThis.MOCK || 'http://127.0.0.1:3001';
  const results = [];
  const title = 'Update on Dashboard and Timeline Alignment with the vendor';
  const style = (loc) => loc.evaluate(e => { const s = getComputedStyle(e); return {mask: s.webkitMaskImage || s.maskImage, overflow: s.textOverflow, fade: s.getPropertyValue('--fade').trim(), duration: s.transitionDuration}; });
  for (const theme of ['light', 'dark']) {
    const reference = await page.context().newPage();
    await reference.route('https://**/*', route => route.abort());
    await reference.emulateMedia({colorScheme: theme});
    await reference.goto(`${MOCK}/motetto-app-mockup.html`);
    await reference.getByRole('button', {name: 'After the call', exact: true}).click();
    await reference.evaluate(theme => document.documentElement.setAttribute('data-theme', theme), theme);
    const p = await page.context().newPage();
    const errors = [];
    p.on('pageerror', error => errors.push(error.message));
    await p.emulateMedia({colorScheme: theme});
    await p.setViewportSize({width: 900, height: 560});
    await p.addInitScript((title) => {
      const now = Date.now();
      const list = [{id: '1', title, created_ms: now, duration_ms: 294000, status: 'Ready', marks_ms: []}, {id: '2', title: 'Call setup', created_ms: now - 60000, duration_ms: 237000, status: 'Ready', marks_ms: []}];
      let next = 0;
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener: () => {}};
      window.__TAURI_INTERNALS__ = {transformCallback: () => ++next, unregisterCallback: () => {}, invoke: async (command, args) => {
        if (command === 'plugin:event|listen') return next;
        if (command === 'get_state') return {state: {type: 'Idle'}, meeting_id: null, companion: {visible: false, size: 'Pill'}, elapsed_ms: 0, quit_pending: false};
        if (command === 'notification_permission') return true;
        if (command === 'list_meetings') return list;
        if (command === 'deleted_meetings') return [];
        if (command === 'get_meeting') return {summary: list.find(m => m.id === args.id) || list[0], notes_md: '', transcript: ''};
        if (command === 'meeting_media') return {tracks: [], duration_ms: 0, folder: '/local/meeting'};
        return false;
      }};
    }, title);
    await p.goto(`${APP}/main/`);
    const row = p.locator('.mrow').filter({hasText: title});
    const strong = row.locator('.meeting-row strong');
    await strong.waitFor();
    const [mockRow] = [reference.locator('.mrow').first()];
    const mockTitle = mockRow.locator('.mt');
    await p.mouse.move(0, 0);
    const rest = await style(strong), mockRest = await style(mockTitle);
    if (rest.overflow === 'ellipsis' || mockRest.overflow === 'ellipsis') throw Error('Title still ends in an ellipsis');
    if (!rest.mask.includes('linear-gradient') || rest.fade !== '0px') throw Error(`No resting fade: ${JSON.stringify(rest)}`);
    if (rest.mask !== mockRest.mask) throw Error(`Resting mask differs from the mockup: ${rest.mask} != ${mockRest.mask}`);
    if (rest.duration !== mockRest.duration || rest.duration !== '0.18s') throw Error(`Transition ${rest.duration} / ${mockRest.duration}`);
    // The title is no longer shortened for the dots: it keeps the full row width.
    const [rowBox, restBox] = [await row.boundingBox(), await strong.boundingBox()];
    if (rowBox.x + rowBox.width - (restBox.x + restBox.width) > 10) throw Error('Title does not use the row width');
    await p.screenshot({path: `.playwright-out/row-fade-rest-${theme}.png`, clip: {x: rowBox.x - 8, y: rowBox.y - 8, width: rowBox.width + 16, height: rowBox.height * 2 + 16}});
    await row.hover();
    await mockRow.hover();
    await p.waitForTimeout(300);
    const hover = await style(strong), mockHover = await style(mockTitle);
    if (hover.fade !== '20px' || mockHover.fade !== '20px') throw Error(`Hover fade ${hover.fade} / ${mockHover.fade}`);
    const [hoverBox, dots] = [await strong.boundingBox(), await row.locator('.rowmore').boundingBox()];
    if (Math.round(hoverBox.width) !== Math.round(restBox.width)) throw Error('Hover still narrows the title');
    // Fully transparent 20 px before the title's end: at or left of the dots' left edge.
    if (hoverBox.x + hoverBox.width - 20 > dots.x + 0.5) throw Error(`Words show under the dots: ${hoverBox.x + hoverBox.width - 20} > ${dots.x}`);
    await p.screenshot({path: `.playwright-out/row-fade-hover-${theme}.png`, clip: {x: rowBox.x - 8, y: rowBox.y - 8, width: rowBox.width + 16, height: rowBox.height * 2 + 16}});
    await reference.screenshot({path: `.playwright-out/row-fade-mockup-hover-${theme}.png`, clip: await mockRow.boundingBox().then(b => ({x: b.x - 8, y: b.y - 8, width: b.width + 16, height: b.height + 16}))});
    // A short title has nothing under the fade.
    const short = p.locator('.mrow').filter({hasText: 'Call setup'}).locator('.meeting-row strong');
    if (await short.evaluate(e => { const r = document.createRange(); r.selectNodeContents(e); return r.getBoundingClientRect().right > e.getBoundingClientRect().right - 24; })) throw Error('Short title reaches the fade');
    // Reduce Motion: the fade jumps instead of sliding.
    await p.emulateMedia({colorScheme: theme, reducedMotion: 'reduce'});
    if ((await style(strong)).duration !== '0s') throw Error('Fade still animates under Reduce Motion');
    if (errors.length) throw Error(errors.join('\n'));
    results.push(`${theme}: rest ${rest.fade}, hover ${hover.fade}, dots at ${Math.round(dots.x)}, title end ${Math.round(hoverBox.x + hoverBox.width)}`);
    await p.close(); await reference.close();
  }
  return results;
}
