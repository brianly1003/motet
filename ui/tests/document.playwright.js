// Browser-only IPC fixture; storage, file validation and waveform sampling have Rust tests.
async (page) => {
  const reference=await page.context().newPage();
  await reference.route('https://**/*',route=>route.abort());
  await reference.setViewportSize({width:1200,height:900});
  await reference.goto('http://127.0.0.1:3001/motet-app-mockup.html');
  await reference.getByRole('button',{name:'macOS',exact:true}).click();
  await reference.getByRole('button',{name:'After the call',exact:true}).click();
  const results=[];
  for(const theme of ['light','dark']) {
    const p=await page.context().newPage();
    const errors=[];
    p.on('pageerror',e=>errors.push(e.message));
    await p.emulateMedia({colorScheme:theme});
    await reference.evaluate(theme=>document.documentElement.setAttribute('data-theme',theme),theme);
    await p.setViewportSize({width:1046,height:662});
    // Silent, valid local WAV exercises actual HTML audio playback without private recordings.
    const wav=Buffer.alloc(44+8000*87*2);
    wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(8000,24);wav.writeUInt32LE(16000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
    await p.route('**/fixture*.wav',route=>{
      const range=/bytes=(\d+)-(\d*)/.exec(route.request().headers().range??'');
      const start=range?Number(range[1]):0, end=range&&range[2]?Math.min(Number(range[2]),wav.length-1):wav.length-1;
      return route.fulfill({status:range?206:200,contentType:'audio/wav',headers:{'Accept-Ranges':'bytes','Content-Length':String(end-start+1),...(range?{'Content-Range':`bytes ${start}-${end}/${wav.length}`}:{})},body:wav.subarray(start,end+1)});
    });
    await p.addInitScript(()=>{
      Object.defineProperty(navigator,'platform',{value:'MacIntel'});
      const summary={id:'123',title:'FCM vs Pusher Beams',created_ms:Date.now(),status:'Ready',marks_ms:[74000],duration_ms:87000,notes_ms:48000,spellings:['FCM','IGB']};
      const meeting={summary,notes_md:'### Action items\n- [ ] You: Share the device table design before Thursday [01:20]\n- [ ] Kevin: Check whether the 2021 Firebase project is still active [00:56]\n- [x] Book a follow-up on the IGB notification table [01:14]\n- [ ] You: Ask Kevin about the Firebase decision [01:01]\n\n### Overview\n- Compared **FCM** with push beams for Android push in the IGB app.\n- Decided to **keep FCM** and build device management ourselves.\n\n### Push beams on top of FCM\n- Pusher beams relies on **FCM** for Android delivery and adds device management and reporting. [00:16]\n- Its device management is hard to fit into the current **IGB back office** flows. [00:36]\n\n### Cost\n- **FCM has no messaging fee.** [01:00]\n\n### Open questions\n- Is the 2021 Firebase project still usable, or do we need a new account?\n\n<details><summary>Transcript</summary>\n\n[00:16] Me: Pusher beams relies on FCM.\n\n</details>\n\n### My notes\n\n[01:01] Kevin decides on Firebase.',transcript:'[00:16] Me: Pusher beams relies on FCM.\n[00:36] Them: Existing back office flows.\n[00:56] Them: Check the Firebase project.\n[01:00] Me: FCM has no messaging fee.\n[01:14] Them: Book a follow-up.\n[01:20] Me: I will share the device table design.'};
      const tracks=['You','Them'].map((speaker,index)=>({path:`/fixture${index}.wav`,name:index?'system.wav':'mic.wav',speaker,offset_ms:0,duration_ms:87000,bytes:2100000,waveform:Array.from({length:200},(_,i)=>i%40<(index?10:30)?.65:0)}));
      let next=0;window.testCalls=[];
      window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
      window.__TAURI_INTERNALS__={convertFileSrc:path=>path,transformCallback:()=>++next,unregisterCallback:()=>{},invoke:async(command,args)=>{
        window.testCalls.push({command,args});
        if(command==='plugin:event|listen')return next;
        if(command==='get_state')return {state:{type:'Idle'},meeting_id:null,companion:{visible:false,size:'Pill'},elapsed_ms:0,quit_pending:false};
        if(command==='notification_permission')return true;
        if(command==='list_meetings')return [summary,{...summary,id:'124',title:'Earlier meeting',created_ms:0}];
        if(command==='deleted_meetings')return [];
        if(command==='get_meeting')return meeting;
        if(command==='meeting_media')return {tracks,duration_ms:87000,folder:'~/Documents/Motet/imports/123'};
        if(command==='set_action_item_done'){
          if(args.expected!==meeting.notes_md)throw Error('Stale notes');
          const lines=meeting.notes_md.split('\n');lines[args.line]=lines[args.line].replace(/\[[ x]\]/,args.done?'[x]':'[ ]');meeting.notes_md=lines.join('\n');return meeting.notes_md;
        }
        return false;
      }};
    });
    await p.goto('http://127.0.0.1:3000/main/');
    await p.getByRole('button',{name:/FCM vs Pusher Beams/}).first().click();
    await p.getByText('01:27',{exact:true}).first().waitFor();
    await p.getByRole('checkbox').first().check();
    await p.waitForFunction(()=>window.testCalls.some(c=>c.command==='set_action_item_done'&&c.args.done));
    await p.getByRole('checkbox').first().uncheck();
    const differences=[];
    const pairs=[['.dochead','.dochead'],['.document-meta','.meta'],['.docbody','.docbody'],['.notes','.notes'],['.notes h3','.notes h3'],['.notes h4','.notes h4'],['.todo','.todo'],['.todo input','.todo input'],['.owner','.owner'],['.src','.src'],['.meeting-aside','.aside'],['.panel','.panel'],['.panel h5','.panel h5'],['.kv','.kv'],['.tl','.tl'],['.tl svg','.tl svg'],['.tl-legend','.tl-legend'],['.notes li:has(.jotmark)','.notes li:has(.jotmark)'],['.jotmark','.jotmark'],['.notes li:has(.jotmark) .tip','.notes li:has(.jotmark) .tip']];
    // Labels stay labels wherever they fall (owner 2026-10-20); topics take the serif heading.
    const levels=await p.locator('.notes section > :is(h3,h4)').evaluateAll(hs=>hs.map(h=>`${h.tagName}:${h.textContent}`).join('|'));
    if(levels!=='H3:Action items|H3:Overview|H4:Push beams on top of FCM|H4:Cost|H3:Open questions|H3:My notes')differences.push(`headings: ${levels}`);
    if(await p.locator('.notes li:has(.jotmark)').innerText()!=='✎Kevin decides on Firebase. 01:01')differences.push('My notes line is not a jot');
    const properties=['padding','gap','fontSize','fontWeight','lineHeight','borderRadius','backgroundColor','color','borderColor','display'];
    for(const [actualSelector,sourceSelector] of pairs){
      const actual=await p.locator(actualSelector).first().evaluate((e,props)=>Object.fromEntries(props.map(k=>[k,getComputedStyle(e)[k]])),properties);
      const expected=await reference.locator('#doc '+sourceSelector).first().evaluate((e,props)=>Object.fromEntries(props.map(k=>[k,getComputedStyle(e)[k]])),properties);
      for(const key of properties)if(actual[key]!==expected[key])differences.push(`${actualSelector} ${key}: ${actual[key]} != ${expected[key]}`);
    }
    await p.screenshot({path:`.playwright-out/document-full-${theme}.png`});
    await reference.locator('.mainwin').screenshot({path:`.playwright-out/document-reference-${theme}.png`});
    await p.evaluate(() => {
      window.audioSeeks=[0,0];
      document.querySelectorAll('audio').forEach((audio,index)=>{
        audio.playbackRate=.5;
        audio.addEventListener('seeking',()=>window.audioSeeks[index]++);
      });
    });
    // A time the user typed (01:01, no transcript line) is a chip like a transcript time (owner 2026-10-20).
    if(!await p.locator('.todo .src',{hasText:'01:01'}).count())differences.push('My notes time is not a chip');
    if(await p.locator('.src').first().getAttribute('title')!=='Play from 01:20')differences.push('Time chip lacks the mockup tooltip');
    await p.locator('.src').first().click();
    // As in the mockup, a time in the notes opens the Transcript and plays from that line.
    await p.waitForFunction(()=>document.getElementById('tab-Transcript')?.getAttribute('aria-selected')==='true');
    await p.waitForFunction(()=>document.querySelector('.tline.playing')?.textContent.includes('share the device table design'));
    await p.waitForFunction(()=>document.querySelector('audio')?.currentTime>=80);
    await p.waitForFunction(()=>[...document.querySelectorAll('audio')].every(a=>!a.seeking&&!a.paused));
    await p.evaluate(()=>{window.audioSeeks=[0,0];});
    await p.waitForTimeout(1200);
    const playback=await p.evaluate(()=>({seeks:window.audioSeeks,position:Number(document.querySelector('.timeline-wave input').value)/1000,audio:document.querySelector('audio').currentTime}));
    if(playback.seeks[0]!==0 || Math.abs(playback.position-playback.audio)>.15)throw Error(`Playback chases wall clock: ${JSON.stringify(playback)}`);
    await p.getByRole('button',{name:'❚❚ Pause',exact:true}).click();
    await p.evaluate(()=>document.querySelectorAll('audio').forEach(a=>a.playbackRate=1));
    await p.getByRole('tab',{name:'Notes',exact:true}).click();
    if(await p.getByRole('checkbox').first().isChecked())throw Error('Source chip toggled task');
    // Clicking the voice wave selects Transcript and plays from that point, as in the mockup.
    const wave=await p.locator('.timeline-wave input').boundingBox();
    await p.mouse.click(wave.x+wave.width*.25,wave.y+wave.height/2);
    await p.waitForFunction(()=>document.getElementById('tab-Transcript')?.getAttribute('aria-selected')==='true');
    await p.waitForFunction(()=>{const t=document.querySelector('audio')?.currentTime;return t>=18&&t<26;});
    await p.getByRole('button',{name:'❚❚ Pause',exact:true}).click();
    await p.getByRole('tab',{name:'Notes',exact:true}).click();
    await p.getByRole('tab',{name:'Transcript',exact:true}).click();
    await p.getByRole('button',{name:'Play from 00:16',exact:true}).click();
    await p.waitForFunction(()=>document.querySelector('audio')?.currentTime>=16&&document.querySelector('audio')?.currentTime<20);
    await p.waitForFunction(()=>document.querySelector('.tline.playing')?.textContent.includes('Pusher beams relies'));
    // As in the mockup, the whole line plays from its start and the heard line is highlighted.
    await p.locator('.tline',{hasText:'Book a follow-up.'}).getByText('Book a follow-up.').click();
    await p.waitForFunction(()=>{const t=document.querySelector('audio')?.currentTime;return t>=74&&t<78;});
    await p.waitForFunction(()=>document.querySelectorAll('.tline.playing').length===1&&document.querySelector('.tline.playing').textContent.includes('Book a follow-up.'));
    const hl=await p.locator('.tline.playing').evaluate(e=>getComputedStyle(e).backgroundColor);
    const refHl=await reference.evaluate(()=>{const e=document.createElement('div');e.className='tline playing';document.querySelector('#doc').append(e);const c=getComputedStyle(e).backgroundColor;e.remove();return c;});
    if(hl!==refHl)differences.push(`.tline.playing background: ${hl} != ${refHl}`);
    await p.screenshot({path:`.playwright-out/transcript-playing-${theme}.png`});
    await reference.locator('#doc').getByRole('tab',{name:'Transcript',exact:true}).click();
    for(const [actualSelector,sourceSelector] of [['.tline:not(.playing)','.tline:not(.playing)'],['.tline .ts','.tline .ts'],['.tline .who','.tline .who'],['.flag','.flag'],['.player','.player'],['.player .bar','.player .bar'],['.player button','.player button']]){
      const actual=await p.locator(actualSelector).first().evaluate((e,props)=>Object.fromEntries(props.map(k=>[k,getComputedStyle(e)[k]])),[...properties,'cursor','fontFamily']);
      const expected=await reference.locator('#doc '+sourceSelector+', .mainwin '+sourceSelector).first().evaluate((e,props)=>Object.fromEntries(props.map(k=>[k,getComputedStyle(e)[k]])),[...properties,'cursor','fontFamily']);
      for(const key of [...properties,'cursor','fontFamily'])if(actual[key]!==expected[key])differences.push(`${actualSelector} ${key}: ${actual[key]} != ${expected[key]}`);
    }
    await reference.locator('.mainwin').screenshot({path:`.playwright-out/transcript-reference-${theme}.png`});
    await p.getByRole('button',{name:'❚❚ Pause',exact:true}).click();
    await p.getByRole('tab',{name:'Audio',exact:true}).click();
    await p.getByRole('button',{name:'mic.wav',exact:true}).waitFor();
    await reference.locator('#doc').getByRole('tab',{name:'Audio',exact:true}).click();
    for(const [actualSelector,sourceSelector] of [['.audio-table','.tbl'],['.audio-table table','.tbl table'],['.audio-table th','.tbl th'],['.audio-table td','.tbl td'],['.audio-table td.mono','.tbl td.mono']]){
      const actual=await p.locator(actualSelector).first().evaluate((e,props)=>Object.fromEntries(props.map(k=>[k,getComputedStyle(e)[k]])),properties);
      const expected=await reference.locator('#doc '+sourceSelector).first().evaluate((e,props)=>Object.fromEntries(props.map(k=>[k,getComputedStyle(e)[k]])),properties);
      for(const key of properties)if(actual[key]!==expected[key])differences.push(`${actualSelector} ${key}: ${actual[key]} != ${expected[key]}`);
    }
    await reference.locator('#doc').getByRole('tab',{name:'Notes',exact:true}).click();
    await p.setViewportSize({width:760,height:520});
    await p.getByRole('tab',{name:'Notes',exact:true}).click();
    const overflow=await p.evaluate(()=>document.documentElement.scrollWidth>innerWidth);
    await p.screenshot({path:`.playwright-out/document-minimum-${theme}.png`});
    if(differences.length||overflow||errors.length)throw Error(JSON.stringify({theme,differences,overflow,errors}));
    results.push({theme,differences,overflow,errors});
    await p.close();
  }
  await reference.close();
  return results;
}
