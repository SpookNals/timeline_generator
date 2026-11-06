'use strict';
const $ = id => document.getElementById(id);
const invoke = (command, args) => window.__TAURI__.core.invoke(command, args);
let timeline = { start_year: 2015, end_year: 2026, blocks: [] };
let dirty = false, name = 'Untitled', editing = null, zoom = 1, busy = false, revision = 0, formPosition = null, committing = false, ready = false, initialForm = null, originalColor = null;
const months = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];
const fraction = value => { const [y,m] = value.split('-').map(Number); return y + (m-1)/12; };
const parts = value => { let y = Math.floor(value), m = Math.round((value-y)*12); if(m===12){y++;m=0;} return [y,m]; };
const monthInput = value => {const [y,m]=parts(value);return `${String(y).padStart(4,'0')}-${String(m+1).padStart(2,'0')}`;};
const date = value => {const [y,m]=parts(value);return `${months[m]} ${y}`;};
const hex = color => '#' + color.slice(0,3).map(v=>v.toString(16).padStart(2,'0')).join('');
const status = (text, error=false) => { $('status').textContent=text; $('status').classList.toggle('error',error); };
function mark(value=true){dirty=value;$('filename').textContent=name+(dirty?' ·':'');}
async function exclusive(action){if(!ready||busy||committing||drag)return;busy=true;updateControls();try{await action();}catch(e){status(String(e),true);}finally{busy=false;updateControls();}}
async function draw(candidate=timeline){const result=await invoke('preview',{timeline:candidate});$('paper').innerHTML=result.svg;return result.timeline;}
function list(){
  $('blocks').replaceChildren();$('count').textContent=String(timeline.blocks.length).padStart(2,'0');$('empty').hidden=timeline.blocks.length>0;
  $('range-start').value=timeline.start_year;$('range-end').value=timeline.end_year;$('range-label').textContent=`${timeline.start_year} — ${timeline.end_year}`;
  timeline.blocks.map((b,i)=>({b,i})).sort((a,b)=>b.b.start_year-a.b.start_year).forEach(({b,i})=>{
    const button=document.createElement('button');button.className='block';button.title=`${b.title} — Edit`;
    const dot=document.createElement('span');dot.className='block-dot';dot.style.background=hex(b.color);
    const content=document.createElement('span');content.className='block-content';const title=document.createElement('strong');title.textContent=b.title;
    const dates=document.createElement('small');dates.textContent=`${date(b.start_year)} — ${date(b.end_year)}`;
    content.append(title,dates);button.append(dot,content);button.onclick=()=>edit(i);$('blocks').append(button);
  });
}
async function commit(candidate){if(committing)throw new Error('Please wait for the current change.');committing=true;lockForm(true);updateControls();try{timeline=await draw(candidate);revision++;mark();list();status('Unsaved changes');}finally{committing=false;lockForm(false);updateControls();}}
function edit(index=null){
  if(!ready||busy||committing||drag)return;
  editing=index;const b=index===null?{title:'',subtitle:'',start_year:timeline.start_year,end_year:Math.min(timeline.start_year+1,timeline.end_year),color:[189,134,75,255]}:timeline.blocks[index];
  $('form-heading').textContent=index===null?'New period':'Edit period';$('title').value=b.title;$('subtitle').value=b.subtitle||'';$('start').value=monthInput(b.start_year);$('end').value=monthInput(b.end_year);$('color').value=hex(b.color);$('form-error').textContent='';$('delete').hidden=index===null;
  $('description').value=b.description||'';formPosition=b.label_position||null;originalColor=[...b.color];updateDuration();initialForm=formSnapshot();
  $('block-dialog').showModal();$('title').focus();
}
function closeForm(){if(committing)return;$('block-dialog').close();initialForm=null;}
$('add').onclick=()=>edit();$('empty-add').onclick=()=>edit();['cancel','cancel-x'].forEach(id=>$(id).onclick=closeForm);
$('block-form').onsubmit=async e=>{
  e.preventDefault();if(!ready||busy||committing)return;const title=$('title').value.trim(), start=fraction($('start').value), end=fraction($('end').value);
  if(!title||!Number.isFinite(start)||!Number.isFinite(end)||end<start){$('form-error').textContent='Enter a title and an end date on or after the start date.';return;}
  const c=$('color').value;const block={description:$('description').value.trim()||null,label_position:formPosition,title,subtitle:$('subtitle').value.trim()||null,start_year:start,end_year:end,color:originalColor&&hex(originalColor)===c?[...originalColor]:[parseInt(c.slice(1,3),16),parseInt(c.slice(3,5),16),parseInt(c.slice(5,7),16),255]};
  const candidate=structuredClone(timeline);if(editing===null)candidate.blocks.push(block);else candidate.blocks[editing]=block;
  try{await commit(candidate);closeForm();}catch(e){$('form-error').textContent=String(e);}
};
$('delete').onclick=async()=>{if(!ready||busy||committing)return;const candidate=structuredClone(timeline);candidate.blocks.splice(editing,1);try{await commit(candidate);closeForm();}catch(e){$('form-error').textContent=String(e);}};
for(const id of ['range-start','range-end'])$(id).onchange=()=>exclusive(async()=>{try{await commit({...timeline,start_year:Number($('range-start').value),end_year:Number($('range-end').value)});}finally{list();}});
$('open').onclick=()=>exclusive(async()=>{if(dirty&&!await invoke('confirm_discard'))return;const opened=await invoke('open_project');if(opened){$('paper').innerHTML=opened.svg;timeline=opened.timeline;name=opened.name;revision++;mark(false);list();status('Project opened');}});
function save(saveAs=false){return exclusive(async()=>{const savedRevision=revision;const result=await invoke('save_project',{timeline:structuredClone(timeline),saveAs});if(result){name=result;mark(revision!==savedRevision);status(dirty?'New changes have not been saved':'Saved');}});}
$('save').onclick=()=>save();$('save-as').onclick=()=>save(true);
$('export').onclick=()=>exclusive(async()=>{status('Exporting PNG…');const done=await invoke('export_image',{timeline:structuredClone(timeline)});status(done?'PNG exported':'Export cancelled');});
function setZoom(value){zoom=Math.max(.5,Math.min(2,value));$('paper').style.width=`${zoom*100}%`;$('zoom-reset').textContent=`${Math.round(zoom*100)}%`;}
$('zoom-in').onclick=()=>setZoom(zoom+.1);$('zoom-out').onclick=()=>setZoom(zoom-.1);$('zoom-reset').onclick=()=>setZoom(1);
function effects(on){document.body.classList.toggle('no-effects',!on);$('effects').setAttribute('aria-pressed',String(on));try{localStorage.setItem('scanlines',String(on));}catch{}}
$('effects').onclick=()=>effects($('effects').getAttribute('aria-pressed')!=='true');
try{effects(localStorage.getItem('scanlines')!=='false');}catch{}
document.addEventListener('keydown',e=>{if((e.ctrlKey||e.metaKey)&&!$('block-dialog').open){if(e.key.toLowerCase()==='s'){e.preventDefault();save(e.shiftKey);}if(e.key.toLowerCase()==='o'){e.preventDefault();$('open').click();}}});
async function start(){
  list();updateControls();
  try{
    const win=window.__TAURI__.window.getCurrentWindow();
    await win.onCloseRequested(createCloseHandler({
      windowHandle: win,
      hasUnsavedChanges: () => dirty || hasFormChanges() || Boolean(drag),
      isBusy: () => busy || committing,
      confirmDiscard: () => invoke('confirm_discard'),
      report: status,
    }));
    timeline=await draw();list();ready=true;updateControls();
  }catch(e){status(`Could not start: ${e}`,true);}
  setTimeout(()=>$('intro').remove(),1900);
}

