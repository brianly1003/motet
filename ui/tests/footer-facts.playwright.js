// The full sidebar footer fits at the minimum window size, including the location label.
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.addInitScript(() => {
    const ms = Array.from({ length: 10 }, (_, i) => ({ id: String(i), title: `Meeting ${i}`, created_ms: Date.now() - i * 60000, status: 'Ready', marks_ms: [], duration_ms: 60000, notes_ms: 8000, spellings: [], locked: false, title_source: 'placeholder' }));
    let next = 0;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = { convertFileSrc: x => x, transformCallback: () => ++next, unregisterCallback: () => {}, invoke: async (c) => {
      if (c === 'plugin:event|listen') return next; if (c === 'get_state') return { state: { type: 'Idle' }, meeting_id: null, companion: { visible: false, size: 'Pill' }, elapsed_ms: 0, quit_pending: false };
      if (c === 'list_meetings') return ms; if (c === 'deleted_meetings') return [ms[0]]; if (c === 'open_action_items') return []; if (c === 'library_bytes') return 395.1 * 1000 * 1000; if (c === 'importing') return null;
      return null; } };
  });
  for (const theme of ["light", "dark"]) for (const width of [1200, 760]) {
    await p.emulateMedia({ colorScheme: theme });
    await p.setViewportSize({ width, height: 520 });
    await p.goto('http://127.0.0.1:3000/main/');
    await p.locator('.privacy .where b', { hasText: '395' }).waitFor();
    const m = await p.locator('.privacy').evaluate(f => { const label = f.querySelector('.where > span'), b = f.querySelector('.where b'), r = b.getBoundingClientRect(), box = f.getBoundingClientRect(); return { text: b.textContent, whole: label.scrollWidth <= label.clientWidth + 1 && b.scrollWidth <= b.clientWidth + 1 && r.right <= box.right + 0.5, title: f.querySelector('.where').title }; });
    if (!m.whole || !/10 meetings · 395/.test(m.text)) fails.push(`${width}px: ${JSON.stringify(m)}`);
    if (!m.title.startsWith('On this computer · 10 meetings')) fails.push('tooltip lacks the full line');
    await p.locator('.privacy').screenshot({ path: `.playwright-out/footer-${theme}-${width}.png` });
  }
  await p.close();
  if (fails.length) throw Error(JSON.stringify(fails));
  return 'ok';
}
