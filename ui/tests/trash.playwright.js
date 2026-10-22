// Run as a Playwright page function. Recently deleted: Restore all and
// Delete all against the mockup (DESIGN 4.7). IPC is mocked.
async (page) => {
  const out = [];
  for (const theme of ['light', 'dark']) {
    const ref = await page.context().newPage();
    await ref.route('https://**/*', r => r.abort());
    await ref.emulateMedia({colorScheme: theme, reducedMotion: 'reduce'});
    await ref.setViewportSize({width: 1400, height: 1000});
    await ref.goto('http://127.0.0.1:3001/motetto-app-mockup.html');
    await ref.evaluate(t => document.documentElement.setAttribute('data-theme', t), theme);
    await ref.getByRole('button', {name: 'macOS', exact: true}).click();
    await ref.getByRole('button', {name: 'After the call', exact: true}).click();
    for (const t of ['Payment proxy page', 'Daily standup', 'Beta flag decision']) { await ref.locator('#sidelist .mitem', {hasText: t}).click(); await ref.keyboard.press('Meta+Backspace'); }
    await ref.locator('#openTrash').click();
    const p = await page.context().newPage();
    const errors = []; p.on('pageerror', e => errors.push(e.message));
    await p.emulateMedia({colorScheme: theme, reducedMotion: 'reduce'});
    await p.setViewportSize({width: 1048, height: 664});
    await p.addInitScript(() => {
      Object.defineProperty(navigator, 'platform', {value: 'MacIntel'});
      const now = Date.now();
      const mk = (id, title, ago) => ({id, title, created_ms: now - ago, status: 'Ready', marks_ms: [], duration_ms: 600000, notes_ms: 1000, spellings: []});
      let live = [mk('1', 'FCM vs Pusher Beams', 600e3)], gone = [mk('2', 'Payment proxy page', 3 * 86400e3), mk('3', 'Daily standup', 4 * 86400e3), mk('4', 'Beta flag decision', 9 * 86400e3)];
      window.calls = []; let next = 0;
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener: () => {}};
      window.__TAURI_INTERNALS__ = {metadata: {currentWindow: {label: 'main'}, currentWebview: {windowLabel: 'main', label: 'main'}}, transformCallback: () => ++next, unregisterCallback: () => {}, invoke: async (c, a) => {
        window.calls.push({c, a});
        if (c === 'plugin:event|listen') return next;
        if (c === 'get_state') return {state: {type: 'Idle'}, meeting_id: null, companion: {visible: false, size: 'Pill'}, elapsed_ms: 0, quit_pending: false};
        if (c === 'notification_permission') return true;
        if (c === 'list_meetings') return live; if (c === 'deleted_meetings') return gone; if (c === 'library_bytes') return 1; if (c === 'open_action_items') return [];
        if (c === 'meeting_action') {
          const move = (from, to) => { const i = from.findIndex(m => m.id === a.id); if (i < 0) throw Error(`No meeting ${a.id}`); to.push(...from.splice(i, 1)); };
          if (a.action === 'restore') move(gone, live); if (a.action === 'delete') move(live, gone); if (a.action === 'purge') { const i = gone.findIndex(m => m.id === a.id); gone.splice(i, 1); }
          return null;
        }
        return false; }};
    });
    await p.goto('http://127.0.0.1:3000/main/');
    await p.getByRole('button', {name: /^Recently deleted/}).click();
    await p.getByRole('heading', {name: 'Recently deleted'}).waitFor();
    const r = {};
    r.head = await p.locator('.trashview .titlerow').innerText();
    const props = ['padding', 'gap', 'fontSize', 'fontWeight', 'borderRadius', 'backgroundColor', 'color', 'borderColor', 'display', 'alignItems', 'opacity'];
    const differences = [];
    const compare = async pairs => { for (const [a, b] of pairs) {
      const x = await p.locator(a).first().evaluate((e, ps) => Object.fromEntries(ps.map(k => [k, getComputedStyle(e)[k]])), props);
      const y = await ref.locator(b).first().evaluate((e, ps) => Object.fromEntries(ps.map(k => [k, getComputedStyle(e)[k]])), props);
      for (const k of props) if (x[k] !== y[k]) differences.push(`${a} ${k}: ${x[k]} != ${y[k]}`);
    } };
    await compare([['.trashview .titlerow', '.trashview .titlerow'], ['.trashview .headacts', '.trashview .headacts'], ['.headacts button:first-child', '.headacts [data-restore-all]'], ['.headacts button:last-child', '.headacts [data-purge-all]'], ['.trashlist', '.trashlist'], ['.trow', '.trow']]);
    await p.screenshot({path: `.playwright-out/app-trash-${theme}.png`});
    // Delete all asks inline; Cancel has focus; Esc cancels.
    await p.getByRole('button', {name: 'Delete all…'}).click();
    await ref.locator('[data-purge-all]').click();
    r.confirm = await p.locator('.trashconfirm').innerText();
    r.focus = await p.evaluate(() => document.activeElement?.textContent);
    if (r.focus !== 'Cancel') throw Error(`Focus is on ${r.focus}, not Cancel`);
    await compare([['.trashconfirm', '.trashconfirm'], ['.trashconfirm .tt b', '.trashconfirm .tt b'], ['.trashconfirm .tt span', '.trashconfirm .tt span'], ['.trashconfirm .doc-action.danger', '.trashconfirm [data-purge-all-yes]'], ['.trashview.confirmingall .trashlist', '.trashview.confirmingall .trashlist']]);
    await p.screenshot({path: `.playwright-out/app-trash-confirm-${theme}.png`});
    // The rows are inert while the question is open: no click, no focus.
    await p.locator('.trow').first().getByRole('button', {name: 'Delete permanently'}).click({force: true, timeout: 2000}).catch(() => {});
    if (await p.locator('.trow.confirming').count()) throw Error('A row could still be deleted during Delete all');
    if (!await p.locator('.trashlist').evaluate(e => e.inert)) throw Error('The list is not inert during Delete all');
    if (!await ref.locator('.trashlist').evaluate(e => e.inert)) throw Error('The mockup list is not inert during Delete all');
    await p.keyboard.press('Escape');
    if (await p.locator('.trashconfirm').count()) throw Error('Esc did not cancel');
    if (await p.locator('.trashlist').evaluate(e => e.inert)) throw Error('The list stayed inert after Cancel');
    if (await p.evaluate(() => window.calls.some(c => c.a?.action === 'purge'))) throw Error('Cancel purged something');
    // Restore all, then Undo puts them back.
    await p.getByRole('button', {name: 'Restore all'}).click();
    await p.getByText('Restored 3 meetings.').waitFor();
    r.listAfterRestore = await p.locator('.meeting-row').count();
    await p.getByRole('button', {name: 'Undo'}).click();
    await p.getByRole('heading', {name: 'Recently deleted'}).waitFor().catch(() => {});
    await p.waitForFunction(() => window.calls.filter(c => c.a?.action === 'delete').length === 3);
    await p.getByRole('button', {name: /^Recently deleted/}).click();
    await p.locator('.trow').nth(2).waitFor();
    // Delete all: one purge per meeting, page closes, toast without Undo.
    await p.getByRole('button', {name: 'Delete all…'}).click();
    await p.getByRole('button', {name: 'Delete 3 meetings'}).click();
    await p.getByText('Deleted 3 meetings permanently.').waitFor();
    r.purges = await p.evaluate(() => window.calls.filter(c => c.a?.action === 'purge').length);
    r.trashClosed = await p.getByRole('button', {name: /^Recently deleted/}).count() === 0;
    r.undo = await p.getByRole('button', {name: 'Undo'}).count();
    if (r.purges !== 3 || !r.trashClosed || r.undo) throw Error(JSON.stringify(r));
    if (differences.length || errors.length) throw Error(JSON.stringify({theme, differences, errors}));
    out.push({theme, differences, errors, ...r});
    await p.close(); await ref.close();
  }
  return out;
}
