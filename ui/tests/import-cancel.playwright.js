// An import runs beside everything else (SPEC 7.7): its meeting opens on its own progress page,
// other meetings stay usable, a second Import waits with its reason, and Cancel shows Stopping…
// then closes the page with a toast. Home's checkboxes match the notes' tasks.
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 1200, height: 720 });
  await p.addInitScript(() => {
    const mk = (id, title, status, notes) => ({ summary: { id, title, created_ms: Date.now() - +id * 1000, status, marks_ms: [], duration_ms: 87000, notes_ms: 48000, spellings: [], locked: false }, transcript: '[00:01] Me: hi', transcript_heard: '', notes_md: notes });
    const ms = [mk('1', 'Daily Meeting', 'Ready', '### Action items\n- [ ] Me: Brush my teeth [01:00]\n')];
    let next = 0, job = null, finish = null; const callbacks = {}, listeners = {};
    window.calls = [];
    const emit = event => (listeners[event] || []).forEach(id => callbacks[id]?.({ event, payload: null }));
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = { convertFileSrc: x => x, transformCallback: cb => { callbacks[++next] = cb; return next; }, unregisterCallback: () => {}, invoke: async (c, a) => {
      window.calls.push(c);
      if (c === 'plugin:event|listen') { (listeners[a.event] ||= []).push(a.handler); return a.handler; }
      if (c === 'get_state') return { state: { type: 'Idle' }, meeting_id: null, companion: { visible: false, size: 'Pill' }, elapsed_ms: 0, quit_pending: false };
      if (c === 'list_meetings') return ms.map(m => m.summary); if (c === 'deleted_meetings') return []; if (c === 'get_meeting') { const m = ms.find(m => m.summary.id === a.id); if (!m) throw 'gone'; return m; }
      if (c === 'open_action_items') return ms.filter(m => m.notes_md.includes('- [ ]')).map(m => [m.summary, m.notes_md]); if (c === 'library_bytes') return 1000;
      if (c === 'meeting_media') return { tracks: [], duration_ms: 87000, folder: 'x' };
      if (c === 'set_action_item_done') { const m = ms.find(m => m.summary.id === a.id); m.notes_md = m.notes_md.replace('- [ ]', '- [x]'); return m.notes_md; }
      if (c === 'importing') return job;
      // Rust makes the meeting folder, then says so; the import resolves only when it ends.
      if (c === 'import_audio') { job = { id: null, import: true }; setTimeout(() => { ms.unshift(mk('2', 'Quarterly review', 'Finishing', '')); job = { id: '2', import: true }; emit('motetto://library-changed'); }, 100); return new Promise(r => { finish = r; }); }
      if (c === 'cancel_import') { setTimeout(() => { ms.shift(); job = null; finish(null); emit('motetto://library-changed'); }, 300); return null; }
      return false; } };
  });
  await p.goto('http://127.0.0.1:3000/main/');
  await p.locator('.openitems .oi').first().waitFor();
  const home = await p.locator('.openitems .oi input').first().evaluate(e => [e.offsetWidth, e.offsetHeight]);
  await p.getByRole('button', { name: 'Import a recording…' }).first().click();
  // The import's own page opens with Cancel; the list shows it writing.
  const cancel = p.getByRole('button', { name: 'Cancel import' });
  await cancel.waitFor();
  if (!(await p.locator('.home h1', { hasText: 'Quarterly review' }).count())) fails.push('progress page lacks the meeting title');
  if (!(await p.locator('.meeting-row[aria-current="page"]', { hasText: 'Writing notes' }).count())) fails.push('import row not current or not marked Writing notes');
  // Other meetings stay usable meanwhile.
  await p.getByRole('button', { name: /Daily Meeting/ }).first().click();
  await p.locator('.todo input').first().waitFor();
  const notes = await p.locator('.todo input').first().evaluate(e => [e.offsetWidth, e.offsetHeight]);
  if (home.join() !== notes.join()) fails.push(`checkboxes differ: Home ${home} vs Notes ${notes}`);
  await p.locator('.homerow').click();
  const again = p.getByRole('button', { name: 'Import a recording…' });
  if (!(await again.isDisabled()) || !(await again.getAttribute('title'))?.startsWith('One import at a time')) fails.push('second Import not disabled with its reason');
  await p.locator('.openitems .oi input').first().click();
  await p.waitForTimeout(300);
  if (!(await p.evaluate(() => window.calls.includes('set_action_item_done')))) fails.push('ticking an item elsewhere did not save');
  // Back to the import, then Cancel.
  await p.getByRole('button', { name: /Quarterly review/ }).first().click();
  await cancel.click();
  if (!(await p.getByRole('button', { name: 'Stopping…' }).isDisabled())) fails.push('Stopping… is not shown, disabled');
  await p.getByText('Import cancelled. Your file is unchanged.').waitFor();
  await p.waitForTimeout(300);
  if (await p.locator('.processing').count()) fails.push('progress page still open');
  if (await p.getByRole('button', { name: /Quarterly review/ }).count()) fails.push('cancelled meeting still listed');
  if (await p.getByRole('button', { name: 'Import a recording…' }).isDisabled()) fails.push('Import still disabled after cancel');
  if (await p.locator('.banner[role=alert]').count()) fails.push('error: ' + await p.locator('.banner[role=alert]').innerText());
  await p.close();
  if (fails.length) throw Error(JSON.stringify(fails));
  return 'ok';
}
