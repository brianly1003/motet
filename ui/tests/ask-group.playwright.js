// Run as a Playwright page function. The Ask card's Record button group
// (owner 2026-10-21, after shadcn's Button Group): one 34 px height, square inner corners, a 1 px
// divider and a square menu button with a 16 px chevron, matching the mockup in both themes.
async (page) => {
  const results = [];
  for (const theme of ['light', 'dark']) {
    const ref = await page.context().newPage();
    await ref.route('https://**/*', r => r.abort());
    await ref.emulateMedia({colorScheme: theme});
    await ref.setViewportSize({width: 1400, height: 1000});
    await ref.goto('http://127.0.0.1:3001/motet-app-mockup.html');
    await ref.evaluate(t => document.documentElement.setAttribute('data-theme', t), theme);
    await ref.locator('#play').click(); // the scripted call asks at 0.7 s
    await ref.locator('#ask:not(.off)').waitFor();
    const p = await page.context().newPage();
    const errors = []; p.on('pageerror', e => errors.push(e.message));
    await p.emulateMedia({colorScheme: theme});
    await p.setViewportSize({width: 480, height: 200});
    await p.addInitScript(() => {
      let next = 0;
      const state = {state: {type: 'Asking', call_id: 'c1', app: 'Zoom'}, meeting_id: null, app: null, elapsed_ms: 0, started_by: null, auto_start: false, ask_on_call: true,
        problem: null, more_problems: 0, quit_pending: false, dnd_active: false, onboarding_done: true, sharing: null, ask_off_pending: null, discarded: null,
        companion: {visible: true, size: 'Ask', pinned: false, peeking: false, display_id: null}};
      window.testCalls = [];
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener: () => {}};
      window.__TAURI_INTERNALS__ = {metadata: {currentWindow: {label: 'companion'}, currentWebview: {windowLabel: 'companion', label: 'companion'}}, transformCallback: () => ++next, unregisterCallback: () => {},
        invoke: async (command, args) => { window.testCalls.push({command, args}); if (command === 'plugin:event|listen') return next; if (command === 'get_state') return state; return null; }};
    });
    await p.goto('http://127.0.0.1:3000/companion/');
    const more = p.locator('.askcard .split .more');
    await more.waitFor();
    const props = ['width', 'height', 'paddingTop', 'paddingLeft', 'borderTopLeftRadius', 'borderTopRightRadius', 'borderBottomRightRadius', 'borderLeftWidth', 'borderLeftColor', 'backgroundColor', 'color'];
    const read = (loc) => loc.evaluate((e, ps) => { const c = getComputedStyle(e), s = e.querySelector('svg'); return {...Object.fromEntries(ps.map(k => [k, c[k]])), svg: s ? `${getComputedStyle(s).width}x${getComputedStyle(s).height} ${s.innerHTML}` : 'none'}; }, props);
    const differences = [];
    for (const [a, b] of [['.askcard .split .more', '#ask .split .more'], ['.askcard .split > button:first-child', '#ask .split > button:first-child']]) {
      const x = await read(p.locator(a)), y = await read(ref.locator(b));
      for (const k of Object.keys(x)) if (k !== 'width' || a.includes('more')) if (x[k] !== y[k]) differences.push(`${a} ${k}: ${x[k]} != ${y[k]}`);
    }
    const [rec, mb] = [await p.locator('.askcard .split > button:first-child').boundingBox(), await more.boundingBox()];
    if (mb.width !== 34 || mb.height !== 34 || rec.height !== 34 || Math.abs(rec.x + rec.width - mb.x) > 0.5) differences.push(`group geometry ${JSON.stringify({rec, mb})}`);
    if (await more.getAttribute('aria-label') !== 'More choices' && !(await more.getAttribute('aria-label'))) differences.push('menu button lost its label');
    await more.click();
    await p.getByRole('menuitem', {name: 'Not for this call'}).waitFor();
    const card = await p.locator('.askcard').boundingBox();
    await p.screenshot({path: `.playwright-out/ask-group-${theme}.png`, clip: {x: card.x + card.width - 150, y: card.y, width: 150, height: 64}});
    if (differences.length || errors.length) throw Error(JSON.stringify({theme, differences, errors}));
    results.push(theme + ' PASS');
    await p.close(); await ref.close();
  }
  return results;
}
