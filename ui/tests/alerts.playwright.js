// Real alert rendering and actions with local IPC fixtures; no recording is started.
async (page) => {
  const results=[];
  const reference=await page.context().newPage();
  await reference.route('https://**/*',r=>r.abort());
  await reference.goto('http://127.0.0.1:3001/motet-app-mockup.html');
  await reference.getByRole('button',{name:'After the call',exact:true}).click();
  if(await reference.locator('#pill').isVisible())throw Error('Inactive library shows a recording companion');
  await reference.getByText('Action items',{exact:true}).first().click();
  for (const theme of ['light','dark']) {
    const p=await page.context().newPage(); const errors=[];p.on('pageerror',e=>errors.push(e.message));
    await p.emulateMedia({colorScheme:theme});
    await reference.evaluate(t=>document.documentElement.dataset.theme=t,theme);
    await p.addInitScript(()=>{
      let next=0;window.testCalls=[];
      window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
      window.__TAURI_INTERNALS__={transformCallback:()=>++next,unregisterCallback:()=>{},invoke:async(command,args)=>{
        window.testCalls.push({command,args});
        if(command==='plugin:event|listen')return next;
        if(command==='get_state')return {state:{type:'Idle'},meeting_id:null,companion:{visible:false,size:'Pill'},elapsed_ms:0,quit_pending:false,problem:{id:1,since:'fixture',problem:{type:'DiskLow',free_bytes:1000000}}};
        if(command==='notification_permission')return true;
        if(command==='list_meetings'||command==='deleted_meetings')return [];
        return null;
      }};
    });
    await p.goto('http://127.0.0.1:3000/main/');
    const alert=p.locator('.banner.alert[role="alert"]').first();await alert.waitFor();
    for (const width of [760,1200]) {
      await p.setViewportSize({width,height:720});await reference.setViewportSize({width,height:1000});
      const read=e=>{const s=getComputedStyle(e);return {border:[s.borderTopWidth,s.borderRightWidth,s.borderBottomWidth,s.borderLeftWidth],padding:s.padding,radius:s.borderRadius,background:s.backgroundColor,color:s.color};};
      const a=await alert.evaluate(read),b=await reference.locator('.banner.alert').first().evaluate(read);
      if(JSON.stringify(a)!==JSON.stringify(b))throw Error('App/mockup alert styles differ: '+JSON.stringify({a,b}));
      if(a.border.some(v=>v!=='1px'))throw Error('Uneven alert border');
      const layout=await alert.evaluate(e=>{const a=e.getBoundingClientRect();return [...e.querySelectorAll('svg,.alert-content,.alert-actions,button')].every(c=>{const r=c.getBoundingClientRect();return r.left>=a.left && r.right<=a.right+.5 && r.bottom<=a.bottom;}) && e.scrollWidth<=e.clientWidth;});
      if(!layout)throw Error('Alert content overflows');
      for(const selector of ['.alert-icon','.alert-title','.alert-description','.alert-actions']){
        const styles=el=>{const s=getComputedStyle(el);return [s.fontSize,s.fontWeight,s.lineHeight,s.color,s.gap];};
        const actual=await alert.locator(selector).evaluate(styles),expected=await reference.locator('.banner.alert').first().locator(selector).evaluate(styles);
        // Icons inherit their surrounding page font; their own 16px geometry is explicit.
        if(selector!=='.alert-icon' && JSON.stringify(actual)!==JSON.stringify(expected))throw Error(selector+' parity '+JSON.stringify({actual,expected}));
      }
      await alert.screenshot({path:`.playwright-out/alert-app-${theme}-${width}.png`});
      await reference.locator('.banner.alert').first().screenshot({path:`.playwright-out/alert-mockup-${theme}-${width}.png`});
    }
    await alert.getByRole('button',{name:'Free up space',exact:true}).click();
    await alert.getByRole('button',{name:'Dismiss',exact:true}).click();
    const calls=await p.evaluate(()=>window.testCalls);
    if(!calls.some(c=>c.command==='open_settings'&&c.args.tab==='privacy')||!calls.some(c=>c.command==='dismiss_problem'))throw Error('Alert actions stopped working: '+JSON.stringify(calls));
    if(errors.length)throw Error(errors.join('\n'));
    await p.close();results.push(theme+' PASS');
  }
  await reference.getByRole('button',{name:'Recover',exact:true}).click();
  if(await reference.locator('.banner.alert').count())throw Error('Recovery action did not dismiss banner');
  await reference.close();return results;
}
