async (page)=>{
 const results=[];
 const ref=await page.context().newPage();await ref.route('https://**/*',r=>r.abort());
 await ref.goto('http://127.0.0.1:3001/motet-app-mockup.html');
 await ref.getByRole('button',{name:'macOS',exact:true}).click();await ref.getByRole('button',{name:'After the call',exact:true}).click();
 for(const theme of ['light','dark']){
 const p=await page.context().newPage();p.setDefaultTimeout(7000);await p.emulateMedia({colorScheme:theme});
 await ref.evaluate(t=>document.documentElement.dataset.theme=t,theme);
    await p.addInitScript(() => {
      const meeting = {summary: {id: '123', title: 'Design review', created_ms: Date.now(), status: 'Ready', marks_ms: [4000], duration_ms: 60000}, notes_md: '# Action items\n- Share the design before Thursday', transcript: '[00:04] Me: Publish the roadmap.'};
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
        if (command === 'meeting_media') return {tracks:[],duration_ms:60000,folder:'/local/meeting'};
        if (command === 'search_meetings') return JSON.stringify(meeting).toLowerCase().includes(args.query.trim().toLowerCase()) ? [meeting.summary] : [];
        return false;
      }};
    });

 await p.goto('http://127.0.0.1:3000/main/');await p.getByRole('button',{name:/Design review/}).first().waitFor();
 await p.keyboard.press('Meta+k');await ref.keyboard.press('Meta+k');
 for(const width of [760,1200]){
  await p.setViewportSize({width,height:720});await ref.setViewportSize({width,height:1000});
  for(const selector of ['.palette','.palette input','.palette .k','.palette .res button','.palette .res .tip']){
   const styles=e=>{const s=getComputedStyle(e);return [s.fontSize,s.fontWeight,s.lineHeight,s.color,s.borderRadius,s.padding,s.boxShadow];};
   const a=await p.locator(selector).first().evaluate(styles),b=await ref.locator(selector).first().evaluate(styles);
   if(JSON.stringify(a)!==JSON.stringify(b))throw Error(selector+' '+JSON.stringify({a,b}));
  }
  if(await p.locator('.palette input').evaluate(e=>getComputedStyle(e).boxShadow)!=='none')throw Error('Command search has yellow focus');
  await p.locator('.palette').screenshot({path:`.playwright-out/ui-palette-${theme}-${width}.png`});
 }
 await p.keyboard.press('Escape');await ref.keyboard.press('Escape');
 await p.getByRole('button',{name:/Design review/}).first().click();await p.getByRole('tab',{name:'Transcript',exact:true}).click();
 const tab=p.getByRole('tab',{name:'Transcript',exact:true});await tab.focus();await p.keyboard.press(' ');
 if(!/Pause/.test(await p.locator('.player button').innerText()))throw Error('Space on Transcript tab did not play');
 await p.keyboard.press(' ');
 if(!/Play/.test(await p.locator('.player button').innerText()))throw Error('Space on Transcript tab did not pause');
 await ref.getByRole('tab',{name:'Transcript',exact:true}).click();const rt=ref.getByRole('tab',{name:'Transcript',exact:true});await rt.focus();await ref.keyboard.press(' ');
 if(!/Pause/.test(await ref.locator('#playBtn').innerText()))throw Error('Mockup Space did not play');
 await ref.keyboard.press(' ');
 for(const [target,button] of [[p,tab],[ref,rt]]){
  await button.click();await target.keyboard.press('ArrowDown');
  const current=await target.evaluate(()=>document.activeElement?.closest('.tline')?.dataset.line);
  if(current===undefined)throw Error('↓ from clicked Transcript tab did not enter a line');
  const last=await target.locator('.tline').count()-1;
  await target.keyboard.press('ArrowDown');
  if(await target.evaluate(()=>Number(document.activeElement?.closest('.tline')?.dataset.line))!==Math.min(last,Number(current)+1))throw Error('↓ did not advance a line');
  await button.click();await target.keyboard.press('ArrowUp');
  if(await target.evaluate(()=>document.activeElement?.closest('.tline')?.dataset.line)===undefined)throw Error('↑ from clicked Transcript tab did not enter a line');
 }
 await p.locator('.tabs').screenshot({path:`.playwright-out/ui-tabs-${theme}.png`});
 for(const target of [tab,rt]){const s=await target.evaluate(e=>({o:getComputedStyle(e).outlineStyle,s:getComputedStyle(e).boxShadow}));if(s.o!=='none'||s.s!=='none')throw Error('Tab focus has a box');}
 await p.close();results.push(theme+' PASS');
 }
 await ref.close();return results;
}
