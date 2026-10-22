// Write the notes again: Confirm closes the question even when it fails (owner 2026-10-21: the
// warning stayed open under the error alert).
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 1200, height: 720 });
  await p.addInitScript(() => {
    const summary = { id: '1', title: 'Again', created_ms: Date.now(), status: 'Ready', marks_ms: [], duration_ms: 60000, notes_ms: 8000, spellings: [], locked: false, title_source: 'placeholder' };
    const meeting = { summary, transcript: '[00:04] Me: Hello', transcript_heard: '', notes_md: '### Action items\n- None\n' };
    let next = 0;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = { convertFileSrc: x => x, transformCallback: () => ++next, unregisterCallback: () => {}, invoke: async (c, a) => {
      if (c === 'plugin:event|listen') return next; if (c === 'get_state') return { state: { type: 'Idle' }, meeting_id: null, companion: { visible: false, size: 'Pill' }, elapsed_ms: 0, quit_pending: false };
      if (c === 'list_meetings') return [summary]; if (c === 'deleted_meetings') return []; if (c === 'get_meeting') return meeting;
      if (c === 'meeting_action' && a.action === 'regenerate') throw 'Cannot prepare new notes: File exists (os error 17)';
      if (c === 'meeting_media') return { tracks: [], duration_ms: 60000, folder: 'x' }; return false; } };
  });
  await p.goto('http://127.0.0.1:3000/main/');
  await p.getByRole('button', { name: /Again/ }).first().click();
  await p.getByRole('button', { name: 'More actions' }).click();
  await p.getByRole('menuitem', { name: 'Write the notes again' }).click();
  await p.getByRole('alertdialog').getByRole('button', { name: 'Confirm' }).click();
  await p.getByText('File exists (os error 17)').waitFor();
  if (await p.getByRole('alertdialog').count()) fails.push('the question stays open after a failed Confirm');
  await p.close();
  return fails.length ? fails : 'PASS';
}