function updateDuration(){
  const count=Math.round((fraction($('end').value)-fraction($('start').value))*12);
  if(!Number.isFinite(count)||count<0){$('duration').textContent='Check dates';return;}
  const years=Math.floor(count/12),months=count%12;
  $('duration').textContent=[years?`${years} year${years===1?'':'s'}`:'',months||!years?`${months} month${months===1?'':'s'}`:''].filter(Boolean).join(' ');
}
$('start').addEventListener('input',updateDuration);$('end').addEventListener('input',updateDuration);
$('reset-position').onclick=()=>{formPosition=null;status('Label position will reset when this period is saved.');};

// Coordinates are in SVG logical units, so dragging is independent of zoom or scrolling.
let drag=null;
const paper=$('paper');
function svgPoint(event,svg){return new DOMPoint(event.clientX,event.clientY).matrixTransform(svg.getScreenCTM().inverse());}
paper.addEventListener('pointerdown',event=>{
  const group=event.target.closest('.timeline-label');
  if(!group||event.button!==0||!ready||busy||committing)return;
  const svg=group.ownerSVGElement,index=Number(group.dataset.index);
  const connector=svg.querySelector(`[data-connector="${index}"]`);
  drag={group,svg,index,connector,original:connector.getAttribute('points'),start:svgPoint(event,svg),x:Number(group.dataset.x),y:Number(group.dataset.y),dx:0,dy:0};
  paper.setPointerCapture(event.pointerId);group.focus();event.preventDefault();
});
paper.addEventListener('pointermove',event=>{
  if(!drag)return;
  const p=svgPoint(event,drag.svg),box=drag.svg.viewBox.baseVal;
  const x=Math.max(8,Math.min(box.width-Number(drag.group.dataset.width)-8,drag.x+p.x-drag.start.x));
  const y=Math.max(8,Math.min(box.height-Number(drag.group.dataset.height)-8,drag.y+p.y-drag.start.y));
  drag.dx=x-drag.x;drag.dy=y-drag.y;
  drag.group.setAttribute('transform',`translate(${drag.dx} ${drag.dy})`);
  const points=drag.original.split(' ').map(pair=>pair.split(',').map(Number));
  points[1][0]+=drag.dx;points[2][0]+=drag.dx;points[2][1]+=drag.dy;points[3][0]+=drag.dx;points[3][1]+=drag.dy;
  drag.connector.setAttribute('points',points.map(pair=>pair.join(',')).join(' '));
});
paper.addEventListener('pointerup',async event=>{
  if(!drag)return;const moved=drag;drag=null;paper.releasePointerCapture(event.pointerId);
  if(Math.abs(moved.dx)+Math.abs(moved.dy)<2){moved.group.removeAttribute('transform');moved.connector.setAttribute('points',moved.original);return;}
  const candidate=structuredClone(timeline);candidate.blocks[moved.index].label_position=[moved.x+moved.dx,moved.y+moved.dy];
  try{await commit(candidate);}catch(e){await draw();status(String(e),true);}
});
function cancelDrag(){if(!drag)return;drag.group.removeAttribute('transform');drag.connector.setAttribute('points',drag.original);drag=null;}
paper.addEventListener('pointercancel',cancelDrag);paper.addEventListener('lostpointercapture',cancelDrag);
paper.addEventListener('dblclick',event=>{const group=event.target.closest('.timeline-label');if(group)edit(Number(group.dataset.index));});
paper.addEventListener('keydown',async event=>{
  const group=event.target.closest('.timeline-label');if(!group||!ready||busy||committing)return;
  const index=Number(group.dataset.index);
  if(event.key==='Enter'){event.preventDefault();edit(index);return;}
  const directions={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]};
  if(!directions[event.key])return;event.preventDefault();
  const [dx,dy]=directions[event.key],step=event.shiftKey?20:5,candidate=structuredClone(timeline);
  candidate.blocks[index].label_position=[Number(group.dataset.x)+dx*step,Number(group.dataset.y)+dy*step];
  try{await commit(candidate);paper.querySelector(`[data-index="${index}"]`).focus();}catch(e){status(String(e),true);}
});

function formSnapshot(){
  return JSON.stringify({fields:['title','subtitle','start','end','color','description'].map(id=>$(id).value),position:formPosition});
}
function hasFormChanges(){return $('block-dialog').open && initialForm!==formSnapshot();}
function updateControls(){document.querySelectorAll('header button, .range input, #add, #empty-add').forEach(control=>control.disabled=!ready||busy||committing);}
function lockForm(locked){document.querySelectorAll('#block-form input, #block-form textarea, #block-form button').forEach(control=>control.disabled=locked);}
$('block-dialog').addEventListener('cancel',event=>{if(committing)event.preventDefault();});
document.addEventListener('keydown',event=>{if(event.key==='Escape'&&drag){event.preventDefault();cancelDrag();}});

start();
