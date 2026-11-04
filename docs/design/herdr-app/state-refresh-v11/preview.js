/* Offline design renderer. Own synthetic template/data only; no runtime app APIs. */
const evaluate=(source,scope)=>Function('scope','with(scope){return ('+source+')}')(scope);
const expression=(text,scope)=>evaluate(text.replace(/^\s*{{|}}\s*$/g,''),scope);
function render(node,scope){
 if(node.nodeType===3)return document.createTextNode(node.textContent.replace(/{{(.*?)}}/gs,(_,code)=>String(evaluate(code,scope)??'')));
 if(node.nodeType!==1)return node.cloneNode(true);
 const tag=node.tagName.toLowerCase();
 if(tag==='sc-for'){
  const fragment=document.createDocumentFragment();
  for(const value of expression(node.getAttribute('list'),scope)??[]){const childScope={...scope,[node.getAttribute('as')]:value};for(const child of node.childNodes)fragment.append(render(child,childScope));}
  return fragment;
 }
 if(tag==='sc-if'){
  const fragment=document.createDocumentFragment();
  if(expression(node.getAttribute('value'),scope))for(const child of node.childNodes)fragment.append(render(child,scope));
  return fragment;
 }
 const result=node.cloneNode(false);
 for(const attribute of node.attributes){
  if(!attribute.value.includes('{{'))continue;
  const value=expression(attribute.value,scope);
  if(attribute.name==='style'&&value&&typeof value==='object'){result.removeAttribute('style');Object.assign(result.style,value);}
  else result.setAttribute(attribute.name,String(value??''));
 }
 for(const child of node.childNodes)result.append(render(child,scope));
 return result;
}
function badgeElement(code,count,mini=false){const badge=makeBadge(code,count);const el=document.createElement('span');el.className=badge.cls+(mini?' mini':'');el.textContent=mini?badge.glyph:badge.t;el.title=STATES[code]?.[0]??'Unknown';el.setAttribute('aria-label',el.title);return el;}
const all=document.createElement('div');
for(const node of document.querySelector('#source').content.childNodes)all.append(render(node,window.PROPOSAL));
const sheets=new Map();
for(const section of all.querySelectorAll('section.frame')){
 const label=section.dataset.screenLabel;const key=label.startsWith('D2')?'D2':label[0];section.dataset.frame=key;sheets.set(key,section);
}
for(const section of sheets.values()){
 for(const row of section.querySelectorAll('.r.ag')){
  const depth=row.querySelectorAll('.seg').length;row.dataset.depth=depth;
  if(depth>=3)for(const badge of row.querySelectorAll('.state-badge')){const code=[...badge.classList].find(x=>x.startsWith('st-'))?.slice(3);badge.classList.add('deep');badge.setAttribute('aria-label',STATES[code][0]);badge.title=STATES[code][0];badge.textContent=code==='wait'?badge.textContent.split(' ')[0]:STATES[code][2];}
 }
 for(const badge of section.querySelectorAll('.sp .state-badge')){const code=[...badge.classList].find(x=>x.startsWith('st-'))?.slice(3);badge.classList.add('deep');if(code!=='wait')badge.textContent=STATES[code][2];badge.title=STATES[code][0];}
 for(const header of section.querySelectorAll('.ph'))if(header.querySelector('.swg')){
  for(const mark of header.querySelectorAll(':scope>.mk'))mark.remove();
  header.append(badgeElement('fail',0,true),badgeElement('wait',2,true),badgeElement('run'));
 }
 for(const card of section.querySelectorAll('.dn'))if(card.querySelector(':scope>.dn-1>.h16')){
  card.classList.add('orch-card');
  const own=card.querySelector('.state-badge:not(.mini)');
  if(own){const code=[...own.classList].find(x=>x.startsWith('st-'))?.slice(3);own.classList.add('mini');own.textContent=STATES[code][2];own.title=STATES[code][0];own.setAttribute('aria-label',own.title);}
 }
 for(const phase of section.querySelectorAll('.dd.dph')){
  const glyph=phase.querySelector('.dn-k')?.textContent;
  phase.classList.add(glyph==='✓'?'phase-done':glyph==='▸'?'phase-now':glyph==='✗'?'phase-fail':glyph==='?'?'phase-wait':'phase-next');
 }
 const peek=section.querySelector('.peek');if(peek){peek.classList.add('state-fail');peek.querySelector('.pk-h>.mk')?.replaceWith(badgeElement('fail'));}
 for(const fold of section.querySelectorAll('.dn.dfold'))fold.classList.add('state-done');
}
const legend=sheets.get('B').querySelector('.legend');legend.innerHTML='<div class="lg-h">State boxes · always visible</div>';
for(const [code,s]of Object.entries(STATES)){
 const row=document.createElement('div');row.className='state-legend-row';row.append(badgeElement(code,code==='wait'?2:undefined));const label=document.createElement('span');label.innerHTML='<b>'+s[0]+'</b><small>'+s[6]+'</small>';row.append(label);legend.append(row);
}
legend.insertAdjacentHTML('beforeend','<div class="lg-h gap">The dock, as in the TUI</div><p class="legend-note">The focused orchestrator opens into its tree of threads and agents. Every other orchestrator is two lines: its own state, then one tile per active thread (its most urgent agent), a ✓n tile for finished threads, and the counts. Running tiles turn; tiles that need you breathe.</p><div class="lg-h gap">State and focus are separate</div><p class="legend-note">Color belongs to the work. A pale outer ring belongs to selection. Running and done remain visible, even when nothing needs you.</p><div class="lg-h">Dense rows</div><p class="legend-note">At depth 3+, an 18px square symbol tile protects the role, fold count and Peek control. Full labels remain on graph cards and in Peek.</p><div class="lg-h">Behavior stays the same</div><p class="legend-note">Tree folds, priorities, agent opens, orchestrator/thread select + drill, and horizontal depth scrolling are unchanged.</p>');
const gallery=document.createElement('section');gallery.className='gallery';gallery.innerHTML='<div class="gallery-header"><div><div class="gallery-kicker">Panorama · V5 state system</div><h1>Work has a visible state.</h1><p>Filled status boxes, semantic edges and quiet tinted surfaces. Selection stays separate.</p></div><div class="gallery-kicker">Synthetic design proposal<br>Not an installed screenshot</div></div><div class="gallery-grid"></div>';
for(const [code,s]of Object.entries(STATES)){
 const card=document.createElement('article');card.className='gallery-card state-'+code;const top=document.createElement('div');top.className='gallery-top';top.innerHTML='<b>'+s[0]+'</b>';top.append(badgeElement(code,code==='wait'?2:undefined));card.append(top);const p=document.createElement('p');p.textContent=s[6];card.append(p);const meta=document.createElement('p');meta.style.marginTop='12px';meta.innerHTML='<code>'+(code==='unknown'?'future-status':code==='idle'?'native herdr_status: idle':code)+'</code> · '+s[3].toUpperCase();card.append(meta);gallery.querySelector('.gallery-grid').append(card);
}
gallery.insertAdjacentHTML('beforeend','<div class="gallery-foot"><div class="selection-demo"><span class="state-badge st-run">↻ Running</span>Selected worker<small>Neutral focus ring + teal state edge coexist.</small></div><div><b>Attention is specific.</b><br>Red × Failed · Amber ! Permission · Amber ?2 Answer<br>Aggregate counts keep their scope; a working parent can still carry all three.</div><div><b>No invented signals.</b><br>Inferred means producer-inferred completion.<br>Stale is freshness, not proof that a writer or agent died.</div></div>');
sheets.set('Gallery',gallery);
const SVG={run:'spin',rev:'review',fail:'x',done:'check',q:'ring',bg:'orbit',cxl:'minus',inf:'check-q',lost:'tilde',stale:'dashed',idle:'ring'};
for(const sheet of sheets.values())for(const badge of sheet.querySelectorAll('.state-badge')){
 const code=[...badge.classList].find(x=>x.startsWith('st-'))?.slice(3);const value=badge.textContent;const gap=value.indexOf(' ');
 const sym=gap<0?value:value.slice(0,gap);const symbol=document.createElement('span');symbol.className='badge-symbol';badge.replaceChildren(symbol);
 if(SVG[code]){symbol.classList.add('gsym','g-'+SVG[code]);const rest=sym.replace(/^[^0-9]+/,'');if(rest){const n=document.createElement('span');n.className='badge-n';n.textContent=rest;badge.append(n);badge.classList.add('wide');}}
 else{symbol.textContent=sym;if(sym.length>1)badge.classList.add('wide');}
 if(gap>=0){const label=document.createElement('span');label.className='badge-label';label.textContent=value.slice(gap+1);badge.append(label);}
 badge.title=STATES[code]?.[0]??'Unknown';badge.setAttribute('aria-label',badge.title);
}
const params=new URLSearchParams(location.search),key=params.get('frame')??'D';
if(params.has('export'))document.body.classList.add('export');
document.querySelector('#stage').append(sheets.get(key)??sheets.get('D'));
for(const key of ['A','B','C','D','D2','Gallery']){const a=document.createElement('a');a.href='?frame='+key;a.textContent=key;a.setAttribute('aria-current',key===(params.get('frame')??'D')?'page':'false');document.querySelector('nav').append(a);}
window.DESIGN_READY=true;window.DESIGN_STATES=Object.keys(STATES);
window.addEventListener('load',()=>setTimeout(()=>{
 const badges=[...document.querySelectorAll('#stage .state-badge')];
 const badgeTextOverflow=badges.filter(el=>el.scrollWidth>el.clientWidth+1).map(el=>({text:el.textContent,width:el.clientWidth,scroll:el.scrollWidth}));
 const rowOverflow=badges.filter(el=>{const parent=el.closest('.r,.dn,.pk-h,.sp');if(!parent)return false;const a=el.getBoundingClientRect(),b=parent.getBoundingClientRect();return a.right>b.right+.75||a.left<b.left-.75;}).map(el=>el.textContent);
 const report={frame:key,interLoaded:document.fonts.check('12px "Inter V4"'),viewport:[innerWidth,innerHeight],images:[...document.querySelectorAll('#stage img')].map(el=>({source:el.getAttribute('src'),loaded:el.complete&&el.naturalWidth>0})),badgeCount:badges.length,badgeTextOverflow,rowOverflow,galleryStates:key==='Gallery'?[...document.querySelectorAll('.gallery-card')].map(el=>[...el.classList].find(x=>x.startsWith('state-')).slice(6)):[],depthHeaders:[...document.querySelectorAll('#stage .dheads .gh')].map(el=>el.textContent.trim()),selectedMeta:document.querySelector('#stage .dn.sel .dn-meta')?.textContent??null,donePhaseColors:[...document.querySelectorAll('#stage .phase-done .dn-k')].map(el=>getComputedStyle(el).color),workSweepColors:[...document.querySelectorAll('#stage .bar-in')].map(el=>({color:getComputedStyle(el).backgroundColor,state:el.closest('.r,.dn')?.className??'legend'})),deepTiles:[...document.querySelectorAll('#stage .r.ag .state-badge.deep')].map(el=>({symbol:el.textContent,label:el.getAttribute('aria-label')}))};
 window.DESIGN_REPORT=report;const script=document.createElement('script');script.id='design-report';script.type='application/json';script.textContent=JSON.stringify(report);document.body.append(script);
},100));
