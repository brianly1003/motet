async (page) => {
  const results=[];
  for (const theme of ['light','dark']) {
    const p=await page.context().newPage();
    await p.emulateMedia({colorScheme:theme});await p.setViewportSize({width:760,height:520});
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
        if (command === 'rename_meeting') { meeting.summary = {...meeting.summary, title: args.title}; return meeting.summary; }
        if (command === 'get_meeting') return meeting;
        if (command === 'meeting_media') return {tracks:[],duration_ms:0,folder:'/local/meeting'};
        if (command === 'search_meetings') return JSON.stringify(meeting).toLowerCase().includes(args.query.trim().toLowerCase()) ? [meeting.summary] : [];
        return false;
      }};
    });
    await p.goto('http://127.0.0.1:3000/main/');
    await p.getByRole('button',{name:/Design review/}).first().click();
    await p.getByRole('button',{name:'More actions',exact:true}).click();
    await p.getByRole('menuitem',{name:'Delete meeting',exact:true}).click();
    const toast=p.locator('.toast');await toast.waitFor();
    const close=toast.getByRole('button',{name:'Dismiss notification',exact:true});
    const box=await close.boundingBox();
    // Owner 2026-10-21: a 20 px circle centred near the toast's top-left corner, like the Ask card's ✕.
    const tb=await toast.boundingBox();
    if(box.width!==20||box.height!==20||await close.evaluate(e=>getComputedStyle(e).borderRadius)!=='50%')throw Error('Close control is not a 20 px circle');
    if(Math.abs(box.x+10-(tb.x+2))>1||Math.abs(box.y+10-(tb.y+2))>1)throw Error(`Close control is not on the top-left corner: ${JSON.stringify({box,tb})}`);
    // The 28 px hit area: a click 3 px outside the visible circle still dismisses.
    if(await p.evaluate(([x,y])=>!document.elementFromPoint(x,y)?.closest('.toast-close'),[box.x-3,box.y+10]))throw Error('Close target is smaller than 28 px');
    if(!await toast.getByRole('button',{name:'Undo',exact:true}).isVisible())throw Error('Undo lost');
    await p.screenshot({path:`.playwright-out/toast-close-${theme}.png`,fullPage:true,clip:{x:tb.x-16,y:tb.y-16,width:tb.width+32,height:tb.height+32}});
    await close.focus();await p.keyboard.press('Enter');
    if(await toast.count())throw Error('Toast not dismissed');
    if(await p.evaluate(()=>window.testCalls.some(c=>c.command==='meeting_action'&&c.args.action==='restore')))throw Error('Dismiss incorrectly invoked Undo');
    await p.close();results.push(theme+' PASS');
  }
  const mock=await page.context().newPage();await mock.route('https://**/*',r=>r.abort());
  await mock.goto('http://127.0.0.1:3001/motet-app-mockup.html');
  await mock.getByRole('button',{name:'After the call',exact:true}).click();
  await mock.getByText('Action items',{exact:true}).first().click();
  await mock.getByRole('button',{name:'Recover',exact:true}).click();
  await mock.locator('#toast:not(.off)').waitFor();
  // The app's close control matches the mockup's (both themes are set by the app runs above; light here).
  const mstyle=await mock.locator('#toastClose').evaluate(e=>{const c=getComputedStyle(e);return [c.width,c.height,c.left,c.top,c.borderRadius,c.borderWidth,c.position].join(' ');});
  if(mstyle!=='20px 20px -8px -8px 50% 1px absolute')throw Error('Mockup close control: '+mstyle);
  await mock.waitForTimeout(300); // the toast fades in over .2 s
  const mb=await mock.locator('#toast').boundingBox();await mock.screenshot({path:'.playwright-out/toast-close-mockup.png',fullPage:true,clip:{x:mb.x-16,y:mb.y-16,width:mb.width+32,height:mb.height+32}});
  await mock.getByRole('button',{name:'Dismiss notification',exact:true}).click();
  if(!await mock.locator('#toast').evaluate(e=>e.classList.contains('off')))throw Error('Mockup toast not dismissed');
  await mock.close();return results;
}
