// Native geometry recovery is covered by companion::tests; this fixture covers pointer capture and mockup interactions.
async (page) => {
  const fails = [];
  const p = await page.context().newPage(); p.on('pageerror', e => fails.push(e.message));
  await p.setViewportSize({ width: 200, height: 260 });
  await p.emulateMedia({reducedMotion:'reduce'});
  await p.addInitScript(() => {
    let next = 0; const callbacks = {}, listeners = {};
    let state = { state: { type: 'Recording' }, meeting_id: 'm1', app: 'Microsoft Teams', elapsed_ms: 56000, started_by: { type: 'App', app: 'Microsoft Teams' }, ask_on_call: true, problem: null, more_problems: 0, quit_pending: false, dnd_active: false, onboarding_done: true, sharing: null, ask_off_pending: null, stops_at_ms: null,
      companion: { visible: true, size: 'Tucked', pinned: false, peeking: false, display_id: '' } };
    window.calls = [];
    window.fixtureState = value => { state = { ...state, ...value }; callbacks[listeners['motetto://event']]?.({ payload: { type: 'StateChanged', ...state } }); };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'companion' }, currentWebview: { windowLabel: 'companion', label: 'companion' } },
      transformCallback: cb => { callbacks[++next] = cb; return next; }, unregisterCallback: id => delete callbacks[id],
      invoke: async (c, a) => {
        window.calls.push(c);
        if (c === 'plugin:event|listen') { listeners[a.event] = a.handler; return a.handler; }
        if (c === 'get_state') return state; if (c === 'get_levels') return [0, 0];
        if (c === 'live_transcript') return ['m1', '', '']; if (c === 'get_my_notes') return { meeting_id: 'm1', text: '', version: 1 };
        if (c === 'pause_recording') { window.fixtureState({ state: { type: 'Paused' } }); return null; }
        if (c === 'resume_recording') { window.fixtureState({ state: { type: 'Recording' } }); return null; }
        return null; } };
  });
  await p.goto('http://127.0.0.1:3000/companion/');
  await p.locator('.handle').waitFor();
  const h = await p.locator('.handle').boundingBox();
  await p.mouse.move(h.x+h.width/2,h.y+h.height/2); await p.mouse.down();
  await p.mouse.move(10,240); await p.mouse.up();
  await p.waitForFunction(()=>window.calls.includes('snap_companion'),null,{timeout:5000}).catch(async e=>{throw Error(e.message+' '+JSON.stringify(await p.evaluate(()=>window.calls)));});
  const calls=await p.evaluate(()=>window.calls);
  if (calls.filter(c=>c.includes('start_dragging')).length!==1 || calls.filter(c=>c==='snap_companion').length!==1) throw Error('Fast drag must start once and request an edge snap');
  if (calls.includes('set_companion_size')) throw Error('Dragging opened the Sidecar');
  if (fails.length) throw Error(fails.join('\n'));
  await p.close();
 const errors=[]; page.on('pageerror',e=>errors.push(e.message));
 await page.route('https://**/*',r=>r.abort());
 await page.setViewportSize({width:1400,height:1000});
 await page.goto('http://127.0.0.1:3001/motetto-app-mockup.html');
 await page.getByRole('button',{name:'macOS',exact:true}).click();
 await page.getByRole('button',{name:'During a call',exact:true}).click();
 await page.locator('#caption').click();
 await page.getByRole('button',{name:'Tuck to the screen edge',exact:true}).click();
 await page.emulateMedia({reducedMotion:'reduce'});
 for (const theme of ['light','dark']) {
  await page.evaluate(t=>document.documentElement.dataset.theme=t,theme);
  for (const target of [{x:1800,y:-50},{x:-500,y:1500}]) {
   const h=await page.locator('#pill .handle').boundingBox();
   await page.mouse.move(h.x+h.width/2,h.y+h.height/2); await page.mouse.down();
   await page.mouse.move(target.x,target.y);
   const safe=await page.locator('#pill').evaluate(e=>{const r=e.getBoundingClientRect(),a=document.querySelector('#stage').getBoundingClientRect(); return r.left>=a.left+9 && r.right<=a.right-9 && r.top>=a.top+43 && r.bottom<=a.bottom-15;});
   if (!safe) throw Error('Fast swipe went outside stage');
   await page.mouse.up();
   if (!await page.locator('#pill[data-size="tucked"]').isVisible()) throw Error('Tucked disappeared');
  }
  await page.locator('#pill').screenshot({path:`.playwright-out/tucked-edge-${theme}.png`});
 }
 await page.getByRole('button',{name:'Stop',exact:true}).first().click();
 if (await page.locator('#pill').isVisible()) throw Error('Stopped companion still visible');
 await page.locator('.recbar [data-act="resume"]').waitFor();
 await page.locator('.recbar').screenshot({path:'.playwright-out/stopped-main-only.png'});
 await page.locator('.recbar [data-act="resume"]').click();
 if (!await page.locator('#pill').isVisible()) throw Error('Resume did not restore companion');
 if (errors.length) throw Error(errors.join('\n'));
 return 'PASS: fast swipes to both edges, light/dark, Stop hides companion, main Resume restores it';
}
