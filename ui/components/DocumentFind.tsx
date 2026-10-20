import {useEffect, useLayoutEffect, useRef, useState, type RefObject} from 'react';

// Ranges paint over the document without inserting markup into editable content.
function findRanges(body:HTMLElement, tab:string, query:string) {
  if(!query.trim())return [];
  const pattern=new RegExp(query.trim().replace(/[.*+?^${}()|[\]\\]/g,'\\$&'),'giu');
  const blocks=tab==='Transcript'?Array.from(body.querySelectorAll('.tline .words'))
    :Array.from(body.querySelectorAll('.notes h1, .notes h2, .notes h3, .notes h4, .notes h5, .notes h6, .notes p:not(.notes-hint), .notes li')).filter(block=>!block.querySelector('h1, h2, h3, h4, h5, h6, p, li'));
  const ranges:Range[]=[];
  for(const block of blocks){
    const nodes:{node:Text;start:number}[]=[],walker=document.createTreeWalker(block,NodeFilter.SHOW_TEXT);
    let node:Node|null,text='';
    while((node=walker.nextNode())){
      if(node.parentElement?.closest('.src, .fixall, .epop, [hidden]'))continue;
      nodes.push({node:node as Text,start:text.length});text+=node.textContent;
    }
    for(const match of text.matchAll(pattern)){
      const start=nodes.findLast(node=>node.start<=match.index)!,end=nodes.findLast(node=>node.start<match.index+match[0].length)!;
      const range=document.createRange();
      range.setStart(start.node,match.index-start.start);range.setEnd(end.node,match.index+match[0].length-end.start);
      ranges.push(range);
    }
  }
  return ranges;
}

function FindIcon({children}:{children:React.ReactNode}) {
  return <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{children}</svg>;
}

export default function DocumentFind({tab,body}:{tab:string;body:RefObject<HTMLDivElement|null>}) {
  const [scope,setScope]=useState<string|null>(null),[query,setQuery]=useState('');
  const [ranges,setRanges]=useState<Range[]>([]),[index,setIndex]=useState(-1);
  const input=useRef<HTMLInputElement>(null),back=useRef<HTMLElement|null>(null);
  const open=scope===tab;
  function close(restore=true){
    setScope(null);
    if(restore)(back.current?.isConnected?back.current:document.getElementById(`tab-${tab}`))?.focus({preventScroll:true});
  }
  useEffect(()=>{setScope(null);},[tab]);
  useLayoutEffect(()=>{if(open){input.current?.focus();input.current?.select();}},[open]);
  useEffect(()=>{
    const keys=(event:KeyboardEvent)=>{
      if((event.metaKey||event.ctrlKey)&&!event.altKey&&event.key.toLowerCase()==='f'&&['Notes','Transcript'].includes(tab)&&!document.querySelector('.palette, [aria-modal="true"]')){
        event.preventDefault();event.stopImmediatePropagation();
        if(!open){back.current=document.activeElement as HTMLElement|null;back.current?.blur();setScope(tab);}
        else{input.current?.focus();input.current?.select();}
      }else if(event.key==='Escape'&&open){event.preventDefault();event.stopImmediatePropagation();close();}
    };
    document.addEventListener('keydown',keys,true);
    return()=>document.removeEventListener('keydown',keys,true);
  });
  useLayoutEffect(()=>{
    if(!open||!body.current)return;
    const element=body.current;
    const update=()=>{setRanges(findRanges(element,tab,query));setIndex(-1);};
    update();
    const observer=new MutationObserver(update);
    observer.observe(element,{subtree:true,childList:true,characterData:true});
    return()=>observer.disconnect();
  },[open,query,tab,body]);
  useLayoutEffect(()=>{
    if(open){
      CSS.highlights.set('motet-find',new Highlight(...ranges));
      CSS.highlights.set('motet-find-current',new Highlight(...(index<0?[]:[ranges[index]])));
    }
    return()=>{CSS.highlights.delete('motet-find');CSS.highlights.delete('motet-find-current');};
  },[open,ranges,index]);
  function step(direction:number){
    if(!ranges.length||!body.current)return;
    const next=index<0?direction>0?0:ranges.length-1:(index+direction+ranges.length)%ranges.length;
    setIndex(next);
    const rect=ranges[next].getBoundingClientRect(),element=body.current;
    element.scrollTop+=rect.top-element.getBoundingClientRect().top-Math.max(12,(element.clientHeight-rect.height)/2);
  }
  if(!open)return null;
  const label=`Find in ${tab.toLowerCase()}`;
  const count=!query.trim()?'':!ranges.length?'No matches':index<0?`${ranges.length} matches`:`${index+1} of ${ranges.length}`;
  return <div className="findrow"><div className="findbar" role="search" aria-label={label}>
    <FindIcon><circle cx="7" cy="7" r="4.5"/><path d="m10.5 10.5 3 3"/></FindIcon>
    <input ref={input} aria-label={label} placeholder={`${label}…`} value={query} onChange={event=>setQuery(event.target.value)} autoComplete="off" spellCheck={false} onKeyDown={event=>{if(event.key==='Enter'){event.preventDefault();event.stopPropagation();step(event.shiftKey?-1:1);}}}/>
    <output aria-live="polite">{count}</output>
    <button type="button" disabled={!ranges.length} aria-label="Previous match" title="Previous match (Shift+Enter)" onClick={()=>step(-1)}><FindIcon><path d="m4 9 4-4 4 4"/></FindIcon></button>
    <button type="button" disabled={!ranges.length} aria-label="Next match" title="Next match (Enter)" onClick={()=>step(1)}><FindIcon><path d="m4 7 4 4 4-4"/></FindIcon></button>
    <button type="button" aria-label="Close find" title="Close find (Esc)" onClick={()=>close()}><FindIcon><path d="m4 4 8 8m0-8-8 8"/></FindIcon></button>
  </div></div>;
}
