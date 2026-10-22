// Mockup-first document find: keyboard, highlights and unmodified editable content.
async (page) => {
 const results=[];
 for(const theme of ['light','dark']){
  const p=await page.context().newPage();p.setDefaultTimeout(7000);
  const errors=[];p.on('pageerror',e=>errors.push(e.message));
  await p.route('https://**/*',r=>r.abort());
  await p.goto('http://127.0.0.1:3001/motetto-app-mockup.html');
  await p.getByRole('button',{name:'macOS',exact:true}).click();
  await p.getByRole('button',{name:'After the call',exact:true}).click();
  await p.evaluate(t=>document.documentElement.dataset.theme=t,theme);
  for(const width of [760,1200]){
   await p.setViewportSize({width,height:1000});
   await p.locator('.mainwin').evaluate((e,width)=>{e.style.width=width+'px';e.style.left='0';},width);
   for(const tab of ['Notes','Transcript']){
    await p.getByRole('tab',{name:tab,exact:true}).click();
    const scope=tab==='Notes'?'#notesText':'.tlist';
    const content=()=>p.locator(scope).evaluate(e=>{const copy=e.cloneNode(true);copy.querySelectorAll('[style=""]').forEach(e=>e.removeAttribute('style'));return copy.innerHTML;});
    const before=await content();
    const topBefore=await p.locator('.docbody').evaluate(e=>e.getBoundingClientRect().top);
    await p.keyboard.press('Meta+f');
    const layout=await p.evaluate(()=>{const tabs=document.querySelector('.tabs').getBoundingClientRect(),find=document.querySelector('.findbar').getBoundingClientRect(),tools=document.querySelector('.doctools').getBoundingClientRect();return {tabs:{left:tabs.left,right:tabs.right,top:tabs.top,bottom:tabs.bottom},find:{left:find.left,right:find.right,top:find.top,bottom:find.bottom},right:tools.right,overflow:document.querySelector('.findbar').scrollWidth>document.querySelector('.findbar').clientWidth};});
    if(layout.find.right-layout.find.left>280.5)throw Error('Find is wider than the compact 280px limit');
    if(layout.find.left<layout.tabs.right||layout.find.top>=layout.tabs.bottom||layout.find.bottom<=layout.tabs.top||layout.find.right>layout.right||layout.overflow)throw Error('Find does not fit beside tabs: '+JSON.stringify(layout));
    if(await p.locator('.docbody').evaluate(e=>e.getBoundingClientRect().top)!==topBefore)throw Error('Opening find consumed vertical space');
    const input=p.getByRole('textbox',{name:'Find in '+tab.toLowerCase(),exact:true});
    await input.fill('FCM');
    await p.waitForFunction(()=>CSS.highlights.get('motetto-find')?.size>1);
    const count=await p.evaluate(()=>CSS.highlights.get('motetto-find').size);
    if(await p.locator('#findCount').innerText()!==`${count} matches`)throw Error('Wrong initial count');
    await input.press('Enter');
    if(await p.locator('#findCount').innerText()!==`1 of ${count}`)throw Error('Enter did not jump to first');
    await input.press('Shift+Enter');
    if(await p.locator('#findCount').innerText()!==`${count} of ${count}`)throw Error('Previous did not wrap');
    const rect=await p.evaluate(()=>{const r=[...CSS.highlights.get('motetto-find-current')][0].getBoundingClientRect(),b=document.querySelector('.docbody').getBoundingClientRect();return {top:r.top,bottom:r.bottom,min:b.top,max:b.bottom};});
    if(rect.top<rect.min||rect.bottom>rect.max)throw Error('Active result not scrolled into view: '+JSON.stringify(rect));
    await p.getByRole('button',{name:'Next match',exact:true}).click();
    if(await p.locator('#findCount').innerText()!==`1 of ${count}`)throw Error('Next button did not wrap');
    await input.fill('fcm');await input.press('Enter');
    if(await p.evaluate(()=>CSS.highlights.get('motetto-find').size)!==count)throw Error('Case-insensitive search changed count');
    await p.keyboard.press('Control+f');
    if(!await input.evaluate(e=>e===document.activeElement&&e.selectionEnd===e.value.length&&e.selectionStart===0))throw Error('Ctrl+F did not select query');
    await p.locator('.doctools').screenshot({path:`.playwright-out/find-${tab}-${theme}-${width}.png`});
    if(width===1200)await p.locator('.doc').screenshot({path:`.playwright-out/find-document-${tab}-${theme}.png`});
    if(tab==='Notes'){
     await input.fill('keep FCM');await input.press('Enter');
     if(await p.locator('#findCount').innerText()!=='1 of 1')throw Error('Phrase across inline formatting not found');
    }
    await input.fill('[not a regex]*');await input.press('Enter');
    if(await p.locator('#findCount').innerText()!=='No matches')throw Error('No matches state missing');
    if(!await p.getByRole('button',{name:'Next match',exact:true}).isDisabled())throw Error('No matches still navigable');
    await input.fill('');if(await p.locator('#findCount').innerText()!=='')throw Error('Empty query should clear');
    await input.fill('FCM');await input.press('Enter');await input.press('Escape');
    if(await p.locator('.findbar').count()||await p.evaluate(()=>CSS.highlights.has('motetto-find')))throw Error('Escape left find visible');
    await p.keyboard.press('Meta+f');
    if(await input.inputValue()!=='')throw Error('Escape retained the search query');
    await input.fill('FCM');await p.getByRole('button',{name:'Close find',exact:true}).click();
    await p.keyboard.press('Meta+f');
    if(await input.inputValue()!=='')throw Error('Close button retained the search query');
    await input.press('Escape');
    const after=await content();if(after!==before){let i=0;while(before[i]===after[i])i++;throw Error('Find changed editable content '+tab+' at '+i+': '+before.slice(i-60,i+180)+' -> '+after.slice(i-60,i+180));}
   }
  }
  await p.keyboard.press('Meta+f');await p.getByRole('textbox',{name:'Find in transcript',exact:true}).fill('FCM');
  await p.getByRole('tab',{name:'Audio',exact:true}).click();
  if(await p.locator('.findbar').count()||await p.evaluate(()=>CSS.highlights.has('motetto-find')))throw Error('Tab switch left stale highlights');
  await p.getByRole('tab',{name:'Notes',exact:true}).click();await p.keyboard.press('Meta+f');
  await p.getByRole('button',{name:'Close find',exact:true}).click();
  if(await p.locator('.findbar').count())throw Error('Close button failed');
  await p.keyboard.press('Meta+f');await p.locator('.findbar input').fill('FCM');
  await p.locator('.side .mitem').nth(1).click();
  if(await p.locator('.findbar').count()||await p.evaluate(()=>CSS.highlights.has('motetto-find')))throw Error('Meeting switch left stale search');
  await p.locator('.side .mitem[data-id="fcm"]').click();await p.keyboard.press('Meta+f');
  if(await p.locator('.findbar input').inputValue()!=='')throw Error('Meeting switch retained query');
  await p.locator('.findbar input').fill('FCM');await p.locator('#openHome').click();
  if(await p.locator('.findbar').count()||await p.evaluate(()=>CSS.highlights.has('motetto-find')))throw Error('Action items left stale search');
  await p.locator('.side .mitem[data-id="fcm"]').click();await p.keyboard.press('Meta+f');
  if(await p.locator('.findbar input').inputValue()!=='')throw Error('Action items retained query');
  await p.keyboard.press('Escape');
  if(errors.length)throw Error(errors.join('\n'));
  await p.close();results.push(theme+' PASS');
 }
 return results;
}
