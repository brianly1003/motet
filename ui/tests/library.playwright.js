// Run as a Playwright page function. IPC is mocked only in this browser fixture.
async (page) => {
  const results = [];
  const reference = await page.context().newPage();
  await reference.route('https://**/*', route => route.abort());
  await reference.goto('http://127.0.0.1:3001/motet-app-mockup.html');
  await reference.getByRole('button', {name: 'After the call', exact: true}).click();
  for (const theme of ['light', 'dark']) {
    const p = await page.context().newPage();
    const errors = [];
    p.on('pageerror', error => errors.push(error.message));
    await p.emulateMedia({colorScheme: theme});
    await p.setViewportSize({width: 760, height: 520});
    await p.addInitScript(() => {
      const meeting = {summary: {id: '123', title: 'Design review', created_ms: Date.now(), status: 'Ready', marks_ms: [4000]}, notes_md: '# Action items\n- Share the design before Thursday', transcript: '[00:04] Me: Publish the roadmap.'};
      let next = 0;
      let deleted = false, purged = false;
      Object.defineProperty(navigator, "clipboard", {value: {writeText: async text => { window.copiedText = text; }}});
      window.testCalls = [];
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener: () => {}};
      window.__TAURI_INTERNALS__ = {transformCallback: () => ++next, unregisterCallback: () => {}, invoke: async (command, args) => {
        window.testCalls.push({command, args});
        if (command === 'plugin:event|listen') return next;
        if (command === 'get_state') return {state: {type: 'Idle'}, meeting_id: null, companion: {visible: false, size: 'Pill'}, elapsed_ms: 0, quit_pending: false};
        if (command === 'notification_permission') return true;
        if (command === 'list_meetings') return deleted ? [] : [meeting.summary];
        if (command === 'deleted_meetings') return deleted && !purged ? [meeting.summary] : [];
        if (command === 'meeting_action') { if(args.action === 'delete') deleted = true; if(args.action === 'restore') deleted = false; if(args.action === 'purge') purged = true; return null; }
        if (command === 'get_meeting') return meeting;
        if (command === 'meeting_media') return {tracks:[],duration_ms:0,folder:'/local/meeting'};
        if (command === 'search_meetings') return JSON.stringify(meeting).toLowerCase().includes(args.query.trim().toLowerCase()) ? [meeting.summary] : [];
        return false;
      }};
    });
    await p.goto('http://127.0.0.1:3000/main/');
    await p.getByPlaceholder('Search notes and transcripts').fill('roadmap');
    await p.getByRole('button', {name: /Design review/}).click();
    if (await p.locator('.titlerow').getByRole('button', {name: 'Meetings', exact: true}).count()) throw Error('Obsolete Meetings button remains');
    await p.getByRole('button', {name: 'Copy ▾', exact: true}).click();
    await p.getByRole('menuitem', {name: /^Action items/}).click();
    if (await p.evaluate(() => window.copiedText) !== 'Share the design before Thursday') throw Error('Copied wrong action items');
    for (const label of ['Notes as Markdown', 'Notes as plain text']) {
      await p.getByRole('button', {name: 'Copy ▾', exact: true}).click();
      await p.getByRole('menuitem', {name: label, exact: true}).click();
      const copied = await p.evaluate(() => window.copiedText);
      if (!copied.includes('Share the design before Thursday') || (label === 'Notes as plain text' && copied.includes('#'))) throw Error('Wrong copy format');
    }
    await p.keyboard.press('Meta+Shift+c');
    if (await p.evaluate(() => window.copiedText) !== 'Share the design before Thursday') throw Error('Copy shortcut failed');
    await p.getByRole('button', {name: 'More actions', exact: true}).click();
    await reference.evaluate(theme => document.documentElement.setAttribute('data-theme', theme), theme);
    await reference.getByRole('button', {name: 'More actions', exact: true}).click();
    const properties = ['width', 'padding', 'gap', 'fontSize', 'borderRadius', 'backgroundColor', 'borderColor', 'backdropFilter'];
    const actual = await p.locator('.doc-dropdown').evaluate((e, keys) => Object.fromEntries(keys.map(k => [k, getComputedStyle(e)[k]])), properties);
    const expected = await reference.locator('.headacts .dropdown').evaluate((e, keys) => Object.fromEntries(keys.map(k => [k, getComputedStyle(e)[k]])), properties);
    for (const key of properties) if(actual[key] !== expected[key]) throw Error(`Menu ${key}: ${actual[key]} != ${expected[key]}`);
    await p.screenshot({path: `.playwright-out/parity-document-menu-${theme}.png`});
    await p.getByRole('menuitem', {name: 'Show in Finder', exact: true}).click();
    if (!await p.evaluate(() => window.testCalls.some(call => call.command === 'meeting_action' && call.args.action === 'reveal' && call.args.id === '123'))) throw Error('Reveal was not called');
    await p.getByRole('button', {name: 'More actions', exact: true}).click();
    await p.getByRole('menuitem', {name: 'Write the notes again', exact: true}).click();
    await p.getByRole('button', {name: 'Confirm', exact: true}).click();
    await p.getByRole('button', {name: 'More actions', exact: true}).click();
    await p.getByRole('menuitem', {name: 'Delete meeting…', exact: true}).click();
    await p.getByRole('button', {name: 'Confirm', exact: true}).click();
    await p.getByRole('button', {name: 'Restore', exact: true}).click();
    await p.getByRole('button', {name: /Design review/}).click();
    await p.getByRole('tab', {name: 'Transcript', exact: true}).click();
    await p.getByText('Publish the roadmap.').waitFor();
    await p.keyboard.press('Meta+k');
    await p.getByRole('dialog', {name: 'Command palette'}).waitFor();
    await p.getByRole('textbox', {name: 'Search meetings or commands'}).fill('roadmap');
    await p.getByRole('dialog').getByRole('button', {name: /Design review/}).waitFor();
    await p.screenshot({path: `.playwright-out/parity-palette-${theme}.png`});
    await p.keyboard.press('Enter');
    if (await p.getByRole('dialog').count()) throw Error('Palette did not close after choosing a meeting');
    await p.getByRole('button', {name: 'Command palette', exact: true}).click();
    await p.keyboard.press('Escape');
    await p.getByRole('button', {name: 'More actions', exact: true}).click();
    await p.getByRole('menuitem', {name: 'Delete meeting…', exact: true}).click();
    await p.getByRole('button', {name: 'Confirm', exact: true}).click();
    await p.getByRole('button', {name: 'Delete permanently', exact: true}).click();
    await p.getByRole('button', {name: 'Cancel', exact: true}).click();
    if (await p.evaluate(() => window.testCalls.some(call => call.args?.action === 'purge'))) throw Error('Cancel permanently deleted a meeting');
    await p.getByRole('button', {name: 'Delete permanently', exact: true}).click();
    await p.screenshot({path: `.playwright-out/permanent-delete-${theme}.png`});
    await p.getByRole('button', {name: 'Delete permanently', exact: true}).click();
    await p.getByRole('button', {name: 'Restore', exact: true}).waitFor({state: 'hidden'});
    if (!await p.evaluate(() => window.testCalls.some(call => call.args?.action === 'purge' && call.args.id === '123'))) throw Error('Permanent deletion not dispatched');
    const overflow = await p.evaluate(() => document.documentElement.scrollWidth > innerWidth);
    if (overflow || errors.length) throw Error(JSON.stringify({overflow, errors}));
    results.push({theme, search: 'transcript match opened', palette: 'keyboard, click, selection and Escape passed', overflow, errors});
    await p.close();
  }
  await reference.close();
  return results;
}
