'use strict';
// Keep the wordmark stationary while its prefix rolls, once visible and every 30 minutes.
(()=>{
 const words=['super','best','smart','just','bold','good','fast','next','run'];
 const period=30*60*1000;
 const reducedMotion=matchMedia('(prefers-reduced-motion: reduce)');
 const logos=[...document.querySelectorAll('.brand')].map(brand=>{
  const prefix=brand.firstChild;
  if(prefix?.nodeType!==Node.TEXT_NODE||prefix.textContent.trim()!=='right')return null;
  const slot=document.createElement('span');slot.className='brand-slot';slot.setAttribute('aria-hidden','true');
  const reel=document.createElement('span');reel.className='brand-reel';slot.append(reel);
  const label=document.createElement('span');label.className='brand-accessible';label.textContent='right';
  prefix.replaceWith(label,slot);
  const row=document.createElement('b');row.textContent='right';reel.append(row);
  // The slot is as wide as "right", so the wordmark has no gap; longer words overflow to the left.
  const fit=()=>{const range=document.createRange();range.selectNodeContents(row);const width=range.getBoundingClientRect().width;if(width)slot.style.width=width/parseFloat(getComputedStyle(slot).fontSize)+.05+'em'};
  new ResizeObserver(fit).observe(brand);document.fonts?.ready.then(fit);
  return {brand,reel,lastRun:null,running:false};
 }).filter(Boolean);
 const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
 async function spin(logo){
  if(logo.running||reducedMotion.matches)return;
  logo.running=true;logo.lastRun=Date.now();
  const shuffled=[...words];
  for(let i=shuffled.length-1;i>0;i--){const j=Math.floor(Math.random()*(i+1));[shuffled[i],shuffled[j]]=[shuffled[j],shuffled[i]]}
  const sequence=['right',...shuffled.slice(0,8),'right'];
  logo.reel.replaceChildren(...sequence.map(word=>{const row=document.createElement('b');row.textContent=word;return row}));
  const durations=[240,220,220,250,290,350,400,480,720];
  try{
   await pause(550);
   for(let i=0;i<sequence.length-1;i++){
    if(reducedMotion.matches||document.hidden||!logo.brand.getClientRects().length)break;
    const animation=logo.reel.animate([{transform:`translateY(${-i*1.4}em)`},{transform:`translateY(${-(i+1)*1.4}em)`}],{duration:durations[i],easing:i===sequence.length-2?'cubic-bezier(.15,.65,.22,1)':'cubic-bezier(.4,0,.2,1)',fill:'forwards'});
    await animation.finished;
    logo.reel.style.transform=`translateY(${-(i+1)*1.4}em)`;animation.cancel();
    if(i<sequence.length-2)await pause(100+i*25);
   }
  }finally{
   const row=document.createElement('b');row.textContent='right';logo.reel.replaceChildren(row);
   logo.reel.style.transform='';logo.running=false;
  }
 }
 function check(){
  if(document.hidden||reducedMotion.matches)return;
  for(const logo of logos){
   if(!logo.running&&logo.brand.getClientRects().length&&(logo.lastRun===null||Date.now()-logo.lastRun>=period))void spin(logo);
  }
 }
 for(const logo of logos){
  logo.brand.setAttribute('role','button');
  logo.brand.tabIndex=0;
  logo.brand.setAttribute('aria-label','rightcommit');
  logo.brand.addEventListener('click',()=>void spin(logo));
  logo.brand.addEventListener('keydown',event=>{
   if(event.key==='Enter'||event.key===' '){event.preventDefault();if(!event.repeat)void spin(logo)}
  });
 }
 const observer=new MutationObserver(check);
 for(const surface of document.querySelectorAll('#auth,#app'))observer.observe(surface,{attributes:true,attributeFilter:['hidden']});
 document.addEventListener('visibilitychange',check);
 reducedMotion.addEventListener('change',check);
 setInterval(check,period);
 check();
})();
let appearanceTheme='system';
try{const savedTheme=localStorage.getItem('nodo.theme');if(['light','dark','system'].includes(savedTheme))appearanceTheme=savedTheme}catch{}
function applyAppearanceTheme(value){
  appearanceTheme=['light','dark'].includes(value)?value:'system';
  document.documentElement.dataset.theme=appearanceTheme;
}
applyAppearanceTheme(appearanceTheme);
window.addEventListener('storage',e=>{if(e.key==='nodo.theme'){applyAppearanceTheme(e.newValue);document.querySelectorAll('[data-appearance-theme]').forEach(field=>field.value=appearanceTheme)}});
const $=(s,r=document)=>r.querySelector(s),$$=(s,r=document)=>[...r.querySelectorAll(s)];
const esc=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const icons={Source:'folder-git-2',Agent:'bot',Task:'list-todo',Review:'clipboard-check',Fork:'parallel-fork',Join:'parallel-join',Gate:'git-branch',Memory:'brain-circuit',Connection:'plug',Workflow:'workflow',MCP:'mcp-client',RAG:'database-rag',A2A:'a2a-client',Comment:'sticky-note'};
const states={starting:'In attesa di risorse',queued:'In coda',running:'In esecuzione',completed:'Completato',paused:'In pausa',waiting_user:'Attende risposta',waiting_review:'Attende review',waiting_external:'Servizio in corso',waiting_child:'Workflow figlio in corso',failed:'Errore',interrupted:'Interrotto',skipped:'Non percorso',rework:'Rework'};
const app={user:null,settings:null,workflows:[],wf:null,run:null,editBase:null,continueNodes:[],artifacts:[],decisions:[],children:[],reviewRun:null,runs:[],selected:null,selectedNodes:[],selectedEdge:null,view:'graph',stepFilter:'all',stepScope:null,dirty:false,zoom:.85,pan:{x:30,y:40},link:null,events:[],stream:null,lastEvent:0,refresh:null};
let edgeDrag=null;
let latestRun=null,latestRunWorkflow='',latestRunLoading=false,latestRunRequest=0,openingLatestRun=false;
let wheelZoomEnabled=true;
try{wheelZoomEnabled=localStorage.getItem('nodo.wheelZoom')!=='false'}catch{}
function icon(name,cls=''){return `<svg class="icon ${cls}" aria-hidden="true"><use href="/icons.svg#${name}"></use></svg>`}
function nodeService(n){const id=n.kind==='Agent'?(n.runtime||app.settings?.default_runtime):n.kind==='Connection'?n.provider:null;return Object.hasOwn(serviceInfo,id)?id:null}
function nodeIcon(n){const service=nodeService(n),info=service&&serviceInfo[service];return icon(info?.icon||icons[n.kind],info?`service-icon service-icon-${service} ${info.icon.startsWith('brand-')?'brand-icon':''}`:'')}
function nodeLabel(n){const service=nodeService(n);return service?serviceInfo[service].name:n.kind}
function associatedAgent(n){return ['Task','Review','Gate'].includes(n.kind)?currentWorkflow().nodes.find(a=>a.id===n.agent&&a.kind==='Agent'):null}
function associatedAgentBadge(agent){return agent?`<span class="node-agent-badge" data-agent="${esc(agent.id)}" data-tip="${esc(agent.title)} · ${esc(nodeLabel(agent))}" aria-hidden="true">${nodeIcon(agent)}</span>`:''}
function hydrate(root=document){$$('[data-icon]',root).forEach(e=>{e.innerHTML=icon(e.dataset.icon);e.removeAttribute('data-icon')})}
async function api(path,body,method){const res=await fetch('/api'+path,{method:method||(body===undefined?'GET':'POST'),headers:{'Content-Type':'application/json','X-Nodo-Request':'1'},credentials:'same-origin',...(body!==undefined?{body:JSON.stringify(body)}:{})});let data;try{data=await res.json()}catch{throw Error('Risposta server non valida')}if(!res.ok){if(res.status===401&&path!='/auth/login')showAuth();throw Error(data.error||`HTTP ${res.status}`)}return data}
let toastTimer;function toast(s){$('#toast').textContent=s;$('#toast').hidden=false;clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('#toast').hidden=true,6500)}
function submitSingleTextDialog(e){
  if(e.key!=='Enter'||e.defaultPrevented||e.isComposing||e.keyCode===229||e.ctrlKey||e.metaKey||e.altKey||e.shiftKey)return;
  const dialog=e.currentTarget,field=e.target;
  if(!dialog.open||!field.matches('input')||!['text','search','email','url','tel','password'].includes(field.type)||field.readOnly||field.matches(':disabled'))return;
  // Count every visible input control, including custom selects and switches.
  const visible=el=>!el.closest('[hidden]')&&el.getClientRects().length>0;
  const fields=$$('input:not([type="hidden"]):not([type="submit"]):not([type="button"]):not([type="reset"]),textarea,select,jui-select,jui-switch,[contenteditable]:not([contenteditable="false"])',dialog).filter(visible);
  const primary=$$('.modal-actions button.primary',dialog).filter(visible);
  if(fields.length!==1||fields[0]!==field||primary.length!==1)return;
  e.preventDefault();
  if(e.repeat||primary[0].matches(':disabled')||primary[0].getAttribute('aria-disabled')==='true')return;
  if(!(field.form||field).reportValidity())return;
  primary[0].click();
}
function modal(title,body,actions='',wide=false){$('#dialog').onkeydown=submitSingleTextDialog;closeAccountMenu();closeNodeMenu();hideTooltip();$('#dialog').classList.toggle('wide',wide);$('#dialog-content').innerHTML=`<div class="modal-head"><h2>${esc(title)}</h2><div class="spacer"></div><button class="icon-button" data-action="close-dialog" aria-label="Chiudi">${icon('x')}</button></div><div class="modal-body">${body}</div>${actions?`<div class="modal-actions">${actions}</div>`:''}`;
 for(const table of $$('.markdown table',$('#dialog-content'))){
  if(table.parentElement.classList.contains('markdown-table-scroll'))continue;
  const wrapper=document.createElement('div');wrapper.className='markdown-table-scroll';wrapper.tabIndex=0;wrapper.setAttribute('role','region');wrapper.setAttribute('aria-label',uiText('Tabella scorrevole'));table.before(wrapper);wrapper.append(table);
 }
 if(!$('#dialog').open)$('#dialog').showModal()
}
function appConfirm({title,message,accept='Conferma',danger=false}){
  closeNodeMenu();
  return new Promise(resolve=>{
    const dialog=document.createElement('dialog');dialog.className='app-confirm';dialog.setAttribute('aria-labelledby','confirmation-title');dialog.setAttribute('aria-describedby','confirmation-message');
    dialog.innerHTML=`<div class="modal-head">${icon(danger?'trash-2':'circle-help')}<h2 id="confirmation-title">${esc(title)}</h2><div class="spacer"></div><button type="button" class="icon-button" data-confirm-cancel aria-label="Chiudi">${icon('x')}</button></div><div class="modal-body"><p id="confirmation-message">${esc(message)}</p></div><div class="modal-actions"><div class="spacer"></div><button type="button" data-confirm-cancel autofocus>Annulla</button><button type="button" class="primary ${danger?'confirm-danger':''}" data-confirm-accept>${esc(accept)}</button></div>`;
    let settled=false;
    const finish=accepted=>{if(settled)return;settled=true;dialog.close();dialog.remove();resolve(accepted)};
    dialog.querySelectorAll('[data-confirm-cancel]').forEach(b=>b.onclick=()=>finish(false));
    dialog.querySelector('[data-confirm-accept]').onclick=()=>finish(true);
    dialog.addEventListener('cancel',e=>{e.preventDefault();finish(false)});
    dialog.addEventListener('close',()=>finish(false),{once:true});
    document.body.append(dialog);dialog.showModal();
  });
}
const closeModal=()=>$('#dialog').close();
function showAuth(){closeAccountMenu();window.resetTemplateManager?.();app.stream?.close();$('#dialog').close();$$('dialog.app-confirm').forEach(dialog=>dialog.close());$('#app').hidden=true;$('#auth').hidden=false}
let registerMode=false,authInfo={};async function init(){hydrate();try{authInfo=await api('/auth/status');registerMode=authInfo.setup_required;$('#auth-title').textContent=registerMode?'Crea il tuo workspace':'Accedi al workspace';$('#auth-submit').textContent=registerMode?'Crea amministratore':'Accedi';$('#auth-note').textContent=registerMode?'Il primo account può creare gli altri utenti.':'Ogni account ha workflow, sessioni e credenziali separati.';$('#auth-switch').hidden=!authInfo.registration_open||registerMode;app.user=await api('/auth/me');await boot()}catch(e){showAuth();if(e.message!=='Accedi per continuare')$('#auth-error').textContent=''}}
async function boot(){resetActivityLogs();latestRun=null;latestRunWorkflow='';++latestRunRequest;app.user=await api('/auth/me');app.settings=await api('/settings');app.workflows=await api('/workflows');await loadWorkflowTopics();await ragRefresh();await a2aRefresh();app.runs=await api('/runs');$('#auth').hidden=true;$('#app').hidden=false;$('#account').textContent=app.user.email[0].toUpperCase();$('#account').dataset.tip=app.user.email;$('#footer-info').textContent=app.user.isolated?'Workspace personali isolati · Sessioni persistenti':'Modalità sviluppo · Isolamento dei file non attivo';workflowOptions();selectWorkflow(pendingDraftId()||workflowOrganization.recent[0]);}
function orderedWorkflows(){return app.workflows.map((w,index)=>({w,index})).sort((a,b)=>(b.w.created||0)-(a.w.created||0)||a.index-b.index).map(x=>x.w)}
let workflowOrganization={topics:[{id:'unassigned',name:'Unassigned'}],assignments:{},recent:[]};
let topicRequest=Promise.resolve();
function workflowTopic(id){return workflowOrganization.assignments[id]||'unassigned'}
async function loadWorkflowTopics(){workflowOrganization=await api('/workflow-topics')}
function editWorkflowTopics(action){
 const request=topicRequest.catch(()=>{}).then(async()=>{workflowOrganization=await api('/workflow-topics',action);workflowOptions();return workflowOrganization});
 topicRequest=request;return request;
}
function rememberWorkflow(id){if(app.workflows.some(w=>w.id===id))editWorkflowTopics({action:'open',workflow:id}).catch(e=>toast(e.message))}
function topicWorkflows(topic){const rank=new Map(workflowOrganization.recent.map((id,i)=>[id,i]));return orderedWorkflows().filter(w=>workflowTopic(w.id)===topic).sort((a,b)=>(rank.get(a.id)??Infinity)-(rank.get(b.id)??Infinity))}
function closeWorkflowMenu(){ $('#workflow-topic-menu').hidden=true;$('#workflow-select').setAttribute('aria-expanded','false') }
function workflowOptions(selectedId=currentWorkflow()?.id){
 const selected=app.workflows.find(w=>w.id===selectedId)||(!app.run&&app.wf?.id===selectedId?app.wf:null);
 $('#workflow-select').innerHTML=`<span data-no-i18n>${esc(selected?.name||'Workflow')}</span>${icon('chevron-down')}`;
 const recent=workflowOrganization.recent.find(id=>app.workflows.some(w=>w.id===id)),activeTopic=recent?workflowTopic(recent):'unassigned';
 const topics=[...workflowOrganization.topics].sort((a,b)=>Number(b.id===activeTopic)-Number(a.id===activeTopic)).slice(0,5);
 $('#workflow-topic-menu').innerHTML=topics.map(t=>{const flows=topicWorkflows(t.id);return `<details class="workflow-topic" ${t.id===activeTopic?'open':''}><summary><span data-no-i18n>${esc(t.name)}</span><span class="small">${flows.length}</span></summary><div>${flows.slice(0,5).map(w=>`<button type="button" data-workflow-choice="${esc(w.id)}" ${w.id===selectedId?'aria-current="true"':''}><span data-no-i18n>${esc(w.name)}</span></button>`).join('')}${flows.length>5?`<button type="button" data-topic-all="${esc(t.id)}">${uiText('Mostra tutti')} (${flows.length})…</button>`:''}${!flows.length?`<p class="small">${uiText('Nessun workflow')}</p>`:''}</div></details>`}).join('')+`<button type="button" id="manage-workflow-list">Organize workflows…</button>`;
 $$('[data-workflow-choice]',$('#workflow-topic-menu')).forEach(b=>b.onclick=async()=>{closeWorkflowMenu();await chooseWorkflow(b.dataset.workflowChoice)});
 $$('[data-topic-all]').forEach(b=>b.onclick=()=>{closeWorkflowMenu();showWorkflowList(b.dataset.topicAll)});
 $('#manage-workflow-list').onclick=()=>{closeWorkflowMenu();showWorkflowList()};
}
async function chooseWorkflow(id){if(!app.run&&app.wf?.id===id){closeModal();rememberWorkflow(id);return}if(app.dirty){try{await saveWorkflow()}catch(e){toast(e.message);return}}closeModal();selectWorkflow(id)}
async function showWorkflowList(topicFilter=''){
 try{
 await topicRequest.catch(()=>{});app.workflows=await api('/workflows');await loadWorkflowTopics();await ragRefresh();workflowOptions();
 let selected=new Set(),busy=false;
 modal('Elenco workflow',`<div class="workflow-list"><div class="workflow-list-toolbar"><label class="workflow-search">Cerca workflow<input type="search" id="workflow-search" placeholder="Cerca per nome…" autocomplete="off"></label><button type="button" id="manage-topics-toggle" aria-expanded="false" aria-controls="workflow-topic-settings">${icon('settings-2')}Gestisci topic</button></div><section id="workflow-topic-settings" hidden><form id="create-topic-form"><label>Nuovo topic<input id="new-topic-name" required maxlength="80" autocomplete="off"></label><button type="submit">${icon('plus')}Crea topic</button></form><div id="topic-management-rows"></div><p class="small">I primi cinque topic sono visibili nel selettore. Il topic dell’ultimo workflow usato viene mostrato per primo automaticamente.</p></section><nav id="workflow-topic-filters" aria-label="Filtra per topic"></nav><div class="workflow-bulk" hidden><span id="workflow-selection-count" role="status"></span><div class="workflow-move-destination"><label for="workflow-topic-destination">Sposta in</label><jui-select id="workflow-topic-destination" aria-label="Sposta in"></jui-select></div><div class="workflow-bulk-actions"><button type="button" id="move-topic-workflows">${icon('move-right')}<span data-no-i18n>Move</span></button><button type="button" id="clear-workflow-selection">${icon('broom')}<span data-no-i18n>Clear</span></button></div></div><div class="workflow-list-heading"><jui-switch id="workflow-select-all" aria-label="Seleziona tutti i risultati"></jui-switch><span class="small" id="workflow-result-count" role="status" aria-live="polite"></span></div><div class="workflow-picker-table"><table><tbody id="workflow-picker-rows"></tbody></table></div><p class="note" id="workflow-picker-empty" hidden>Nessun workflow trovato.</p></div>`,'<button type="button" data-action="close-dialog">Chiudi</button>',true);
 const search=$('#workflow-search'),filter={value:topicFilter};
 $('#manage-topics-toggle').onclick=()=>{const panel=$('#workflow-topic-settings');panel.hidden=!panel.hidden;$('#manage-topics-toggle').setAttribute('aria-expanded',String(!panel.hidden))};
 const options=()=>workflowOrganization.topics.map(t=>`<jui-option value="${esc(t.id)}" data-no-i18n>${esc(t.name)}</jui-option>`).join('');
 const visibleItems=()=>orderedWorkflows().filter(w=>(!filter.value||workflowTopic(w.id)===filter.value)&&w.name.toLocaleLowerCase(nodoLocale()).includes(search.value.trim().toLocaleLowerCase(nodoLocale())));
 const selection=()=>{$('.workflow-bulk').hidden=!selected.size;$('#workflow-selection-count').textContent=selected.size+' '+uiText('selezionati');$('#move-topic-workflows').disabled=!selected.size||busy;const ids=visibleItems().map(w=>w.id),all=$('#workflow-select-all');all.checked=ids.length>0&&ids.every(id=>selected.has(id));all.indeterminate=!all.checked&&ids.some(id=>selected.has(id))};
 const perform=async action=>{if(busy)return;busy=true;selection();try{await action();refreshTopics();refresh()}catch(e){toast(e.message)}finally{busy=false;selection()}};
 const refreshTopics=()=>{
  const previous=filter.value,destination=$('#workflow-topic-destination').value;
  filter.value=workflowOrganization.topics.some(t=>t.id===previous)?previous:'';
  $('#workflow-topic-filters').innerHTML=[{id:'',name:uiText('Tutti i workflow')},...workflowOrganization.topics].map(t=>`<button type="button" class="topic-filter" data-topic-filter="${esc(t.id)}" aria-pressed="${t.id===filter.value}"><span data-no-i18n>${esc(t.name)}</span><span class="topic-count">${t.id?topicWorkflows(t.id).length:app.workflows.length}</span></button>`).join('');
  $$('[data-topic-filter]').forEach(button=>button.onclick=()=>{if(busy)return;filter.value=button.dataset.topicFilter;selected.clear();refreshTopics();refresh()});
  $('#workflow-topic-destination').innerHTML=options();$('#workflow-topic-destination').value=workflowOrganization.topics.some(t=>t.id===destination)?destination:'unassigned';
  $('#topic-management-rows').innerHTML=workflowOrganization.topics.map((t,i)=>`${i===5?'<p class="topic-visibility-divider">'+uiText('Altri topic')+'</p>':''}<div class="topic-management-row" data-topic-id="${esc(t.id)}"><button type="button" class="icon-button topic-drag-handle" data-topic-drag="${esc(t.id)}" aria-label="${esc(uiText('Riordina topic'))}" title="${esc(uiText('Trascina per riordinare. Usa le frecce su e giù da tastiera.'))}">${icon('grip-vertical')}</button><span data-no-i18n>${esc(t.name)}</span><span class="small">${topicWorkflows(t.id).length}</span>${t.id==='unassigned'?'':`<button type="button" class="icon-button" data-topic-rename="${esc(t.id)}" aria-label="${esc(uiText('Rinomina'))}" title="${esc(uiText('Rinomina'))}">${icon('pencil')}</button><button type="button" class="icon-button danger" data-topic-remove="${esc(t.id)}" aria-label="${esc(uiText('Rimuovi topic'))}" title="${esc(uiText('Rimuovi topic'))}">${icon('trash-2')}</button>`}</div>`).join('');
  const saveOrder=async(ids,id)=>{await perform(()=>editWorkflowTopics({action:'reorder',topics:ids}));$('[data-topic-drag="'+CSS.escape(id)+'"]')?.focus({preventScroll:true})};
  $$('[data-topic-drag]').forEach(handle=>{
   const id=handle.dataset.topicDrag;
   handle.onkeydown=e=>{
    if(!['ArrowUp','ArrowDown'].includes(e.key)||busy)return;e.preventDefault();
    const ids=workflowOrganization.topics.map(t=>t.id),i=ids.indexOf(id),next=i+(e.key==='ArrowUp'?-1:1);
    if(next<0||next>=ids.length)return;[ids[i],ids[next]]=[ids[next],ids[i]];saveOrder(ids,id);
   };
   handle.onpointerdown=e=>{
    if(e.button!==0||busy)return;e.preventDefault();
    const container=$('#topic-management-rows'),row=handle.closest('.topic-management-row'),start={x:e.clientX,y:e.clientY};
    let active=false,target=null,after=false,x=e.clientX,y=e.clientY,frame;
    handle.setPointerCapture(e.pointerId);
    const clearIndicators=()=>$$('.topic-drop-before,.topic-drop-after',container).forEach(el=>el.classList.remove('topic-drop-before','topic-drop-after'));
    const locate=()=>{
     clearIndicators();target=null;
     const bounds=container.getBoundingClientRect();if(x<bounds.left||x>bounds.right||y<bounds.top-20||y>bounds.bottom+20)return;
     const rows=$$('.topic-management-row',container).filter(el=>el!==row&&el.getBoundingClientRect().bottom>bounds.top&&el.getBoundingClientRect().top<bounds.bottom);
     if(!rows.length)return;
     target=rows.find(el=>y<el.getBoundingClientRect().bottom)||rows.at(-1);
     const rect=target.getBoundingClientRect();after=y>rect.top+rect.height/2;
     target.classList.add(after?'topic-drop-after':'topic-drop-before');
    };
    const tick=()=>{
     if(active){const bounds=container.getBoundingClientRect();if(x>=bounds.left&&x<=bounds.right&&y>=bounds.top-20&&y<=bounds.bottom+20){if(y<bounds.top+28)container.scrollTop-=8;else if(y>bounds.bottom-28)container.scrollTop+=8}locate()}
     frame=requestAnimationFrame(tick);
    };
    const move=event=>{x=event.clientX;y=event.clientY;if(!active&&Math.hypot(x-start.x,y-start.y)>5){active=true;row.classList.add('is-dragging')}if(active)locate()};
    const finish=event=>{
     cancelAnimationFrame(frame);document.removeEventListener('keydown',escape,true);handle.removeEventListener('pointermove',move);handle.removeEventListener('pointerup',finish);handle.removeEventListener('pointercancel',cancel);handle.removeEventListener('lostpointercapture',cancel);
     clearIndicators();row.classList.remove('is-dragging');if(handle.hasPointerCapture(e.pointerId))handle.releasePointerCapture(e.pointerId);
     if(event?.type!=='pointerup'||!active||!target||!container.isConnected)return;
     const original=workflowOrganization.topics.map(t=>t.id),ids=original.filter(t=>t!==id),index=ids.indexOf(target.dataset.topicId)+(after?1:0);ids.splice(index,0,id);
     if(ids.some((t,i)=>t!==original[i]))saveOrder(ids,id);
    };
    const cancel=()=>finish(null),escape=event=>{if(event.key==='Escape'){event.preventDefault();event.stopImmediatePropagation();cancel()}};
    handle.addEventListener('pointermove',move);handle.addEventListener('pointerup',finish);handle.addEventListener('pointercancel',cancel);handle.addEventListener('lostpointercapture',cancel);document.addEventListener('keydown',escape,true);frame=requestAnimationFrame(tick);
   };
  });
  $$('[data-topic-rename]').forEach(b=>b.onclick=()=>{
   const row=b.closest('.topic-management-row'),topic=workflowOrganization.topics.find(t=>t.id===b.dataset.topicRename);row.innerHTML=`<form class="rename-topic-form"><input required maxlength="80" aria-label="Nome topic" value="${esc(topic.name)}"><button type="submit">${uiText('Salva')}</button><button type="button">${uiText('Annulla')}</button></form>`;
   $('button[type=button]',row).onclick=refreshTopics;$('form',row).onsubmit=e=>{e.preventDefault();const name=$('input',row).value;perform(()=>editWorkflowTopics({action:'rename',topic:topic.id,name}))};$('input',row).focus();
  });
  $$('[data-topic-remove]').forEach(b=>b.onclick=()=>perform(async()=>{if(await appConfirm({title:'Rimuovi topic',message:uiText('I workflow verranno spostati in Unassigned.'),accept:'Rimuovi topic'}))await editWorkflowTopics({action:'remove',topic:b.dataset.topicRemove})}));
 };
 const refresh=()=>{
  const items=visibleItems();$('#workflow-picker-rows').innerHTML=items.map(w=>`<tr><td><jui-switch data-workflow-check="${esc(w.id)}" aria-label="${esc(uiText('Seleziona')+' '+w.name)}" ${selected.has(w.id)?'checked':''}></jui-switch></td><td><button type="button" class="workflow-choice" data-workflow-choice="${esc(w.id)}"><span data-no-i18n>${esc(w.name)}</span><span class="workflow-topic-label" data-no-i18n>${esc(workflowOrganization.topics.find(t=>t.id===workflowTopic(w.id))?.name)}</span></button></td><td><button type="button" class="icon-button danger" data-workflow-remove="${esc(w.id)}" aria-label="${esc(uiText('Elimina workflow'))}" title="${esc(uiText('Elimina workflow'))}">${icon('trash-2')}</button></td></tr>`).join('');
  $('#workflow-picker-empty').hidden=items.length>0;$('#workflow-result-count').textContent=items.length+' / '+app.workflows.length;
  $$('[data-workflow-choice]',$('#workflow-picker-rows')).forEach(b=>b.onclick=()=>{if(!busy)chooseWorkflow(b.dataset.workflowChoice)});
  $$('[data-workflow-check]').forEach(c=>c.onchange=()=>{c.checked?selected.add(c.dataset.workflowCheck):selected.delete(c.dataset.workflowCheck);selection()});
  $$('[data-workflow-remove]').forEach(b=>b.onclick=()=>perform(async()=>{const id=b.dataset.workflowRemove;if(await deleteWorkflowById(id))selected.delete(id)}));selection();
 };
 $('#create-topic-form').onsubmit=e=>{e.preventDefault();const name=$('#new-topic-name').value;perform(async()=>{await editWorkflowTopics({action:'create',name});$('#new-topic-name').value=''})};
 $('#move-topic-workflows').onclick=()=>perform(async()=>{await editWorkflowTopics({action:'move',workflows:[...selected],topic:$('#workflow-topic-destination').value});selected.clear()});
 $('#workflow-select-all').onchange=e=>{for(const w of visibleItems())e.target.checked?selected.add(w.id):selected.delete(w.id);refresh()};
 $('#clear-workflow-selection').onclick=()=>{selected.clear();refresh()};search.oninput=refresh;refreshTopics();refresh();search.focus();
 }catch(e){toast(e.message)}
}
let workflowTemplates=[],creatingWorkflow=false;
let templateSections=[];
function emptyWorkflowDialog(){modal('Nuovo workflow',input('Nome','workflow_name','Nuovo workflow'),'<button class="primary" data-action="create-workflow">Crea workflow</button>');$('[name="workflow_name"]').focus()}
function templatePreview(t){
 const kinds=t.workflow.nodes.some(n=>n.kind==='Connection'&&n.provider!=='local')?['Connection','Task','Review']:t.workflow.nodes.some(n=>n.command==='implement')?['Source','Task','Gate']:['Source','Task','Review'];
 return `<span class="template-preview" aria-hidden="true">${kinds.map((kind,i)=>`${i?'<span class="template-line"></span>':''}<span class="template-preview-node">${icon(icons[kind])}</span>`).join('')}</span>`;
}
async function showWorkflowCatalog(){
 try{
 const catalog=await api('/template-catalog');
 templateSections=catalog.sections.map(s=>({...s,...catalogText(s)}));
 workflowTemplates=catalog.templates.map(t=>({...t,setup:catalogText(t).setup,workflow:{...t.workflow,name:catalogText(t).title,description:catalogText(t).description}}));
 modal('Nuovo workflow',`<div class="workflow-catalog"><section class="catalog-empty"><div><h3>Parti da zero</h3><p>Crea un workflow vuoto e scegli il nome.</p></div><button type="button" class="primary" data-action="empty-workflow">${icon('plus')}Crea workflow vuoto</button></section><section class="catalog-empty"><div><h3>Importa workflow</h3><p>Carica un workflow da un file JSON.</p></div><button type="button" data-action="import-workflow">${icon('upload')}Importa workflow</button></section><div class="catalog-templates"><p class="catalog-intro">Oppure scegli un modello: potrai confermare o modificare il nome della tua copia personale.</p>${templateSections.map(section=>`<section class="catalog-section" aria-labelledby="catalog-${section.id}"><h3 data-no-i18n id="catalog-${section.id}">${esc(section.title)}</h3><p data-no-i18n class="catalog-section-description">${esc(section.description)}</p><div class="template-grid">${workflowTemplates.filter(t=>t.section===section.id).map(t=>`<button type="button" class="template-card" data-action="create-template" data-template-id="${esc(t.id)}">${templatePreview(t)}<strong data-no-i18n>${esc(t.workflow.name)}</strong><span data-no-i18n class="template-description">${esc(t.workflow.description)}</span><span class="template-services">${t.services.length?t.services.map(service=>`<span data-no-i18n>${esc(service)}</span>`).join(''):'<span>Locale</span>'}</span><span data-no-i18n class="template-setup">${esc(t.setup)}</span><span class="template-card-action">Crea workflow <span aria-hidden="true">→</span></span></button>`).join('')}</div></section>`).join('')}</div></div>`,'',true);
 }catch(e){toast(e.message)}
}
function templateWorkflowName(title,date=new Date()){
 const pad=n=>String(n).padStart(2,'0');
 const base=title.normalize('NFKD').replace(/[\u0300-\u036f]/g,'').replace(/[^a-zA-Z0-9]+/g,'_').replace(/^_|_$/g,'').toUpperCase();
 let name;
 do {name=`${base}_${pad(date.getMonth()+1)}${pad(date.getDate())}${pad(date.getFullYear()%100)}${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;date=new Date(date.getTime()+1000)} while(app.workflows.some(w=>w.name===name));
 return name;
}
function confirmTemplateWorkflow(id){
 if(creatingWorkflow)return;
 const template=workflowTemplates.find(t=>t.id===id);if(!template)throw Error('Modello non disponibile');
 const name=templateWorkflowName(template.workflow.name);
 modal('Conferma creazione workflow',`<p data-no-i18n>${esc(template.workflow.description)}</p><p>Conferma il nome proposto o modificalo prima di salvare il nuovo workflow.</p><form id="template-workflow-form">${input('Nome','workflow_name',name)}</form>`,
  `<button type="button" data-action="template-back">Indietro</button><div class="spacer"></div><button type="button" data-action="close-dialog">Chiudi</button><button type="button" class="primary" data-action="save-template" data-template-id="${esc(id)}">Salva</button>`);
 const field=$('#template-workflow-form [name="workflow_name"]');field.required=true;field.maxLength=160;field.focus();field.select();
 $('#template-workflow-form').onsubmit=e=>{e.preventDefault();$('[data-action="save-template"]').click()};
}
async function createTemplateWorkflow(id){
 if(creatingWorkflow)return;
 const template=workflowTemplates.find(t=>t.id===id);if(!template)throw Error('Modello non disponibile');
 const form=$('#template-workflow-form');if(!form||!form.reportValidity())return;
 const name=form.elements.workflow_name.value.trim();if(!name)throw Error('Nome obbligatorio');
 creatingWorkflow=true;
 const buttons=$$('#dialog-content button');buttons.forEach(b=>b.disabled=true);
 try{
 if(app.dirty)await saveWorkflow();
 const w=structuredClone(template.workflow);w.id=crypto.randomUUID();w.created=0;w.name=name;
 const saved=await api('/workflows',w);
 app.workflows.push(saved);selectWorkflow(saved.id);closeModal();toast('Workflow creato dal modello');
 }finally{creatingWorkflow=false;buttons.forEach(b=>b.disabled=false)}
}
function updateViewContext(){
 if(!app.wf)return;
 const mode=app.run?'run':'design',context=$('#view-context');
 context.dataset.mode=mode;document.body.dataset.workspaceMode=mode;
 $('#workspace-mode').textContent=app.run?'Esecuzione in corso · '+app.run.id.slice(0,8):'Progettazione in corso';
 context.title=app.run?app.run.id:app.editBase?.id||'';
 if(app.editBase)$('#workspace-mode').textContent='Progettazione in corso · '+app.editBase.id.slice(0,8);
 $('#edit-continue').hidden=!app.run;$('#edit-continue').disabled=!!app.run?.parent||editingRun;$('#revision-preview').hidden=!app.editBase;$('#revision-cancel').hidden=!app.editBase;$('#execute').innerHTML=icon(app.editBase?'git-compare-arrows':'play')+esc(uiText(app.editBase?'Anteprima ripartenza':'Esegui workflow'));
 updateAutosaveStatus();
 $('#run-status').hidden=!app.run&&!app.editBase;
 $('#run-status').textContent=app.run?(states[app.run.status]||app.run.status)+' · '+uiText('Workflow in sola lettura'):uiText(app.editBase?'Bozza per una nuova ripartenza':'Bozza modificabile');
 const button=$('#latest-run'),matching=latestRunWorkflow===currentWorkflow().id;
 button.disabled=openingLatestRun||latestRunLoading||!matching||!latestRun;
 button.setAttribute('aria-busy',String(openingLatestRun||latestRunLoading));
 button.dataset.tip=latestRunLoading?uiText('Caricamento ultima run…'):latestRun&&matching?uiText('Apri l’ultima run del workflow')+' · '+latestRun.id.slice(0,8):uiText('Nessuna esecuzione per questo workflow.');
}
async function refreshLatestRun(workflow){
 const request=++latestRunRequest,owner=app.user?.id;
 latestRunWorkflow=workflow;latestRun=null;latestRunLoading=true;updateViewContext();
 try{
  const runs=await api('/runs?workflow_id='+encodeURIComponent(workflow));
  if(request!==latestRunRequest||owner!==app.user?.id)return null;
  latestRun=runs.find(run=>run.workflow_id===workflow)||null;
  return latestRun;
 }finally{if(request===latestRunRequest&&owner===app.user?.id){latestRunLoading=false;updateViewContext()}}
}
function selectWorkflow(id){leaveDraftContext();app.editBase=null;app.selectedNodes=[];latestRunWorkflow='';cancelEdgeDrag();app.link=null;deletedNodes=[];closeNodeMenu();disconnect();app.wf=structuredClone(app.workflows.find(w=>w.id===id)||{id:id||crypto.randomUUID(),name:'Nuovo workflow',description:'',entry:'',nodes:[],edges:[]});app.run=null;app.children=[];app.reviewRun=null;app.selected=null;app.selectedEdge=null;app.artifacts=[];app.decisions=[];app.dirty=false;app.pan={x:30,y:40};app.zoom=.85;restoreLocalDraft();workflowOptions(app.wf.id);rememberWorkflow(app.wf.id);render();if(app.view==='history')refreshHistory().catch(e=>toast(e.message))}
const currentWorkflow=()=>app.run?.workflow||app.wf;
const draftSaves=new Map();
const draftPrefix=()=>`nodo.workflowDraft.${app.user?.id}.`;
function draftState(){if(!app.user||!app.wf)return null;const key=draftPrefix()+app.wf.id;let state=draftSaves.get(key);if(!state){state={key,owner:app.user.id,id:app.wf.id,version:0,saved:0,snapshot:null,promise:null,timer:null,error:null,local:false};draftSaves.set(key,state)}return state}
function journalDraft(state){try{localStorage.setItem(state.key,JSON.stringify({workflow:state.snapshot,editBase:state.editBase,updated:Date.now()}));state.local=true}catch{state.local=false}}
function updateAutosaveStatus(){
 const label=$('#dirty');if(!label||!app.wf)return;label.hidden=!!app.run;if(app.run){const retry=$('#draft-retry');if(retry)retry.hidden=true;return}
 const state=draftSaves.get(draftPrefix()+app.wf.id),pending=state&&state.version>state.saved;
 const status=state?.error?'error':pending?'saving':app.workflows.some(w=>w.id===app.wf.id)?'saved':'draft';
 label.dataset.saveState=status;label.setAttribute('role','status');label.setAttribute('aria-live','polite');
 label.textContent=uiText(status==='error'?(state.local?'Salvato sul dispositivo':'Salvataggio non riuscito'):status==='saving'?'Salvataggio…':status==='saved'?'Salvato':'Bozza non salvata');
 const retry=$('#draft-retry');if(retry)retry.hidden=!state?.error;
 label.title=state?.error||'';$('#save').dataset.tip=uiText(state?.error?'Riprova salvataggio':'Salva workflow');
}
function scheduleDraft(state,delay=650){if(state.discarded)return;clearTimeout(state.timer);state.timer=setTimeout(()=>{state.timer=null;persistDraft(state).catch(()=>{})},delay)}
function queueDraft(){
 if(app.run||!app.wf||!app.user)return null;const state=draftState();const snapshot=structuredClone(app.wf);
 if(JSON.stringify(snapshot)!==JSON.stringify(state.snapshot)){state.snapshot=snapshot;state.editBase=app.editBase;state.version++;state.error=null;journalDraft(state)}
 app.dirty=state.version>state.saved;scheduleDraft(state);updateAutosaveStatus();return state;
}
function dirty(){app.dirty=true;queueDraft()}
async function persistDraft(state){
 if(state.promise)return state.promise;
 if(state.discarded||state.version<=state.saved||state.owner!==app.user?.id)return;
 clearTimeout(state.timer);state.timer=null;
 const version=state.version,snapshot=structuredClone(state.snapshot);state.error=null;
 state.promise=(async()=>{
  try{
   const saved=await api('/workflows',snapshot);
   state.saved=version;
   if(!state.discarded&&app.user?.id===state.owner){const i=app.workflows.findIndex(w=>w.id===saved.id),refreshOptions=i<0||app.workflows[i].name!==saved.name||app.workflows[i].created!==saved.created;if(i>=0)app.workflows[i]=saved;else app.workflows.push(saved);
    if(app.wf?.id===saved.id&&!app.run){if(!app.wf.created)app.wf.created=saved.created;app.dirty=state.version>state.saved;if(refreshOptions)workflowOptions(app.wf.id);if(i<0)rememberWorkflow(saved.id);$('#delete-workflow').disabled=false;$('#rename-workflow').disabled=false}
   }
   if(state.version===version){if(state.editBase)journalDraft(state);else try{localStorage.removeItem(state.key)}catch{}}
   return saved;
  }catch(error){state.error=error.message;journalDraft(state);throw error}
  finally{state.promise=null;if(state.owner===app.user?.id){updateAutosaveStatus();if(state.version>state.saved&&!state.error)scheduleDraft(state)}}
 })();updateAutosaveStatus();return state.promise;
}
function localDraft(id){try{const d=JSON.parse(localStorage.getItem(draftPrefix()+id));return d?.workflow?.id===id&&Array.isArray(d.workflow.nodes)&&Array.isArray(d.workflow.edges)?d:null}catch{return null}}
function pendingDraftId(){try{let latest=null;for(let i=0;i<localStorage.length;i++){const key=localStorage.key(i);if(key?.startsWith(draftPrefix())){const d=localDraft(key.slice(draftPrefix().length));if(d&&(!latest||d.updated>latest.updated))latest=d}}return latest?.workflow.id}catch{return null}}
function restoreLocalDraft(){
 const d=localDraft(app.wf.id);if(!d)return;app.wf=d.workflow;app.editBase=d.editBase||null;app.dirty=true;queueDraft();toast('Modifiche recuperate dal dispositivo. Salvataggio automatico in corso.');
}
function leaveDraftContext(){
 if(!app.editBase||!app.wf)return;const state=draftSaves.get(draftPrefix()+app.wf.id);if(!state)return;
 state.editBase=null;if(state.version>state.saved)journalDraft(state);else try{localStorage.removeItem(state.key)}catch{}
}
window.addEventListener('online',()=>{for(const state of draftSaves.values())if(state.owner===app.user?.id&&state.version>state.saved)scheduleDraft(state,0)});
window.addEventListener('beforeunload',e=>{if(app.run||!app.wf)return;captureInspectorFields();const state=draftSaves.get(draftPrefix()+app.wf.id);if(app.dirty&&state&&!state.local){e.preventDefault();e.returnValue=''}});
document.addEventListener('visibilitychange',()=>{if(document.visibilityState==='hidden'&&!app.run&&app.wf){captureInspectorFields();const state=draftSaves.get(draftPrefix()+app.wf.id);if(state?.version>state?.saved)persistDraft(state).catch(()=>{})}});
let editingRun=false,revisionRequest=0;
$('#dialog').addEventListener('close',()=>{++revisionRequest});
async function beginRunRevision(){
 if(!app.run||editingRun)return;const id=app.run.id;editingRun=true;updateViewContext();
 try{
  let d=await api('/runs/'+id);
  if(!d.revision_available){
   if(!['running','starting'].includes(d.run.status))throw Error('Completa le attività dei workflow figli o metti in pausa la run prima di modificarla');
   if(!await appConfirm({title:'Modifica e continua',message:'Mettere in pausa la run al termine del nodo attivo per modificarne il workflow?',accept:'Metti in pausa e modifica'}))return;
   await api('/runs/'+id+'/pause',{});toast('Pausa richiesta. Attendi la fine del nodo attivo.');
   for(let i=0;i<180;i++){await new Promise(r=>setTimeout(r,500));if(app.run?.id!==id)return;d=await api('/runs/'+id);if(d.revision_available)break;if(!['running','starting','paused'].includes(d.run.status))break}
   if(!d.revision_available)throw Error('La run non è ancora pronta. Attendi la pausa e riprova Modifica e continua.');
  }
  if(app.run?.id!==id)return;
  disconnect();app.editBase={id,name:d.run.workflow.name};app.wf=structuredClone(d.run.workflow);app.run=null;app.selected=null;app.selectedNodes=[];app.selectedEdge=null;app.artifacts=[];app.decisions=[];app.children=[];app.reviewRun=null;app.view='graph';deletedNodes=[];app.link=null;cancelEdgeDrag();dirty();render();
  toast('Modifica il workflow, poi apri Anteprima ripartenza.');
 }catch(e){toast(e.message)}finally{editingRun=false;if(app.wf)updateViewContext()}
}
const revisionReasons={new:'Nodo aggiunto',changed:'Configurazione o collegamenti modificati',policy:'Modalità di esecuzione modificata',unfinished:'Non completato con successo',evidence:'Verifica o operazione da rinnovare',dependency:'Dipende da un passaggio da rieseguire',restart:'Ripartenza richiesta',compatible:'Risultato precedente compatibile'};
function revisionSteps(steps){return steps.length?'<ul class="revision-steps">'+steps.map(s=>`<li><strong data-no-i18n>${esc(s.title)}</strong><small>${esc(s.kind)} · ${esc(uiText(revisionReasons[s.reason]||s.reason))}</small>${s.summary?`<p class="revision-result" data-no-i18n>${esc(s.summary)}</p>`:''}</li>`).join('')+'</ul>':`<p class="note">${uiText('Nessuno')}</p>`}
async function previewRevision(node=null){
 if(!app.editBase)return;try{await saveWorkflow()}catch(error){toast(error.message);return}const base=app.editBase.id,workflow=structuredClone(app.wf),version=++revisionRequest;
 try{
  const p=await api('/runs/'+base+'/revision/preview',{workflow,node});
  if(version!==revisionRequest||app.editBase?.id!==base)return;
  const body=`<p class="note">${uiText('Verrà creata una nuova run. La run originale resta nella cronologia. I file correnti vengono copiati, senza annullare le modifiche già fatte; .git, target e node_modules sono esclusi. Anche le Source permanenti vengono copiate nel nuovo workspace.')}</p><label>${uiText('Riparti dal nodo')}<jui-select id="revision-node"><jui-option value="">${uiText('Automatico: primo passaggio da eseguire')}</jui-option>${p.candidates.map(n=>`<jui-option data-no-i18n value="${esc(n.id)}" ${node===n.id?'selected':''}>${esc(n.title)}</jui-option>`).join('')}</jui-select></label><p class="note">${p.restart?uiText('La ripartenza inizierà da')+' <strong data-no-i18n>'+esc(workflow.nodes.find(n=>n.id===p.restart)?.title||p.restart)+'</strong>':uiText('Tutti i passaggi del percorso sono già completati. Scegli un nodo precedente da rieseguire.')}</p><div class="revision-columns"><section><h3>${uiText('Risultati mantenuti')} (${p.kept.length})</h3>${revisionSteps(p.kept)}</section><section><h3>${uiText('Nodi da eseguire di nuovo')} (${p.rerun.length})</h3>${revisionSteps(p.rerun)}<p class="note">${uiText('I rami condizionali verranno seguiti secondo gli esiti della nuova run.')}</p></section></div>${p.removed.length?`<section><h3>${uiText('Nodi rimossi')} (${p.removed.length})</h3>${revisionSteps(p.removed)}</section>`:''}<details><summary>${uiText('Versioni degli artefatti mantenute')} (${p.artifacts.length})</summary><ul class="revision-files">${p.artifacts.map(a=>`<li data-no-i18n>${esc(a.path)} · v${a.version}</li>`).join('')}</ul></details><details><summary>${uiText('File copiati nel nuovo workspace')} (${p.workspace_files.length})</summary><ul class="revision-files">${p.workspace_files.map(path=>`<li data-no-i18n>${esc(path)}</li>`).join('')}</ul></details><p class="note">${uiText('Le approvazioni dei passaggi da rieseguire e le autorizzazioni a operazioni esterne vanno richieste di nuovo. Le sessioni degli agenti e i contatori della nuova run ripartono da zero; i risultati mantenuti restano disponibili come contesto.')}</p><p id="revision-error" class="error" role="alert"></p>`;
  modal('Anteprima ripartenza',body,`<button type="button" data-action="close-dialog">${uiText('Torna alle modifiche')}</button><div class="spacer"></div><button type="button" class="primary" id="revision-confirm" ${p.restart?'':'disabled'}>${icon('play')}${uiText('Crea run e continua')}</button>`,true);
  $('#revision-node').onchange=e=>{const confirm=$('#revision-confirm');confirm.disabled=true;previewRevision(e.target.value||null)};
  $('#revision-confirm').onclick=async e=>{const button=e.currentTarget;button.disabled=true;$('#revision-node').disabled=true;
   try{const r=await api('/runs/'+base+'/revision',{workflow,node,token:p.token,acknowledge:true});if(app.editBase?.id!==base)return;closeModal();app.workflows=await api('/workflows');app.dirty=false;await loadRun(r.id);logs(true);toast(r.status==='paused'?'Nuova run pronta. Premi Riprendi per avviarla.':'Nuova run avviata conservando il lavoro compatibile')}
   catch(error){const output=$('#revision-error');if(output){output.textContent=error.message;const refresh=document.createElement('button');refresh.type='button';refresh.textContent=uiText('Aggiorna anteprima');refresh.onclick=()=>previewRevision(node);output.append(document.createElement('br'),refresh)}}
  };
 }catch(error){toast(error.message)}
}
async function saveWorkflow(){
 if(app.run)throw Error('Torna alla progettazione prima di modificare il workflow');
 captureInspectorFields();const state=queueDraft(),owner=app.user?.id,id=app.wf.id;
 if(!state)throw Error('Accedi per continuare');
 // Serialize saves; a late response must never replace a newer draft or focused field.
 while(state.version>state.saved){await persistDraft(state);if(app.user?.id!==owner)throw Error('Accedi per continuare')}
 if(app.wf?.id===id&&!app.run){app.dirty=false;updateAutosaveStatus()}
 return app.workflows.find(w=>w.id===id)||state.snapshot;
}
function downloadWorkflow(){
 if(!app.run)captureInspectorFields();
 const workflow=currentWorkflow();if(!workflow)return;
 const blob=new Blob([JSON.stringify(workflow,null,2)+'\n'],{type:'application/json'}),url=URL.createObjectURL(blob),link=document.createElement('a');
 link.href=url;link.download=(workflow.name.replace(/[^\p{L}\p{N}._-]+/gu,'-').slice(0,120)||'workflow')+'.json';
 document.body.append(link);link.click();link.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);
}
function parseWorkflowImport(text){
 let workflow;try{workflow=JSON.parse(text)}catch{throw Error(uiText('File JSON non valido.'))}
 const fail=()=>{throw Error(uiText('Il file non contiene un workflow valido.'))};
 if(!workflow||typeof workflow!=='object'||Array.isArray(workflow)||typeof workflow.name!=='string'||!workflow.name.trim()||workflow.name.length>160||!Array.isArray(workflow.nodes)||!Array.isArray(workflow.edges)||workflow.nodes.length>200||workflow.edges.length>600)fail();
 const ids=new Set();
 for(const node of workflow.nodes){if(!node||typeof node!=='object'||typeof node.id!=='string'||!node.id||ids.has(node.id)||!Object.hasOwn(icons,node.kind))fail();ids.add(node.id)}
 if(typeof workflow.entry!=='string'||workflow.entry&&!ids.has(workflow.entry))fail();
 for(const edge of workflow.edges)if(!edge||!ids.has(edge.from)||!ids.has(edge.to)||!['context','control','artifact','connection','rework'].includes(edge.kind))fail();
 if(workflow.description!==undefined&&typeof workflow.description!=='string')fail();
 return {...workflow,id:crypto.randomUUID(),created:0,description:workflow.description||''};
}
function importWorkflowDialog(){
 modal('Importa workflow',`<div id="workflow-import"><p class="note">Scegli o trascina un workflow JSON. Verrà importato come nuovo workflow.</p><label class="workflow-import-drop" id="workflow-import-drop">${icon('upload')}<span>Scegli un file o trascinalo qui</span><input type="file" id="workflow-import-file" accept=".json,application/json"></label><p class="small" id="workflow-import-preview" role="status" data-no-i18n></p><p class="error" id="workflow-import-error" role="alert" hidden></p></div>`,`<button type="button" data-action="close-dialog">Chiudi</button><button type="button" id="workflow-import-confirm" class="primary" disabled>${icon('upload')}Importa</button>`);
 const root=$('#workflow-import'),input=$('#workflow-import-file'),drop=$('#workflow-import-drop'),confirm=$('#workflow-import-confirm'),error=$('#workflow-import-error'),preview=$('#workflow-import-preview');
 let workflow=null,version=0,busy=false;
 const showError=message=>{error.textContent=message;error.hidden=false};
 const read=async files=>{
  if(busy)return;const request=++version;workflow=null;confirm.disabled=true;error.hidden=true;preview.textContent='';
  if(files.length!==1){showError(uiText('Seleziona un solo file JSON.'));return}
  const file=files[0];if(file.size>1024*1024){showError(uiText('Il file supera il limite di 1 MB.'));return}
  try{const text=await file.text();if(request!==version||!root.isConnected)return;workflow=parseWorkflowImport(text);preview.textContent=file.name+' · '+workflow.name+' · '+workflow.nodes.length+' '+uiText('nodi')+' · '+workflow.edges.length+' '+uiText('collegamenti');confirm.disabled=false}
  catch(e){if(request===version&&root.isConnected)showError(e.message)}
 };
 input.onchange=()=>read([...input.files]);
 drop.ondragover=e=>{e.preventDefault();if(!busy)drop.classList.add('drag-over')};
 drop.ondragleave=()=>drop.classList.remove('drag-over');
 root.ondrop=e=>{e.preventDefault();drop.classList.remove('drag-over');read([...e.dataTransfer.files])};
 root.ondragover=e=>e.preventDefault();
 confirm.onclick=async()=>{
  if(!workflow||busy)return;busy=true;confirm.disabled=true;input.disabled=true;error.hidden=true;
  const owner=app.user?.id;
  try{if(app.dirty&&!app.run)await saveWorkflow();const saved=await api('/workflows',workflow);if(app.user?.id!==owner)return;app.workflows.push(saved);app.view='graph';selectWorkflow(saved.id);if(root.isConnected)closeModal();toast('Workflow importato')}
  catch(e){if(root.isConnected)showError(e.message)}
  finally{busy=false;confirm.disabled=!workflow;input.disabled=false}
 };
}

function state(n){const s=app.run?.states[n.id]||'queued';return app.run?.status==='interrupted'&&['running','starting','waiting_external','waiting_child'].includes(s)?'interrupted':s}
const executionModes={interactive:'Interattiva',assisted:'Assistita',unattended:'Unattended'};
function executionPolicy(value){return {mode:'interactive',runtime:'',model:'',instructions:'',max_decisions:50,max_per_node:5,allow_external_writes:false,...value}}
function policyFields(value){const p=executionPolicy(value);return `<div class="execution-policy-fields">${select('Modalità di esecuzione','execution_mode',p.mode,Object.entries(executionModes))}<p class="note">Interattiva: rispondi tu. Assistita: l’AI propone e tu confermi. Unattended: l’AI decide e prosegue entro i limiti configurati.</p><div class="form-grid">${select('Agente supervisore','supervisor_runtime',p.runtime,[['','Preferenza personale'],['codex','Codex'],['claude','Claude Code'],['pi','Pi Agent']])}${input('Modello supervisore (vuoto = personale)','supervisor_model',p.model)}</div><label>Istruzioni del supervisore<textarea name="supervisor_instructions" rows="5" maxlength="32000" placeholder="Obiettivi, criteri di qualità e scelte preferite">${esc(p.instructions)}</textarea></label><div class="form-grid"><label>Decisioni AI massime per run<input name="max_decisions" type="number" min="1" max="200" required value="${p.max_decisions}" aria-describedby="supervisor-limits-note"></label><label>Decisioni AI massime per nodo<input name="max_per_node" type="number" min="1" max="20" required value="${p.max_per_node}" aria-describedby="supervisor-limits-note"></label></div><p id="supervisor-limits-note" class="note">Questi limiti contano solo gli interventi del supervisore AI, non tutte le chiamate agli agenti o la chat del manager. Il limite per run vale per l’intera esecuzione; quello per nodo vale sullo stesso nodo, anche se ripetuto. Al raggiungimento di uno dei limiti, l’esecuzione si interrompe.</p><div class="storage-hidden-control"><span id="external-writes-label">Consenti al supervisore di approvare operazioni esterne</span><button type="button" class="storage-hidden-toggle" role="switch" data-action="toggle-external-writes" aria-labelledby="external-writes-label" aria-checked="${p.allow_external_writes}"><span aria-hidden="true"></span></button><input type="hidden" name="allow_external_writes" value="${p.allow_external_writes}"></div><p class="note">Il supervisore usa le credenziali personali e una sessione dedicata. Non modifica i file. Le review continuano a verificare il lavoro; gli errori tecnici interrompono il run. La modalità e le istruzioni sono ereditate dai workflow figli.</p></div>`}
function readPolicy(form){const f=new FormData(form);return {mode:f.get('execution_mode'),runtime:f.get('supervisor_runtime'),model:f.get('supervisor_model'),instructions:f.get('supervisor_instructions'),max_decisions:Number(f.get('max_decisions')),max_per_node:Number(f.get('max_per_node')),allow_external_writes:f.get('allow_external_writes')==='true'}}
function policyDialog(){const p=executionPolicy(app.run?.execution||app.wf.execution);modal('Modalità di esecuzione',`<form id="execution-policy-form"><fieldset ${app.run?'disabled':''}>${policyFields(p)}</fieldset></form>${app.run?'<p class="note">Configurazione congelata per questo run.</p>':''}`,`${app.run?'<button data-action="decision-history">'+icon('history')+'Registro decisioni</button><div class="spacer"></div>':''}<button data-action="close-dialog">Chiudi</button>${app.run?'':'<button class="primary" data-action="save-execution-policy">'+icon('save')+'Applica al workflow</button>'}`,true)}
function aiDecisionBadge(node){const count=app.decisions.filter(d=>d.node===node&&d.actor==='ai').length;return count?`<span class="ai-decision-badge">${icon('bot')}<span>AI · ${count}</span></span>`:''}
function render(){if(!app.wf)return;if(!app.selected||!app.selectedNodes.includes(app.selected))app.selectedNodes=[];$('#execution-mode-label').textContent=executionModes[(app.run?.execution||app.wf.execution)?.mode||'interactive'];$('#delete-workflow').disabled=!app.workflows.some(w=>w.id===currentWorkflow().id);$('#rename-workflow').disabled=$('#delete-workflow').disabled;$('#dirty').hidden=!app.dirty;$('#execute').disabled=!!app.run&&['running','starting'].includes(app.run.status);$('#save').disabled=!!app.run;$('#pause').hidden=!app.run||!['running','starting','waiting_child','waiting_review','waiting_user'].includes(app.run.status);$('#stop').hidden=$('#pause').hidden;$('#resume').hidden=!app.run||!['paused','failed','interrupted'].includes(app.run.status);$('#close-run').hidden=!app.run;$('#run-storage').hidden=!app.run;$('#run-family').hidden=!app.run||(!app.run.parent&&!app.children.length);$('#add-context').hidden=!app.run;$('#add-context').disabled=!app.run||!['paused','waiting_review','waiting_user'].includes(app.run.status);updateViewContext();if(latestRunWorkflow!==currentWorkflow().id)refreshLatestRun(currentWorkflow().id).catch(e=>toast(e.message));const pending=app.run?.pending;$('#review-banner').hidden=!pending&&!app.reviewRun;if(!pending&&app.reviewRun)$('#review-banner span:last-child').textContent='Review nel workflow figlio';$('#review-banner').disabled=!!pending&&app.run.status==='running'&&(app.run.execution?.mode||'interactive')!=='interactive';if(pending)$('#review-banner span:last-child').textContent=app.run.status==='running'&&app.run.execution?.mode!=='interactive'?'Supervisore AI in valutazione':pending.proposal?'Proposta AI da confermare':pending.kind==='question'?'Domanda in attesa':pending.kind==='gate'?'Gate in attesa':'Review richiesta';$('#graph').hidden=app.view!=='graph';$('#execution-list').hidden=app.view!=='list';$('#run-history').hidden=app.view!=='history';$('#palette').hidden=app.view==='history';$('#graph-tab').classList.toggle('active',app.view==='graph');$('#list-tab').classList.toggle('active',app.view==='list');$('#history-tab').classList.toggle('active',app.view==='history');for(const id of ['graph','list','history'])$('#'+id+'-tab').setAttribute('aria-selected',String(app.view===id));renderGraph();renderList();renderHistory();renderInspector();renderActivityLog();window.nodoManagerContextChanged?.();}
function selectedNodeIds(){
 const workflow=currentWorkflow();
 if(app.run||!app.selected)return app.selected?[app.selected]:[];
 return app.selectedNodes.includes(app.selected)?app.selectedNodes.filter(id=>workflow.nodes.some(n=>n.id===id)):[app.selected];
}
function setNodeSelection(ids){app.selectedNodes=[...new Set(ids)];app.selected=app.selectedNodes[0]||null;app.selectedEdge=null;app.link=null}
function paintNodeSelection(){const ids=new Set(selectedNodeIds());$$('#nodes .node').forEach(node=>node.classList.toggle('selected',ids.has(node.dataset.id)));updateEdgeSelection()}
const commentMarkdownCache=new Map();
const commentResizeObserver=new ResizeObserver(entries=>{if(app.run)return;for(const {target} of entries){if(!target.isConnected)continue;const n=app.wf?.nodes.find(n=>n.id===target.dataset.id&&n.kind==='Comment');if(n&&(Math.abs((n.width||300)-target.offsetWidth)>1||Math.abs((n.height||220)-target.offsetHeight)>1)){n.width=target.offsetWidth;n.height=target.offsetHeight;dirty()}}});
function renderComment(n){
 const preview=!!n.markdown;
 return `<section class="node comment-node ${selectedNodeIds().includes(n.id)?'selected':''} ${app.run?'is-readonly':''}" data-id="${esc(n.id)}" data-kind="Comment" style="left:${n.x}px;top:${n.y}px;width:${n.width||300}px;height:${n.height||220}px"><header class="comment-header"><span>${icon('sticky-note')}<span>Commento</span></span><div class="comment-actions">${!app.run?`<button type="button" class="icon-button" data-comment-preview aria-label="${preview?'Modifica testo':'Anteprima Markdown'}" title="${preview?'Modifica testo':'Anteprima Markdown'}">${icon(preview?'pencil':'eye')}</button><button type="button" class="icon-button" data-comment-delete aria-label="Elimina commento">${icon('trash-2')}</button>`:''}</div></header><div class="comment-body">${preview?'<div class="comment-preview markdown" data-no-i18n></div>':`<textarea class="comment-text" data-no-i18n aria-label="Testo del commento" maxlength="64000" placeholder="Scrivi una nota… Puoi usare Markdown." ${app.run?'readonly':''}>${esc(n.text||'')}</textarea>`}</div></section>`;
}
function bindComments(){
 for(const card of $$('.comment-node')){
  const n=currentWorkflow().nodes.find(n=>n.id===card.dataset.id);
  card.addEventListener('pointerdown',e=>{if(e.target.closest('.comment-body,.comment-actions')||!e.target.closest('.comment-header')||app.run)e.stopPropagation()});
  card.addEventListener('wheel',e=>{if(e.target.closest('.comment-body'))e.stopPropagation()},{passive:true});
  const textarea=$('textarea',card);
  if(textarea&&!app.run){textarea.oninput=()=>{n.text=textarea.value;dirty()};textarea.onfocus=()=>{setNodeSelection([n.id]);paintNodeSelection();renderInspector()}};
  $('[data-comment-delete]',card)?.addEventListener('click',()=>deleteNode(n.id));
  $('[data-comment-preview]',card)?.addEventListener('click',()=>{n.markdown=!n.markdown;dirty();renderGraph();if(!n.markdown)requestAnimationFrame(()=>$('.comment-node[data-id="'+n.id+'"] textarea')?.focus())});
  if(n.markdown){
   const preview=$('.comment-preview',card),text=n.text||'';
   const display=html=>{if(!card.isConnected)return;preview.innerHTML=html;$$('a',preview).forEach(a=>{a.target='_blank';a.rel='noopener noreferrer'})};
   if(commentMarkdownCache.has(text))display(commentMarkdownCache.get(text));
   else{preview.textContent=uiText('Caricamento anteprima…');api('/markdown/preview',{text}).then(d=>{if(commentMarkdownCache.size>100)commentMarkdownCache.clear();commentMarkdownCache.set(text,d.html);display(d.html)}).catch(error=>{if(card.isConnected)preview.textContent=error.message})}
  }
  if(!app.run)commentResizeObserver.observe(card);
 }
}
// Tool labels follow execution events; only a new start animates the next name.
const nodeToolLabels={run:null,latest:new Map(),seen:0};
function syncNodeToolLabels(){
 const run=app.run;
 if(nodeToolLabels.run!==run?.id){nodeToolLabels.run=run?.id;nodeToolLabels.latest.clear();nodeToolLabels.seen=0}
 const events=[...(activityLog.runs.get(run?.id)?.events.values()||[])].sort((a,b)=>a.seq-b.seq);
 for(const event of events){
  if(event.seq<=nodeToolLabels.seen)continue;
  nodeToolLabels.seen=event.seq;
  if(event.kind==='activity'){const data=activityData(event);if(data?.phase==='start'&&data.tool)nodeToolLabels.latest.set(event.node,{name:data.tool,seq:event.seq})}
  if(event.kind==='state'){
   if(!['running','starting','waiting_child'].includes(event.text))nodeToolLabels.latest.clear();
   else if(event.node)nodeToolLabels.latest.delete(event.node);
  }
 }
 const active=run&&['running','starting','waiting_child'].includes(run.status);
 for(const card of $$('#nodes > .node')){
  const tool=active&&['running','waiting_external'].includes(state({id:card.dataset.id}))&&nodeToolLabels.latest.get(card.dataset.id);
  let label=$('.node-tool-label',card);
  if(!tool){label?.remove();continue}
  if(label?.dataset.seq===String(tool.seq))continue;
  const previous=label?.lastElementChild?.textContent;
  if(!label){label=document.createElement('span');label.className='node-tool-label';label.setAttribute('data-no-i18n','');card.append(label)}
  label.dataset.seq=tool.seq;
  label.dataset.tip=tool.name;
  label.setAttribute('aria-label',tool.name);
  label.replaceChildren();
  const next=document.createElement('span');next.textContent=tool.name;label.append(next);
  if(!matchMedia('(prefers-reduced-motion: reduce)').matches){
   if(previous){const old=document.createElement('span');old.textContent=previous;old.className='node-tool-previous';label.prepend(old);old.animate([{transform:'translateY(0)'},{transform:'translateY(-100%)'}],{duration:250,easing:'ease-in-out'}).onfinish=()=>old.remove()}
   next.animate([{transform:'translateY(100%)',opacity:0},{transform:'translateY(0)',opacity:1}],{duration:250,easing:'ease-in-out'});
  }
 }
}
function renderGraph(){
  hideTooltip();
  const data=stepFilterData(),w=data.w;
  const focusedFilter=document.activeElement?.closest('#graph-filters')?.querySelector(':focus')?.dataset.filter;
  $('#graph').dataset.stepFilter=app.stepFilter;
  $('#graph-filters').innerHTML=stepFilterToolbar(data,'graph');
  $('#graph-empty').hidden=!!w.nodes.length;
  commentResizeObserver.disconnect();
  const toolLabels=new Map($$('#nodes > .node').map(card=>[card.dataset.id,$('.node-tool-label',card)]));
  $('#nodes').innerHTML=w.nodes.map(n=>{
    if(n.kind==='Comment')return renderComment(n);
    const s=state(n),agent=associatedAgent(n),agentLabel=agent?' · '+nodeLabel(agent):'',matches=stepMatchesFilter(n,app.stepFilter),filterClass=app.stepFilter==='all'?'':matches?'filter-match':'filter-muted';
    const badge=stepMatchesFilter(n,'running')?'loader-circle':s==='completed'?'check':['failed','interrupted'].includes(s)?'x':stepMatchesFilter(n,'review')?'circle-help':null;
    return `<div class="node ${selectedNodeIds().includes(n.id)?'selected':''} ${filterClass}" data-id="${esc(n.id)}" data-kind="${n.kind}" data-status="${s}" data-filter-match="${matches}" style="left:${n.x}px;top:${n.y}px"><button class="node-main" data-action="select-node" data-id="${esc(n.id)}" aria-label="${esc(n.title)} · ${esc(nodeLabel(n)+agentLabel)} · ${esc(states[s])}${app.stepFilter!=='all'&&!matches?' · Fuori filtro':''}" data-tip="${esc(n.title)} · ${esc(nodeLabel(n)+agentLabel)}">${nodeIcon(n)}${associatedAgentBadge(agent)}${badge?`<span class="node-badge">${icon(badge)}</span>`:''}</button><button class="port in" data-action="port-in" data-id="${esc(n.id)}" aria-label="Collega a ${esc(n.title)}"></button><button class="port out ${app.link===n.id?'chosen':''}" data-action="port-out" data-id="${esc(n.id)}" aria-label="Collega da ${esc(n.title)}"></button><span class="node-title">${esc(n.title)}</span><span class="node-meta">${esc(nodeLabel(n))}</span>${s.startsWith('waiting')?`<span class="node-wait">${esc(states[s])}</span>`:''}${aiDecisionBadge(n.id)}</div>`;
  }).join('');
  for(const card of $$('#nodes > .node')){const label=toolLabels.get(card.dataset.id);if(label)card.append(label)}
  syncNodeToolLabels();
  bindComments();drawEdges();transform();
  if(focusedFilter)$('#graph-filters .list-filter[data-filter="'+focusedFilter+'"]').focus({preventScroll:true});
}
function edgeGeometry(w,e){
  const a=w.nodes.find(n=>n.id===e.from),b=w.nodes.find(n=>n.id===e.to);if(!a||!b)return null;
  const x1=a.x+103,y1=a.y+45,x2=b.x+7,y2=b.y+45;
  const returning=e.kind==='rework'||b.x<a.x||(b.x===a.x&&b.y<a.y);
  if(!returning){const c=Math.max(60,Math.abs(x2-x1)/2);return {d:`M${x1} ${y1}C${x1+c} ${y1},${x2-c} ${y2},${x2} ${y2}`,labelX:(x1+x2)/2,labelY:(y1+y2)/2-7}}
  // Prefer a free horizontal corridor between rows, including titles and
  // comments as obstacles. Keep separate lanes for overlapping return edges.
  const left=Math.min(a.x,b.x),right=Math.max(a.x,b.x)+116;
  const index=w.edges.indexOf(e),earlier=index<0?w.edges:w.edges.slice(0,index);
  const lane=earlier.filter(other=>{const s=w.nodes.find(n=>n.id===other.from),t=w.nodes.find(n=>n.id===other.to);return s&&t&&(other.kind==='rework'||t.x<s.x||(t.x===s.x&&t.y<s.y))&&Math.min(s.x,t.x)<=right&&Math.max(s.x,t.x)+116>=left}).length;
  const cards=new Map($$('#nodes .node').map(card=>[card.dataset.id,card]));
  const boxes=w.nodes.map(n=>({id:n.id,left:n.x-8,right:n.x+(n.kind==='Comment'?(n.width||300):124),top:n.y-12,bottom:n.y+(n.kind==='Comment'?(n.height||220):Math.max(150,cards.get(n.id)?.offsetHeight||0))+12}));
  const exit=x1+40+lane*14,entry=x2-40-lane*14;
  const pathLeft=Math.min(x1,x2,entry,exit),pathRight=Math.max(x1,x2,entry,exit);
  const obstacles=boxes.filter(n=>n.right>=pathLeft&&n.left<=pathRight);
  const bands=[];
  for(const box of [...obstacles].sort((a,b)=>a.top-b.top)){const last=bands.at(-1);if(last&&box.top<=last.bottom)last.bottom=Math.max(last.bottom,box.bottom);else bands.push({top:box.top,bottom:box.bottom})}
  const offset=lane===0?0:Math.ceil(lane/2)*22*(lane%2?1:-1);
  const corridors=[];
  // Boxes already include 12px of padding: retain narrow row gaps (200px layouts).
  for(let i=1;i<bands.length;i++){const low=bands[i-1].bottom+4,high=bands[i].top-4,y=(low+high)/2+offset;if(low<=y&&y<=high)corridors.push(y)}
  corridors.sort((u,v)=>Math.abs(y1-u)+Math.abs(y2-u)-Math.abs(y1-v)-Math.abs(y2-v));
  // Distant cards must not enlarge an external route either: use the same
  // local obstacles for corridors, detours and collision checks.
  const outer=[Math.min(...obstacles.map(n=>n.top))-28-lane*24,Math.max(...obstacles.map(n=>n.bottom))+28+lane*24];
  outer.sort((u,v)=>Math.abs(y1-u)+Math.abs(y2-u)-Math.abs(y1-v)-Math.abs(y2-v));
  const clear=points=>points.slice(1).every((p,i)=>!obstacles.some(box=>{
   if(i===0&&box.id===a.id||i===points.length-2&&box.id===b.id)return false;
   const q=points[i];return p.x===q.x?p.x>box.left&&p.x<box.right&&Math.max(p.y,q.y)>box.top&&Math.min(p.y,q.y)<box.bottom:p.y>box.top&&p.y<box.bottom&&Math.max(p.x,q.x)>box.left&&Math.min(p.x,q.x)<box.right;
  }));
  let points;
  for(const y of [...corridors,...outer]){
   const candidate=[{x:x1,y:y1},{x:exit,y:y1},{x:exit,y},{x:entry,y},{x:entry,y:y2},{x:x2,y:y2}];
   if(clear(candidate)){points=candidate;break}
  }
  // The outer route remains available for densely packed or overlapping cards.
  points ||= [{x:x1,y:y1},{x:exit,y:y1},{x:exit,y:outer[0]},{x:entry,y:outer[0]},{x:entry,y:y2},{x:x2,y:y2}];
  const routeY=points[2].y;
  return {d:roundedEdgePath(points),labelX:(exit+entry)/2,labelY:routeY-8,bounds:{x:Math.min(entry,exit),y:Math.min(routeY,y1,y2)-20,width:Math.abs(exit-entry),height:Math.max(routeY,y1,y2)-Math.min(routeY,y1,y2)+40}};
}
function roundedEdgePath(points){
 let d=`M${points[0].x} ${points[0].y}`;
 for(let i=1;i<points.length-1;i++){
  const prev=points[i-1],p=points[i],next=points[i+1],before=Math.hypot(p.x-prev.x,p.y-prev.y),after=Math.hypot(next.x-p.x,next.y-p.y),r=Math.min(24,before/2,after/2);
  if(!r){d+=`L${p.x} ${p.y}`;continue}
  d+=`L${p.x+(prev.x-p.x)*r/before} ${p.y+(prev.y-p.y)*r/before}Q${p.x} ${p.y} ${p.x+(next.x-p.x)*r/after} ${p.y+(next.y-p.y)*r/after}`;
 }
 return d+`L${points.at(-1).x} ${points.at(-1).y}`;
}
function drawEdges(){
  const w=currentWorkflow();
  if(app.selectedEdge&&!w.edges.some(e=>graphEdgeKey(e)===app.selectedEdge))app.selectedEdge=null;
  let out='<defs>'+['control','context','rework','active'].map(kind=>`<marker id="arrow-${kind}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="10" markerHeight="10" markerUnits="userSpaceOnUse" orient="auto"><path class="edge-arrow ${kind}" d="M1 1L9 5L1 9Z"/></marker>`).join('')+'</defs>';
  for(const e of w.edges){
    const a=w.nodes.find(n=>n.id===e.from),b=w.nodes.find(n=>n.id===e.to);if(!a||!b)continue;
    // End before the input port so the filled arrowhead remains fully visible.
    const {d,labelX,labelY}=edgeGeometry(w,e);
    const kind=['context','rework'].includes(e.kind)?e.kind:'control';
    const key=graphEdgeKey(e),selected=key===app.selectedEdge;
    out+=`<path class="edge-hit" data-action="select-edge" data-edge-key="${esc(key)}" d="${d}" tabindex="0" role="button" aria-pressed="${selected}" aria-label="${esc(a.title)} → ${esc(b.title)} · ${esc(e.kind)}${e.outcome?' · '+esc(e.outcome):''}"/><path class="edge ${esc(e.kind)} ${selected?'selected':''}" data-edge-key="${esc(key)}" data-edge-kind="${kind}" data-from="${esc(e.from)}" data-to="${esc(e.to)}" d="${d}" marker-end="url(#arrow-${selected?'active':kind})"/>`;
    const active=app.run?.active_edge;
    if(['running','waiting_child'].includes(app.run?.status)&&active&&active.from===e.from&&active.to===e.to&&active.kind===e.kind&&['running','waiting_external'].includes(app.run.states[e.to]))out+=`<path class="edge signal" data-from="${esc(e.from)}" data-to="${esc(e.to)}" data-kind="${esc(e.kind)}" d="${d}"/>`;
    if(e.outcome)out+=`<text class="edge-label" data-action="select-edge" data-edge-key="${esc(key)}" x="${labelX}" y="${labelY}" text-anchor="middle">${esc(e.outcome)}</text>`;
  }
  $('#edges').innerHTML=out;highlightGraphEdge(null);drawEdgePreview();
}
function transform(){$('#graph-world').style.transform=`translate(${app.pan.x}px,${app.pan.y}px) scale(${app.zoom})`;$('#zoom-value').textContent=Math.round(app.zoom*100)+'%'}
function centerNode(id){const n=currentWorkflow().nodes.find(n=>n.id===id);if(!n)return;const r=$('#graph-canvas').getBoundingClientRect();app.pan={x:r.width/2-(n.x+58)*app.zoom,y:r.height*.42-(n.y+44)*app.zoom};transform()}
function fit(){
 const workflow=currentWorkflow();if(!workflow.nodes.length)return;
 const boxes=workflow.nodes.map(n=>({x:n.x,y:n.y,width:n.kind==='Comment'?(n.width||300):150,height:n.kind==='Comment'?(n.height||220):160}));
 for(const e of workflow.edges){const bounds=edgeGeometry(workflow,e)?.bounds;if(bounds)boxes.push(bounds)}
 const r=$('#graph-canvas').getBoundingClientRect(),x=Math.min(...boxes.map(n=>n.x)),y=Math.min(...boxes.map(n=>n.y)),w=Math.max(...boxes.map(n=>n.x+n.width))-x,h=Math.max(...boxes.map(n=>n.y+n.height))-y;
 app.zoom=Math.min(1,(r.width-80)/w,(r.height-90)/h);app.pan={x:(r.width-w*app.zoom)/2-x*app.zoom,y:(r.height-h*app.zoom)/2-y*app.zoom};transform();
}
const stepFilters=[
  {id:'all',label:'Tutti',short:'Tutti',icon:'list',tip:'Tutti gli step del workflow',empty:'Il workflow non contiene ancora step.'},
  {id:'running',label:'In esecuzione',short:'In corso',icon:'play',tip:'Step attivi e operazioni esterne in corso',empty:'Nessuno step in esecuzione.'},
  {id:'review',label:'In attesa di review',short:'Review',icon:'clipboard-check',tip:'Review, domande e decisioni in attesa di risposta',empty:'Nessuno step in attesa di review o risposta.'},
  {id:'errors',label:'In errore',short:'Errori',icon:'x',tip:'Step in errore o interrotti',empty:'Nessuno step in errore o interrotto.'}
];
function stepMatchesFilter(n,filter){const s=state(n);return filter==='all'||filter==='running'&&['running','waiting_external'].includes(s)||filter==='review'&&['waiting_review','waiting_user'].includes(s)||filter==='errors'&&['failed','interrupted'].includes(s)}
function stepFilterData(){
  const w=currentWorkflow(),scope=app.run?'run:'+app.run.id:'workflow:'+w.id;
  if(app.stepScope!==scope){app.stepScope=scope;app.stepFilter='all'}
  const counts=Object.fromEntries(stepFilters.map(f=>[f.id,w.nodes.filter(n=>n.kind!=='Comment'&&stepMatchesFilter(n,f.id)).length]));
  const visible=w.nodes.filter(n=>n.kind!=='Comment'&&stepMatchesFilter(n,app.stepFilter)),selectedFilter=stepFilters.find(f=>f.id===app.stepFilter)||stepFilters[0];
  return {w,counts,visible,selectedFilter};
}
function stepFilterToolbar({w,counts,visible},view){
  const graph=view==='graph',target=graph?'nodes':'step-results',unit=graph?'nodi':'step';
  return `<div class="${graph?'graph-filter-toolbar':'list-filter-toolbar'}"><div class="list-filters" role="group" aria-label="Filtra gli step">${stepFilters.map(f=>`<button type="button" class="list-filter filter-${f.id} ${app.stepFilter===f.id?'active':''}" data-action="filter-steps" data-filter="${f.id}" data-tip="${esc(f.tip)}" aria-pressed="${app.stepFilter===f.id}" aria-controls="${target}" aria-label="${esc(f.label)}: ${counts[f.id]} ${unit}">${icon(f.icon)}<span class="filter-label">${esc(f.label)}</span><span class="filter-label-short" aria-hidden="true">${esc(f.short)}</span><span class="filter-count">${counts[f.id]}</span></button>`).join('')}</div><span class="list-result-count" role="status" aria-live="polite">${visible.length} di ${w.nodes.filter(n=>n.kind!=='Comment').length} ${unit}${graph&&app.stepFilter!=='all'?' evidenziati':''}</span></div>`;
}
function renderList(){
  const data=stepFilterData(),{w,visible,selectedFilter}=data,phases=[...new Set(visible.map(n=>n.phase||'Workflow'))];
  const focusedFilter=document.activeElement?.closest('#execution-list')&&document.activeElement.matches('[data-action="filter-steps"]')?document.activeElement.dataset.filter:null;
  const filters=stepFilterToolbar(data,'list');
  const empty=`<div class="list-empty">${icon(selectedFilter.icon)}<p>${esc(selectedFilter.empty)}</p>${app.stepFilter!=='all'?`<button type="button" data-action="filter-steps" data-filter="all">${icon('list')}Mostra tutti</button>`:''}</div>`;
  const rows=phases.map(p=>`<h2 class="phase-title">${esc(p)}</h2>${visible.filter(n=>(n.phase||'Workflow')===p).map(n=>{
    const s=state(n),agent=associatedAgent(n),running=stepMatchesFilter(n,'running'),pending=stepMatchesFilter(n,'review'),statusIcon=running?'loader-circle':pending?'circle-help':s==='completed'?'check':['failed','interrupted'].includes(s)?'x':null;
    const agentBadge=agent?`<span class="list-agent-badge" data-agent="${esc(agent.id)}" data-tip="${esc(agent.title)} · ${esc(nodeLabel(agent))}">${nodeIcon(agent)}<span>${esc(nodeLabel(agent))}</span></span>`:'';
    return `<button class="step-row ${agent?'has-agent':''} ${pending?'pending':''} ${running?'running':''}" data-action="select-node" data-id="${esc(n.id)}" data-kind="${esc(n.kind)}" data-status="${esc(s)}">${nodeIcon(n)}<span class="step-copy"><b>${esc(n.title)}</b><small>${esc([nodeService(n)?nodeLabel(n):'',n.command||n.operation||n.kind].filter(Boolean).join(' · '))}</small></span>${agentBadge}${aiDecisionBadge(n.id)}<span class="step-state">${statusIcon?icon(statusIcon):''}<span>${esc(states[s]||s)}</span></span></button>${n.kind==='Workflow'&&app.run?childCalls(n.id):''}`;
  }).join('')}`).join('');
  $('#execution-list').innerHTML=(app.run?.error?`<p class="error" role="alert">${esc(app.run.error)}</p>`:'')+(!app.run?'<p class="note">Avvia il workflow per osservare agenti, sessioni e avanzamento reali.</p>':'')+filters+`<div id="step-results">${visible.length?rows:empty}</div>`;
  if(focusedFilter)$('#execution-list .list-filter[data-filter="'+focusedFilter+'"]').focus({preventScroll:true});
}
function renderHistory(){const workflow=currentWorkflow(),runs=app.runs.filter(r=>r.workflow_id===workflow.id);$('#run-history').innerHTML='<h2 class="phase-title">Esecuzioni persistenti</h2>'+`<p class="small" data-no-i18n>${esc(workflow.name)}</p>`+(!runs.length?'<p class="note">Nessuna esecuzione per questo workflow.</p>':'')+runs.map(r=>`<button class="history-row" data-action="load-run" data-id="${r.id}">${icon(r.parent?'git-branch':'workflow')}<span><span data-no-i18n>${esc(r.name)}</span>${r.parent?`<small>Workflow principale · <span data-no-i18n>${esc(r.parent.run.slice(0,8))}</span></small>`:''}<small class="code"> · ${r.id.slice(0,8)} · ${new Date(r.created*1000).toLocaleString(nodoLocale())}</small></span><span class="status-label">${esc(states[r.status]||r.status)}</span></button>`).join('')}
async function showExecutions(){
 const owner=app.user?.id;
 modal('Registro esecuzioni','<p id="executions-loading" role="status">Caricamento esecuzioni…</p>');
 try{
  const runs=await api('/runs');
  if(app.user?.id!==owner||$('#app').hidden||!$('#dialog').open||!$('#executions-loading'))return;
  const ordered=[...runs].sort((a,b)=>b.created-a.created);
  const statusOptions=[...new Set(ordered.map(r=>r.status))];
  modal('Registro esecuzioni',`<div class="form-grid"><label>Cerca workflow<input type="search" id="executions-search" placeholder="Cerca per nome…" autocomplete="off"></label><label>Stato dell’esecuzione<jui-select id="executions-status"><jui-option value="">Tutti gli stati</jui-option>${statusOptions.map(status=>`<jui-option value="${esc(status)}">${esc(uiText(states[status]||status))}</jui-option>`).join('')}</jui-select></label></div><p class="small">Dal più recente al meno recente</p><p class="small" id="executions-count" role="status" aria-live="polite"></p><div id="executions-rows"></div>`,'<button type="button" data-action="close-dialog">Chiudi</button>',true);
  const search=$('#executions-search'),status=$('#executions-status');
  const refresh=()=>{
   const term=search.value.trim().toLocaleLowerCase(nodoLocale());
   const filtered=ordered.filter(r=>(r.name||'').toLocaleLowerCase(nodoLocale()).includes(term)&&(!status.value||r.status===status.value));
   $('#executions-count').textContent=filtered.length+' / '+ordered.length;
   $('#executions-rows').innerHTML=filtered.length?filtered.map(r=>`<button type="button" class="history-row" data-execution-id="${esc(r.id)}">${icon(r.parent?'git-branch':'workflow')}<span><span data-no-i18n>${esc(r.name)}</span><small class="code" data-no-i18n>${esc(r.id)} · ${esc(new Date(r.created*1000).toLocaleString(nodoLocale()))}</small></span><span class="status-label">${esc(uiText(states[r.status]||r.status))}</span></button>`).join(''):`<p class="note">${uiText(ordered.length?'Nessuna esecuzione corrisponde ai filtri.':'Nessuna esecuzione disponibile.')}</p>`;
   $$('[data-execution-id]').forEach(button=>button.onclick=async()=>{
    const buttons=$$('[data-execution-id]');buttons.forEach(b=>b.disabled=true);
    try{if(app.dirty)await saveWorkflow();await loadRun(button.dataset.executionId);closeModal()}catch(error){toast(error.message)}finally{buttons.forEach(b=>b.disabled=false)}
   });
  };
  search.oninput=refresh;status.onchange=refresh;refresh();search.focus();
 }catch(error){if($('#executions-loading'))$('#executions-loading').textContent=uiText('Impossibile caricare le esecuzioni.');toast(error.message)}
}
$('#executions').onclick=showExecutions;
$('#latest-run').onclick=async()=>{
 if(openingLatestRun)return;
 const workflow=currentWorkflow().id,owner=app.user?.id;openingLatestRun=true;updateViewContext();
 try{
  const latest=await refreshLatestRun(workflow);
  if(owner!==app.user?.id||currentWorkflow().id!==workflow)return;
  if(!latest)return toast('Nessuna esecuzione per questo workflow.');
  if(app.dirty)await saveWorkflow();
  if(owner!==app.user?.id||currentWorkflow().id!==workflow)return;
  if(await loadRun(latest.id,workflow)){app.dirty=false;$('#dirty').hidden=true}
 }catch(e){toast(e.message)}finally{openingLatestRun=false;updateViewContext()}
};
async function refreshHistory(){const id=currentWorkflow().id;const runs=await api('/runs?workflow_id='+encodeURIComponent(id));if(currentWorkflow().id!==id)return;app.runs=runs;renderHistory()}
function input(label,name,value,type='text',placeholder=''){return `<label>${label}<input name="${name}" type="${type}" value="${esc(value)}"${placeholder?` placeholder="${esc(placeholder)}"`:''}></label>`}
function select(label,name,value,options){if(value!==''&&value!=null&&!options.some(o=>(Array.isArray(o)?o[0]:o)===value))options=[[value,String(value)+' · '+uiText('Non disponibile')],...options];return `<label>${label}<jui-select name="${name}">${options.map(o=>{const v=Array.isArray(o)?o[0]:o,t=Array.isArray(o)?o[1]:o;return `<jui-option ${['agent','target','base_run','workflow_id','mcp_id'].includes(name)&&v?'data-no-i18n':''} value="${esc(v)}" ${v===value?'selected':''}>${esc(t)}</jui-option>`}).join('')}</jui-select></label>`}
function sourceModeFields(n){return `<div id="source-mode-fields" ${n.source_type==='folder'?'':'hidden'}>${select('Gestione sorgente','source_mode',n.source_mode||'copy',[['direct','Modifica sorgente permanentemente'],['copy','Crea una copia della sorgente']])}<p class="note">${uiText("La modalità permanente salva le modifiche direttamente in /sources. La copia usa un workspace separato; limite per file:")} ${app.settings.source_copy_limit_mib??10} MiB.</p></div>`}
async function confirmSourceCopy(n,onConfirmedLarge=()=>{}){
 const d=await api('/storage/source-size?path='+encodeURIComponent(n.location||'.'));
 if(d.copy_error)throw Error(d.copy_error);
 if(d.bytes<=d.threshold_bytes)return true;
 const accepted=await appConfirm({title:uiText('Sorgente superiore alla soglia di copia'),message:uiText('La sorgente contiene')+' '+(d.bytes/1048576).toFixed(2)+' MiB. '+uiText('Soglia configurata:')+' '+(d.threshold_bytes/1048576)+' MiB. '+uiText('Per contenuti voluminosi è consigliabile lavorare direttamente su /sources. Ogni esecuzione creerà una nuova copia.'),accept:uiText('Crea comunque una copia')});
 if(accepted)onConfirmedLarge();return accepted;
}
function sourcePathField(n){return `<div class="source-path-field"><label for="source-path">Percorso o URL</label><div class="source-path-picker"><div class="source-path-input"><input id="source-path" name="location" value="${esc(n.location||'')}" autocomplete="off"><button type="button" class="icon-button" data-source-folder-toggle aria-label="${esc(uiText('Mostra cartelle di /sources'))}" hidden><svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg></button></div><div id="source-folder-options" class="source-folder-options" role="listbox" aria-label="${esc(uiText('Cartelle di /sources'))}" hidden></div></div></div><p id="source-folder-hint" class="note" hidden></p>`}
function bindSourceFolderPicker(form){
 if(form.sourcePickerBound)return;form.sourcePickerBound=true;
 const field=form.elements.location,list=$('#source-folder-options',form),toggle=$('[data-source-folder-toggle]',form),picker=field.closest('.source-path-picker');let active=-1;
 const options=()=>$$('[role=option]',list);
 const close=()=>{list.hidden=true;field.setAttribute('aria-expanded','false');field.removeAttribute('aria-activedescendant');active=-1;options().forEach(o=>o.setAttribute('aria-selected','false'))};
 const open=()=>{if(toggle.hidden||field.disabled||app.run)return;list.hidden=false;field.setAttribute('aria-expanded','true')};
 form.closeSourceFolderPicker=close;
 field.addEventListener('focus',open);field.addEventListener('click',open);
 toggle.addEventListener('click',()=>{const wasOpen=!list.hidden;field.focus();if(wasOpen)close();else open()});
 const choose=option=>{field.value=option.dataset.folder;field.dispatchEvent(new Event('input',{bubbles:true}));field.dispatchEvent(new Event('change',{bubbles:true}));field.focus();close()};
 list.addEventListener('pointerdown',e=>e.preventDefault());list.addEventListener('click',e=>{const option=e.target.closest('[role=option]');if(option)choose(option)});
 field.addEventListener('keydown',e=>{const rows=options();if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close()}else if(['ArrowDown','ArrowUp'].includes(e.key)){e.preventDefault();open();if(!rows.length)return;active=active<0?(e.key==='ArrowDown'?0:rows.length-1):(active+(e.key==='ArrowDown'?1:-1)+rows.length)%rows.length;rows.forEach((o,i)=>o.setAttribute('aria-selected',String(i===active)));field.setAttribute('aria-activedescendant',rows[active].id);rows[active].scrollIntoView({block:'nearest'})}else if(e.key==='Enter'&&!list.hidden&&active>=0){e.preventDefault();choose(rows[active])}else if(e.key==='Tab')close()});
 picker.addEventListener('focusout',e=>{if(!picker.contains(e.relatedTarget))close()});
}
async function loadSourceFolderOptions(form){
 const field=form?.elements.location,list=form?.querySelector('#source-folder-options'),hint=form?.querySelector('#source-folder-hint');
 if(!field||!list||!hint)return;
 bindSourceFolderPicker(form);form.closeSourceFolderPicker();
 const toggle=$('[data-source-folder-toggle]',form),version=form.sourceFoldersVersion=(form.sourceFoldersVersion||0)+1;
 const enabled=!app.run&&!app.editBase&&form.elements.source_type.value==='folder';
 hint.hidden=!enabled;toggle.hidden=!enabled;list.replaceChildren();
 for(const attr of ['role','aria-controls','aria-expanded','aria-autocomplete'])field.removeAttribute(attr);
 if(!enabled)return;
 field.setAttribute('role','combobox');field.setAttribute('aria-controls',list.id);field.setAttribute('aria-expanded','false');field.setAttribute('aria-autocomplete','none');
 hint.textContent=uiText('Caricamento cartelle di /sources…');
 try{
  const result=await api('/storage?root=sources&path=');
  if(!form.isConnected||form.sourceFoldersVersion!==version||form.elements.source_type.value!=='folder')return;
  const folders=result.entries.filter(entry=>entry.directory).sort((a,b)=>a.name.localeCompare(b.name));
  list.innerHTML=folders.map((entry,i)=>`<div id="source-folder-option-${i}" role="option" aria-selected="false" data-no-i18n data-folder="${esc(entry.name)}">${icon('folder-git-2')}<span>${esc(entry.name)}</span></div>`).join('')||`<p class="note">${uiText('Nessuna cartella disponibile. Puoi scrivere un percorso.')}</p>`;
  hint.textContent=uiText('Scegli una cartella suggerita oppure scrivi un percorso, anche di una sottocartella.');
 }catch{
  if(form.isConnected&&form.sourceFoldersVersion===version)hint.textContent=uiText('Elenco cartelle non disponibile. Scrivi il percorso o usa Sfoglia Storage.');
 }
}
const artifactSearch={scope:null,value:''};
function artifactKey(a){return a.file?.change==='report'?'report:'+a.node:a.file?.location||a.path}
function artifactTime(a){return a.file?.modified||a.created*1000||0}
function artifactDate(a){const time=artifactTime(a);return time?new Date(time).toLocaleString(nodoLocale(),{year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false}):'—'}
function artifactRoot(a){return a.file?.root||(app.run?.context?.['direct-primary']?'/sources':'/data')}
function inspectorArtifacts(arts){
 const scope=app.user?.id+':'+app.run?.id;if(artifactSearch.scope!==scope){artifactSearch.scope=scope;artifactSearch.value=''}
 const latest=new Map();for(const a of arts){const key=artifactKey(a),old=latest.get(key);if(!old||a.version>=old.version)latest.set(key,a)}
 const all=[...latest.values()].sort((a,b)=>artifactTime(b)-artifactTime(a)||b.created-a.created||a.path.localeCompare(b.path)),reports=all.filter(a=>a.file?.change==='report'||(!a.file&&a.path.startsWith('.nodo/reports/'))),files=all.filter(a=>!reports.includes(a));
 const row=a=>`<button type="button" class="artifact-item" data-action="artifact" data-id="${esc(a.id)}" data-artifact-search="${esc(a.path.toLocaleLowerCase())}" data-tip="${esc(a.file?.location||a.path)}">${icon('file-text')}<span class="artifact-file"><span data-no-i18n>${esc(a.path)}</span><small>${uiText('Versione')} ${a.version}</small><time class="artifact-date" data-no-i18n datetime="${artifactTime(a)?new Date(artifactTime(a)).toISOString():''}" title="${esc(uiText(a.file?.change==='legacy'?'Data della versione salvata':'Ultima modifica del file'))}">${esc(artifactDate(a))}</time></span><span class="artifact-origin" data-no-i18n>${artifactRoot(a).slice(1)}</span></button>`;
 return `<div class="inspector-artifacts-heading"><span class="eyebrow" data-artifact-count>ARTEFATTI · ${files.length}</span><span class="small">${uiText('Tutta la run')}</span></div><input type="search" class="artifact-search" data-artifact-query value="${esc(artifactSearch.value)}" placeholder="${esc(uiText('Cerca artefatti…'))}" aria-label="${esc(uiText('Cerca artefatti'))}"><div data-inspector-artifacts>${files.map(row).join('')}<p class="note" data-artifact-empty>${uiText(files.length?'Nessun artefatto corrispondente.':'I file creati o modificati nella run appariranno qui.')}</p>${reports.length?`<details class="artifact-reports"><summary>${uiText('Report')} · ${reports.length}</summary>${reports.map(row).join('')}</details>`:''}</div>`;
}
function bindArtifactSearch(){
 const input=$('[data-artifact-query]');if(!input)return;
 const apply=()=>{const query=input.value.toLocaleLowerCase().trim();artifactSearch.value=input.value;const rows=$$('[data-artifact-search]',$('#inspector'));rows.forEach(row=>row.hidden=!row.dataset.artifactSearch.includes(query));const files=rows.filter(row=>!row.closest('.artifact-reports')),visible=files.filter(row=>!row.hidden).length;const count=$('[data-artifact-count]');if(count)count.textContent=uiText('ARTEFATTI · '+visible);const empty=$('[data-artifact-empty]');if(empty)empty.hidden=visible>0;const reports=$('.artifact-reports');if(reports)reports.hidden=!rows.some(row=>row.closest('.artifact-reports')&&!row.hidden)};
 input.oninput=apply;apply();
}
function renderInspector(){
 const artifactInput=$('[data-artifact-query]'),artifactFocus=artifactInput===document.activeElement,artifactSelection=artifactInput?.selectionStart,artifactScroll=$('[data-inspector-artifacts]')?.scrollTop||0;
 const selected=selectedNodeIds();
 if(!app.run&&selected.length>1){$('#inspector').hidden=false;$('#inspector').innerHTML=`<div class="panel-heading">${icon('workflow')}<b>${selected.length} ${esc(uiText('nodi selezionati'))}</b><div class="spacer"></div><button type="button" class="icon-button" data-action="clear-node-selection" aria-label="${esc(uiText('Deseleziona'))}">${icon('x')}</button></div><p class="note">Trascina uno dei nodi selezionati per spostare il gruppo. Shift + clic aggiunge o rimuove un nodo.</p><button type="button" class="danger" data-action="delete-selected-nodes">${icon('trash-2')}${esc(uiText('Cancella selezione'))}</button>`;return}
 const n=currentWorkflow().nodes.find(n=>n.id===app.selected);$('#inspector').hidden=!n||n.kind==='Comment';if(!n||n.kind==='Comment'){$('#inspector').replaceChildren();return}let fields=input('Nome','title',n.title)+input('Fase','phase',n.phase||'Workflow');const agents=currentWorkflow().nodes.filter(n=>n.kind==='Agent').map(n=>[n.id,n.title]);if(n.kind==='A2A')fields+=select('Agente remoto','a2a_id',n.a2a_id||'',[['','Seleziona connessione'],...a2aConnections.map(c=>[c.id,c.name])])+`<label>Richiesta<textarea name="prompt" rows="6">${esc(n.prompt||'')}</textarea></label><button type="button" data-action="a2a-connections">Connessioni A2A</button><button type="button" data-action="a2a-probe">Verifica Agent Card</button><p class="note">Invia richiesta, input della run e contesto a monte. I chiarimenti riprendono lo stesso task remoto.</p>`;if(n.kind==='Agent')fields+=select('Runtime','runtime',n.runtime||'', [['','Preferenza personale'],['codex','Codex'],['claude','Claude Code'],['pi','Pi Agent']])+input('Modello (vuoto = configurazione personale)','model',n.model||'');if(['Task','Gate'].includes(n.kind)||n.kind==='Review')fields+=select('Agent','agent',n.agent||'', [['','Seleziona Agent'],...agents]);if(n.kind==='Task')fields+=input('Skill Spec Kit (es. implement)','command',n.command||'');if(['Task','Gate','Review'].includes(n.kind))fields+=`<label>Prompt / criteri<textarea name="prompt" rows="7">${esc(n.prompt)}</textarea></label>`;if(n.kind==='Gate')fields+=input('Esiti separati da virgola','outcomes',(n.outcomes||[]).join(', '))+input('Limite rework','max_rework',n.max_rework??2,'number')+select('Richiede test deterministici','require_tests',String(!!n.require_tests),[['false','No'],['true','Sì']]);if(n.kind==='Gate')fields+=select('Verifica chiusura task','require_tasks_closed',String(!!n.require_tasks_closed),[['false','No'],['true','Sì']])+input('Percorso tasks.md (vuoto = feature unica)','location',n.location||'');if(n.kind==='Review')fields+=select('Review','mode',n.mode||'agent_and_human',[['human','Umana'],['agent','Agente in contesto separato'],['agent_and_human','Agente + approvazione umana']])+input('Limite rework','max_rework',n.max_rework??2,'number');if(n.kind==='Source')fields+=select('Tipo Source','source_type',n.source_type||'example',[['example','Repository precaricato'],['folder','Folder in /sources'],['examples','Folder in examples'],['git','Repository Git HTTPS']])+sourcePathField(n)+sourceModeFields(n)+`<button type="button" data-action="browse-source">${icon('folder-git-2')}Sfoglia Storage</button>`;if(n.kind==='Memory')fields+=select('Task sorgente','agent',n.agent||'',[['','Seleziona Task'],...currentWorkflow().nodes.filter(n=>n.kind==='Task').map(n=>[n.id,n.title])])+select('Agent destinatario','target',n.target||'',[['','Seleziona Agent'],...agents]);if(n.kind==='Connection')fields+=select('Provider','provider',n.provider||'local',[['local','Git / CI locali'],['github','GitHub'],['azure','Azure DevOps'],['servicenow','ServiceNow'],['atlassian','Atlassian Teamwork Graph CLI']])+`<div id="connection-fields">${connectionFields(n)}</div>`;if(n.kind==='RAG')fields+=select('Cartella indicizzata','rag_index',n.rag_index||'',[['','Seleziona indice RAG'],...ragCatalog.filter(i=>i.directory&&i.generation).map(i=>[i.id,i.name+' · '+i.root+'/'+i.path])])+select('Modalità di ricerca','rag_mode',n.rag_mode||'',[['','Configurazione indice'],['hybrid','Hybrid'],['semantic','Semantic'],['keyword','Keyword']])+input('Risultati per ricerca','rag_top_k',n.rag_top_k??8,'number')+`<button type="button" data-action="rag-node-manage">${icon('database-rag')}Gestisci indice</button><p class="note">Collega questo nodo a un Agent con un arco context. Ricerca, stato, elenco documenti e lettura chunk si aggiungono ai tool nativi. L’indice descrive la sorgente, non le modifiche successive nel workspace.</p>`;if(n.kind==='MCP')fields+=select('Server MCP personale','mcp_id',n.mcp_id||'',[['','Seleziona server MCP'],...Object.entries(app.settings.mcp_servers||{}).map(([id,c])=>[id,c.name||id])])+`<label>Tool consentiti (vuoto = tutti)<textarea name="mcp_tools" rows="4">${esc((n.mcp_tools||[]).join('\n'))}</textarea></label><div class="actions"><button type="button" data-action="mcp-node-probe">${icon('list-checks')}Scopri e seleziona tool</button><button type="button" data-action="mcp-settings">${icon('settings-2')}Server MCP personali</button></div><p class="note">Collega la porta di uscita a un Agent con un arco context. I tool nativi restano disponibili.</p>`;if(n.kind==='Workflow')fields+=select('Workflow da richiamare','workflow_id',n.workflow_id||'',[['','Seleziona workflow'],...app.workflows.filter(w=>w.id!==currentWorkflow().id).map(w=>[w.id,w.name])])+`<label>Contesto da passare<textarea name="prompt" rows="5">${esc(n.prompt||'')}</textarea></label><p class="note">Sessioni separate. Workspace ereditato dal chiamante; i Source del figlio non vengono ricopiati. Il padre attende il completamento.</p>`;if(n.kind==='Fork')fields+=select('Nodo di ricongiungimento','target',n.target||'',[['','Seleziona Join'],...currentWorkflow().nodes.filter(j=>j.kind==='Join').map(j=>[j.id,j.title])])+`<p class="note">Collega almeno due rami di Task, ciascuno con un Agent distinto. Tutti i rami devono terminare nel Join selezionato. Il piano del Task precedente viene passato automaticamente a ogni agente, senza Memory. Il workspace è condiviso: assegna file diversi ai rami.</p>`;if(n.kind==='Join')fields+=`<p class="note">Attende tutti i rami del Fork e trasferisce i risultati agli agenti dei Task successivi, senza Memory.</p>`;const arts=app.artifacts;const edges=currentWorkflow().edges.filter(e=>e.from===n.id||e.to===n.id);$('#inspector').innerHTML=`<div class="panel-heading">${nodeIcon(n)}<b>${esc(n.kind)}</b>${nodeService(n)?`<span class="small">${esc(nodeLabel(n))}</span>`:''}<div class="spacer"></div><button class="icon-button" data-action="close-inspector" aria-label="Chiudi dettagli">${icon('x')}</button></div>${app.run?`<p class="note">Run ${esc(app.run.id.slice(0,8))} · ${esc(states[state(n)])}<br>La configurazione del run è congelata.</p>`:''}<form id="node-form" data-workflow-id="${esc(currentWorkflow().id)}" data-node-id="${esc(n.id)}" class="inspector-form"><fieldset ${app.run?'disabled':''} style="border:0;margin:0;padding:0;display:grid;gap:15px">${fields}</fieldset><div class="actions">${!app.run?`<button type="button" class="icon-button" data-action="save-inspector" data-tip="Salva" aria-label="Salva">${icon('save')}</button><button type="button" data-action="set-entry" data-id="${n.id}" data-tip="Usa come ingresso" aria-label="Usa come ingresso">${icon('log-in')}</button><button type="button" class="danger" data-action="delete-node" data-id="${n.id}" aria-label="Elimina nodo">${icon('trash-2')}</button>`:''}${n.kind==='Review'||app.decisions.some(d=>d.node===n.id)?`<button type="button" data-action="review-history" data-id="${n.id}" data-tip="Storico review" aria-label="Storico review">${icon('history')}</button>`:''}</div></form>${n.kind==='Workflow'&&app.run?childCalls(n.id):''}${app.run?.pending?.node===n.id?'<div class="divider"></div><button data-action="pending" class="review-banner">Apri review o domanda</button>':''}<div class="divider"></div><span class="eyebrow">COLLEGAMENTI</span>${edges.map((e,i)=>`<div class="note code inspector-connection-row"><span>${esc(e.from)} → ${esc(e.to)} · ${esc(e.kind)} ${esc(e.outcome)}</span>${!app.run?`<button type="button" class="icon-button inspector-connection-delete" data-action="delete-edge" data-index="${currentWorkflow().edges.indexOf(e)}" aria-label="Rimuovi arco">${icon('x')}</button>`:''}</div>`).join('')}${!edges.length?'<p class="note">Usa le porte del nodo per collegarlo.</p>':''}<div class="divider"></div>${inspectorArtifacts(arts)}`;bindArtifactSearch();const artifactList=$('[data-inspector-artifacts]');if(artifactList)artifactList.scrollTop=artifactScroll;if(artifactFocus){const query=$('[data-artifact-query]');query?.focus({preventScroll:true});query?.setSelectionRange(artifactSelection,artifactSelection)}if(n.kind==='Source'){loadSourceFolderOptions($('#node-form'));if(app.editBase){for(const name of ['source_type','source_mode','location'])$('#node-form').elements[name].disabled=true;$('[data-source-folder-toggle]')?.setAttribute('hidden','');const browse=$('[data-action="browse-source"]');if(browse)browse.disabled=true;$('#source-folder-hint').hidden=false;$('#source-folder-hint').textContent=uiText('Le Source restano quelle della run originale. La ripartenza usa una copia dei file correnti.')}}}
function childCalls(node){const calls=app.children.filter(c=>c.node===node);if(!calls.length)return '';return `<details class="child-calls" ${app.run?.current===node?'open':''}><summary>${icon('workflow')}<span>Workflow collegati</span><b>${calls.length}</b></summary>${calls.map(c=>`<button type="button" class="child-run-link" data-action="load-run" data-id="${esc(c.id)}" data-status="${esc(c.status)}">${icon(['waiting_review','waiting_user'].includes(c.status)?'circle-help':'workflow')}<span><b data-no-i18n>${esc(c.name)}</b><small><span data-no-i18n>${esc(c.id.slice(0,8))}</span> · ${esc(states[c.status]||c.status)}</small></span>${icon('log-in')}</button>`).join('')}</details>`}
function showRunFamily(){if(!app.run)return;modal('Workflow collegati',`<p class="note">Stesso workspace, sessioni native separate. Apri un’esecuzione per vedere grafo, review e artefatti.</p>${app.run.parent?`<button class="child-run-link" data-action="load-run" data-id="${esc(app.run.parent.run)}">${icon('arrow-left')}<span>Workflow principale<small data-no-i18n>${esc(app.run.parent.run.slice(0,8))}</small></span></button>`:''}${[...new Set(app.children.map(c=>c.node))].map(id=>`<h3 data-no-i18n>${esc(app.run.workflow.nodes.find(n=>n.id===id)?.title||id)}</h3>${childCalls(id)}`).join('')}`,'<button data-action="close-dialog">Chiudi</button>')}
const activityLog={owner:null,generation:0,workflow:null,selected:null,page:0,timer:null,runs:new Map()};
function resetActivityLogs(){activityLog.owner=app.user?.id;activityLog.generation++;activityLog.workflow=null;activityLog.selected=null;activityLog.page=0;clearTimeout(activityLog.timer);activityLog.runs.clear();}
function logRun(meta){let entry=activityLog.runs.get(meta.id);if(!entry){entry={meta,events:new Map(),scroll:0,loading:false,loaded:false,error:''};activityLog.runs.set(meta.id,entry)}else entry.meta={...entry.meta,...meta};return entry}
function activityData(event){try{return JSON.parse(event.text)}catch{return null}}
function activityDetail(value){
 const text=String(value??'');
 if(app.settings?.activity_log_verbosity==='full')return text;
 const chars=Array.from(text.replace(/\s+/g,' ').trim());
 return chars.slice(0,30).join('')+(chars.length>30?'…':'');
}
function activityMessage(event){
 const data=activityData(event);
 if(event.kind==='compaction'&&data){const phase={start:'Compattazione avviata',end:'Compattazione completata',error:'Compattazione fallita'},mode={manual:'Manuale',auto:'Automatica',automatic:'Automatica',threshold:'Soglia automatica',overflow:'Contesto esaurito'};return `${uiText(phase[data.phase]||'Compattazione')} · ${data.runtime||''} · ${uiText(mode[data.mode]||data.mode||'Automatica')}${data.binding?' · '+data.binding:''}${data.tokens_before!=null?'\n'+uiText('Token prima')+': '+data.tokens_before:''}${data.tokens_after_estimate!=null?'\n'+uiText('Token dopo (stima)')+': '+data.tokens_after_estimate:''}${data.message?' · '+activityDetail(data.message):''}`}
 if(event.kind==='activity'&&data){const phase={start:'Avvio strumento',end:'Strumento completato',error:'Errore strumento',agent_start:'Agente avviato'};return `${uiText(phase[data.phase]||'Attività')} · ${data.runtime||''}${data.tool?' · '+data.tool:''}${data.detail?' · '+activityDetail(data.detail):''}${data.exit_code!=null?' · Exit: '+data.exit_code:''}`}
 if(event.kind==='output'){const match=event.text.match(/<nodo_result>([\s\S]*?)<\/nodo_result>/);if(match){try{const r=JSON.parse(match[1]);return [event.text.slice(0,match.index).trim(),r.summary,r.details].filter(Boolean).map(activityDetail).join(' · ')}catch{}}}
 if(event.kind==='usage'&&data)return `${data.runtime||''} · ${data.model||''}\n${uiText('Token in ingresso')}: ${data.input??'—'} · ${uiText('Token in uscita')}: ${data.output??'—'}${data.usd!=null?' · USD '+Number(data.usd).toFixed(4):''}`;
 if(event.kind==='agent_turn'&&data)return `${data.runtime||''} · ${uiText('Turno agente terminato')}`;
 if(event.kind==='state')return uiText(states[event.text]||event.text);
 return activityDetail(event.text);
}
function activityCurrent(entry){
 const tools=new Map(),compacting=new Map();let agent='';
 for(const event of [...entry.events.values()].sort((a,b)=>a.seq-b.seq)){
  const data=activityData(event);
  if(event.kind==='compaction'&&data){const key=data.operation||event.node+':'+data.runtime;if(data.phase==='start')compacting.set(key,data);else compacting.delete(key)}
  if(event.kind==='activity'&&data){if(data.phase==='agent_start')agent=data.runtime||'';const key=event.node+':'+data.id;if(data.phase==='start')tools.set(key,data);if(['end','error'].includes(data.phase))tools.delete(key)}
  if(event.kind==='agent_turn'){agent='';for(const [key,value] of compacting)if(!value.operation&&key.startsWith(event.node+':'))compacting.delete(key);for(const key of tools.keys())if(key.startsWith(event.node+':'))tools.delete(key)}
  if(event.kind==='error'||event.kind==='state'&&!['running','starting','waiting_child'].includes(event.text)){tools.clear();compacting.clear();agent=''}
 }
 if(compacting.size)return uiText('Compattazione in corso…')+' · '+[...compacting.values()].map(c=>c.runtime).join(', ');
 if(!['running','starting','waiting_child'].includes(entry.meta.status))return '';
 const current=[...tools.values()];return current.length?current.map(t=>`${t.runtime} · ${t.tool}${t.detail?' · '+activityDetail(t.detail):''}`).join('\n'):uiText(agent?'Agente in elaborazione':'Passaggio in esecuzione');
}
function renderActivityLog(){
 if(!app.user||!app.wf||!$('#log-lines'))return;if(activityLog.owner!==app.user.id)resetActivityLogs();
 const workflow=currentWorkflow().id,metas=app.runs.filter(r=>r.workflow_id===workflow);
 if(activityLog.workflow!==workflow){activityLog.workflow=workflow;activityLog.page=0;for(const [id,entry] of activityLog.runs)if(entry.meta.workflow_id!==workflow)activityLog.runs.delete(id);if(!$('#logs').hidden)queueMicrotask(()=>loadActivityRuns().catch(error=>toast(error.message)))}
 if(app.run)metas.push({id:app.run.id,name:app.run.workflow.name,workflow_id:workflow,status:app.run.status,created:app.run.created,current:app.run.current});
 for(const child of app.children||[])metas.push({...child,workflow_id:workflow});
 metas.forEach(logRun);
 const history=[...activityLog.runs.values()].filter(e=>e.meta.workflow_id===workflow).sort((a,b)=>b.meta.created-a.meta.created||b.meta.id.localeCompare(a.meta.id));
 const entries=history.slice(0,100);if(app.run&&history.some(e=>e.meta.id===app.run.id)&&!entries.some(e=>e.meta.id===app.run.id))entries.push(history.find(e=>e.meta.id===app.run.id));
 const retained=new Set(entries.map(e=>e.meta.id));for(const [id,entry] of activityLog.runs)if(entry.meta.workflow_id===workflow&&!retained.has(id))activityLog.runs.delete(id);
 const panel=$('#log-lines'),focusedTab=panel.contains(document.activeElement)?document.activeElement.closest('[data-log-run]')?.dataset.logRun:null,tabScroll=$('.activity-tabs',panel)?.scrollLeft||0;
 if(!entries.some(e=>e.meta.id===activityLog.selected))activityLog.selected=entries.find(e=>e.meta.id===app.run?.id)?.meta.id||entries[0]?.meta.id||null;
 const active=entries.find(e=>e.meta.id===activityLog.selected);
 const pageSize=Math.max(1,Math.min(100,Number(app.settings?.activity_log_page_size)||10));
 activityLog.page=Math.floor(Math.max(0,entries.indexOf(active))/pageSize);
 const visible=entries.slice(activityLog.page*pageSize,activityLog.page*pageSize+pageSize),previous=$('#activity-panel',panel),oldHeight=previous?.scrollHeight||0,sameRun=previous?.dataset.logOwner===active?.meta.id;
 panel.innerHTML=entries.length?`<div class="activity-navigation"><button type="button" data-log-page="-1" ${activityLog.page===0?'disabled':''}>${uiText('Più recenti')}</button><span data-no-i18n>${activityLog.page*pageSize+1}–${Math.min(entries.length,activityLog.page*pageSize+pageSize)} / ${entries.length} · ${esc(uiText('Ultime 100 esecuzioni'))}</span><button type="button" data-log-page="1" ${(activityLog.page+1)*pageSize>=entries.length?'disabled':''}>${uiText('Meno recenti')}</button></div><div class="activity-tabs tabs" role="tablist" aria-label="${esc(uiText('Esecuzioni del workflow'))}">${visible.map(entry=>{
  const r=entry.meta,selected=entry===active;
  return `<button type="button" role="tab" id="activity-tab-${esc(r.id)}" data-log-run="${esc(r.id)}" aria-controls="activity-panel" aria-selected="${selected}" tabindex="${selected?0:-1}" class="${selected?'active':''}"><span class="activity-run-title" data-no-i18n>${esc(r.name)} · ${esc(r.id.slice(0,8))}</span><small data-no-i18n>${esc(new Date(r.created*1000).toLocaleString(nodoLocale()))} · ${esc(uiText(states[r.status]||r.status))}${app.run?.id===r.id?' · '+uiText('Live'):''}</small></button>`;
 }).join('')}</div><div id="activity-panel" data-log-owner="${esc(active.meta.id)}" class="activity-run-content" role="tabpanel" tabindex="0" aria-labelledby="activity-tab-${esc(active.meta.id)}">${(()=>{
  const entry=active,r=entry.meta,events=[...entry.events.values()].sort((a,b)=>b.seq-a.seq),current=activityCurrent(entry);
  return `${current?`<p class="activity-current" data-no-i18n>${esc(uiText('In corso'))}: ${esc(current.replace(/\s+/g,' ').trim())}</p>`:''}${entry.error?`<p class="error">${esc(entry.error)}</p><button type="button" data-log-retry="${esc(r.id)}">${uiText('Riprova')}</button>`:''}${entry.loading?`<p class="note">${uiText('Caricamento…')}</p>`:''}${events.map(event=>{const kind={activity:'Strumenti',output:'Risposta agente',state:'Stato',error:'Errore',usage:'Consumi',agent_turn:'Turno agente',compaction:'Compattazione',restart:'Riavvio server'}[event.kind]||event.kind;const title=app.run?.id===r.id?app.run.workflow.nodes.find(n=>n.id===event.node)?.title:null;return `<article class="activity-event" data-log-seq="${event.seq}"><div class="activity-event-meta"><time data-no-i18n>${esc(new Date(event.created*1000).toLocaleTimeString(nodoLocale()))}</time> · <span data-no-i18n>${esc(title||event.node||'run')}</span> · <span>${esc(uiText(kind))}</span></div><pre data-no-i18n>${esc(activityMessage(event).replace(/\s+/g,' ').trim())}</pre></article>`}).join('')}${!events.length&&!entry.loading&&!entry.error?`<p class="note">${uiText('Nessun messaggio disponibile.')}</p>`:''}`;
 })()}</div>`:`<p class="note">${uiText('Nessuna esecuzione per questo workflow.')}</p>`;
 const content=$('#activity-panel',panel);if(content){content.scrollTop=active.scroll+(sameRun&&active.scroll>0?Math.max(0,content.scrollHeight-oldHeight):0);content.onscroll=()=>{active.scroll=content.scrollTop}}
 const tabs=$$('[data-log-run]',panel);
 const tablist=$('.activity-tabs',panel);if(tablist)tablist.scrollLeft=tabScroll;
 if(focusedTab)tabs.find(tab=>tab.dataset.logRun===focusedTab)?.focus({preventScroll:true});
 const activate=tab=>{const entry=activityLog.runs.get(tab.dataset.logRun);activityLog.selected=entry.meta.id;renderActivityLog();$('#activity-tab-'+entry.meta.id)?.focus({preventScroll:true});if(!entry.loading)loadActivityEvents(entry.meta.id)};
 tabs.forEach((tab,i)=>{tab.onclick=()=>activate(tab);tab.onkeydown=e=>{let next;if(e.key==='ArrowRight')next=(i+1)%tabs.length;else if(e.key==='ArrowLeft')next=(i+tabs.length-1)%tabs.length;else if(e.key==='Home')next=0;else if(e.key==='End')next=tabs.length-1;else return;e.preventDefault();activate(tabs[next])}});
 $$('[data-log-page]',panel).forEach(button=>button.onclick=()=>{const index=(activityLog.page+Number(button.dataset.logPage))*pageSize;activityLog.selected=entries[index].meta.id;renderActivityLog();loadActivityEvents(activityLog.selected)});
 $$('[data-log-retry]',panel).forEach(button=>button.onclick=()=>loadActivityEvents(button.dataset.logRetry));
}

async function loadActivityEvents(id){
 const entry=activityLog.runs.get(id);if(!entry||entry.loading)return;const generation=activityLog.generation;entry.loading=true;entry.error='';renderActivityLog();
 try{let cursor=entry.loaded?Math.max(0,...entry.events.keys()):0;for(;;){const batch=await api('/runs/'+encodeURIComponent(id)+'/events?after='+cursor);if(generation!==activityLog.generation)return;for(const event of batch)if(event.run===id)entry.events.set(event.seq,event);if(batch.length<500)break;cursor=batch[batch.length-1].seq}entry.loaded=true}catch(error){if(generation===activityLog.generation)entry.error=error.message}finally{if(generation===activityLog.generation){entry.loading=false;renderActivityLog();syncNodeToolLabels()}}
}
async function loadActivityRuns(){
 if(!app.wf)return;const workflow=currentWorkflow().id,owner=app.user?.id,generation=activityLog.generation;
 const runs=await api('/runs?workflow_id='+encodeURIComponent(workflow));if(owner!==app.user?.id||generation!==activityLog.generation||workflow!==currentWorkflow().id)return;
 app.runs=[...app.runs.filter(r=>r.workflow_id!==workflow),...runs];runs.forEach(logRun);renderActivityLog();for(const entry of activityLog.runs.values())if(entry.meta.workflow_id===workflow&&entry.meta.id===activityLog.selected&&!entry.loaded&&!entry.loading)loadActivityEvents(entry.meta.id);
}
function recordActivityEvent(id,event){if(event.run!==id||app.run?.id!==id)return;const entry=logRun({id,name:app.run.workflow.name,workflow_id:app.run.workflow.id,status:app.run.status,created:app.run.created});entry.events.set(event.seq,event);syncNodeToolLabels();clearTimeout(activityLog.timer);activityLog.timer=setTimeout(renderActivityLog,40)}

async function loadRun(id,expectedWorkflow=null){if(!app.run&&app.dirty)await saveWorkflow();const owner=app.user?.id,d=await api('/runs/'+id);if(expectedWorkflow&&(owner!==app.user?.id||currentWorkflow().id!==expectedWorkflow))return false;leaveDraftContext();app.editBase=null;app.dirty=false;app.run=d.run;app.selectedNodes=[];app.continueNodes=d.continue_nodes||[];app.children=d.children||[];app.reviewRun=d.review_run;workflowOptions(app.run.workflow.id);app.artifacts=d.artifacts;app.decisions=d.decisions;app.selected=null;app.selectedEdge=null;app.view=app.view==='list'?'list':'graph';app.events=[];app.lastEvent=0;logRun({id:app.run.id,name:app.run.workflow.name,workflow_id:app.run.workflow.id,status:app.run.status,created:app.run.created});activityLog.selected=app.run.id;latestRunWorkflow='';render();centerNode(app.run.current);connectStream(id);return true}
function disconnect(){app.stream?.close();app.stream=null;clearTimeout(app.refresh)}
async function refreshRun(){if(!app.run)return;const prev=app.run.current,id=app.run.id;try{const d=await api('/runs/'+id);if(app.run?.id!==id)return;app.run=d.run;app.continueNodes=d.continue_nodes||[];app.children=d.children||[];app.reviewRun=d.review_run;app.artifacts=d.artifacts;app.decisions=d.decisions;render();if(prev!==app.run.current&&app.view==='graph')centerNode(app.run.current);if(app.run.error)$('#footer-info').textContent=app.run.error;if(app.view==='history')await refreshHistory();else app.runs=await api('/runs')}catch(e){toast(e.message)}}
function connectStream(id){disconnect();const stream=new EventSource('/api/runs/'+id+'/stream');app.stream=stream;stream.onopen=()=>{if(app.stream!==stream)return;$('#connection-state').textContent='Live · Connesso';$('#graph').classList.remove('connection-lost');refreshRun();const entry=activityLog.runs.get(id);if(entry)loadActivityEvents(id)};stream.onerror=()=>{if(app.stream!==stream)return;$('#connection-state').textContent='Riconnessione…';$('#graph').classList.add('connection-lost')};stream.addEventListener('refresh',()=>{refreshRun();const entry=activityLog.runs.get(id);if(entry)loadActivityEvents(id)});stream.onmessage=e=>{if(app.stream!==stream||app.run?.id!==id)return;const v=JSON.parse(e.data);if(v.seq<=app.lastEvent)return;app.lastEvent=v.seq;app.events.push(v);if(app.events.length>500)app.events.shift();recordActivityEvent(id,v);if(['state','error','created'].includes(v.kind)){clearTimeout(app.refresh);app.refresh=setTimeout(refreshRun,120)}};}

const serviceInfo={
  codex:{name:'Codex',icon:'brand-openai',description:'Sviluppo con sessioni persistenti'},
  claude:{name:'Claude Code',icon:'brand-claude',description:'Il tuo agente di sviluppo Anthropic'},
  pi:{name:'Pi Agent',icon:'brand-pi',description:'Agente di sviluppo multi-provider'},
  atlassian:{name:'Atlassian Teamwork Graph CLI',icon:'plug',description:'Jira, Confluence, Bitbucket e altri prodotti Atlassian'},
  github:{name:'GitHub',icon:'brand-github',description:'Issue, repository e pull request'},
  azure:{name:'Azure DevOps',icon:'brand-azuredevops',description:'Work item, progetti e pull request'},
  servicenow:{name:'ServiceNow',icon:'brand-servicenow',description:'Incident e aggiornamenti operativi'}
};
function connectionFields(n){
 if(n.provider==='atlassian')return `<input type="hidden" name="operation" value="twg"><label>Argomenti TWG (JSON)<textarea name="command" rows="6" placeholder='["jira","workitem","get","ABC-123"]'>${esc(n.command||'')}</textarea></label><p class="note">Usa i comandi di Jira, Confluence, Bitbucket e degli altri prodotti TWG. Il workflow richiede conferma prima di eseguire il comando.</p><button type="button" data-action="twg-setup">${icon('plug')}Configura accesso Atlassian</button>`;
 return select('Operazione','operation',n.operation==='twg'?'read_ticket':n.operation||'test',['read_ticket','test','commit','read_ci','create_pr','update_ticket'])+input('Ticket ID / file locale / base branch','location',n.location||'');
}
function twgSetup(){
 const uid=Number(app.user.id),osuid=20000+uid,home='/data/users/'+uid;
 const prefix=`docker compose exec --user ${osuid}:${osuid} -e HOME=${home} -e XDG_CONFIG_HOME=${home}/.config -e TWG_CONFIG_DIR=${home}/.config/twg nodo twg`;
 modal('Collega Atlassian',`<p>Completa il login nel terminale del progetto e apri il link Atlassian mostrato dalla CLI.</p><pre class="source-view" data-no-i18n>${esc(prefix+' setup')}</pre><p>Per Bitbucket, completa anche la configurazione dedicata.</p><pre class="source-view" data-no-i18n>${esc(prefix+' setup bitbucket')}</pre><p class="note">Il login è personale e resta salvato dopo i riavvii. Inserisci eventuali token solo nel prompt protetto della CLI.</p><a href="https://developer.atlassian.com/cloud/twg-cli/getting-started/installation/" target="_blank" rel="noopener noreferrer">Documentazione Atlassian</a><p id="twg-status" role="status"></p>`,'<button type="button" data-action="twg-check">Verifica collegamento</button><button type="button" data-action="close-dialog">Chiudi</button>');
}
async function twgCheck(button){
 const output=$('#twg-status');button.disabled=true;output.textContent='Verifica collegamento…';
 try{const s=await api('/connections/atlassian/status');if(output.isConnected)output.textContent=!s.available?'TWG CLI non disponibile. Ricostruisci il container.':s.connected?'Collegamento Atlassian verificato.':'Completa il login e verifica i permessi Atlassian, poi riprova.'}
 catch(e){if(output.isConnected)output.textContent=e.message}finally{if(button.isConnected)button.disabled=false}
}
function serviceHeading(id,configured,isAgent){const info=serviceInfo[id],status=configured?(isAgent?'Credenziale configurata':'Token salvato'):'Da configurare';return `<header class="service-heading"><span class="service-mark service-${id}">${icon(info.icon,info.icon.startsWith('brand-')?'brand-icon':'')}</span><div class="service-title"><h3>${esc(info.name)}</h3><p>${esc(info.description)}</p></div><span class="service-status ${configured?'is-configured':''}" data-tip="${esc(status)}"><i aria-hidden="true"></i>${esc(status)}</span></header>`}
function compactNumber(rt,key,label,value,min,max){return `<label>${label}<input name="compact_${rt}_${key}" type="number" min="${min}" max="${max}" step="1" required value="${value}"></label>`}
function compactionFields(rt,c){
 let fields='',note='';
 if(rt==='claude'){fields=compactNumber(rt,'trigger_percent','Soglia compattazione (%; 0 = gestione nativa)',c.trigger_percent??80,0,100);note='Con 0 decide Claude quando compattare, usando la sua gestione nativa. Con una percentuale da 1 a 100, il riferimento è automaticamente il contesto massimo del modello selezionato. Con Opus 5.5 (1 milione di token), l’80% corrisponde a circa 800.000 token utilizzati. Claude può anticipare la compattazione per mantenere il proprio margine di sicurezza.'}
 if(rt==='codex'){fields=compactNumber(rt,'threshold_tokens','Soglia compattazione (token; 0 = automatica)',c.threshold_tokens??0,0,10000000)+compactNumber(rt,'context_window_tokens','Contesto disponibile (token; 0 = modello)',c.context_window_tokens??0,0,10000000);note='La soglia deve essere inferiore al contesto disponibile. Lascia 0 per usare i valori nativi del modello e del provider.'}
 if(rt==='pi'){fields=compactNumber(rt,'reserve_tokens','Riserva contesto (token)',c.reserve_tokens??16384,1024,1000000)+compactNumber(rt,'keep_recent_tokens','Contesto recente da conservare (token)',c.keep_recent_tokens??20000,1024,1000000);note='Pi compatta quando i token superano il contesto del modello meno la riserva. Le impostazioni native del progetto o del modello possono prevalere su quelle personali.'}
 return `<details class="compaction-settings"><summary>Compattazione automatica</summary><div class="form-grid">${fields}</div><p class="note">${note}</p><p class="note">Le modifiche salvate valgono dal prossimo turno, anche sulle sessioni esistenti.</p></details>`;
}
function compactionSessionPanel(){return `<section class="settings-section"><h3>Compatta sessione</h3><p class="note">Seleziona una sessione di un’esecuzione ferma. La compattazione usa le impostazioni salvate, può consumare token e conserva la stessa sessione. Non riprende il workflow. L’esito compare anche nell’Activity Log.</p><label>Esecuzione<jui-select id="compact-run"><jui-option value="">Caricamento…</jui-option></jui-select></label><label>Sessione<jui-select id="compact-binding" disabled><jui-option value="">Seleziona un’esecuzione</jui-option></jui-select></label><button type="button" data-action="compact-session" disabled>Compatta sessione</button><p id="compact-status" class="note" role="status"></p></section>`}
async function loadCompactRuns(){
 const select=$('#compact-run');if(!select)return;
 try{const runs=await api('/runs');if(!select.isConnected)return;select.innerHTML='<jui-option value="">Seleziona un’esecuzione</jui-option>'+runs.map(r=>`<jui-option data-no-i18n value="${esc(r.id)}">${esc(r.name||r.workflow_id)} · ${esc(r.id.slice(0,8))} · ${esc(uiText(states[r.status]||r.status))}</jui-option>`).join('');if(app.run&&runs.some(r=>r.id===app.run.id)){select.value=app.run.id;await loadCompactBindings()}}
 catch(e){if(select.isConnected)$('#compact-status').textContent=e.message}
}
async function loadCompactBindings(){
 const select=$('#compact-binding'),button=$('[data-action="compact-session"]'),run=$('#compact-run')?.value;if(!select)return;select.disabled=true;button.disabled=true;select.innerHTML='';$('#compact-status').textContent='';if(!run)return;
 try{const d=await api('/runs/'+encodeURIComponent(run));if(!select.isConnected||$('#compact-run').value!==run)return;
 const busy=['running','starting','queued','waiting_child'].includes(d.run.status);const sessions=Object.entries(d.run.sessions||{}).filter(([,s])=>s.native_id);select.innerHTML=sessions.map(([binding,s])=>`<jui-option data-no-i18n value="${esc(binding)}">${esc(binding)} · ${esc(s.runtime)} · ${esc(s.native_id.slice(0,8))}</jui-option>`).join('');select.disabled=busy||!sessions.length;button.disabled=select.disabled;$('#compact-status').textContent=busy?uiText('Metti in pausa il workflow e attendi la fine del turno prima di compattare.'):!sessions.length?uiText('Nessuna sessione salvata in questa esecuzione.'):''}
 catch(e){if(select.isConnected)$('#compact-status').textContent=e.message}
}
async function compactSession(){
 const run=$('#compact-run')?.value,binding=$('#compact-binding')?.value,status=$('#compact-status'),button=$('[data-action="compact-session"]');if(!run||!binding||!status)return;button.disabled=true;$('#compact-run').disabled=true;$('#compact-binding').disabled=true;status.textContent=uiText('Compattazione in corso…');
 try{const response=await api('/runs/'+encodeURIComponent(run)+'/compact',{binding});let cursor=0;
 while(status.isConnected){const batch=await api('/runs/'+encodeURIComponent(run)+'/events?after='+cursor);for(const event of batch){cursor=Math.max(cursor,event.seq);const data=activityData(event);if(event.kind==='compaction'&&data?.operation===response.operation&&['end','error'].includes(data.phase)){status.textContent=data.phase==='end'?uiText('Compattazione completata. Puoi riprendere il workflow.'):uiText('Compattazione fallita:')+' '+data.message;if(activityLog.runs.has(run))await loadActivityEvents(run);return}}
 if(batch.length<500)await new Promise(resolve=>setTimeout(resolve,1000));}
 }catch(e){if(status.isConnected)status.textContent=e.message}finally{if(button.isConnected){button.disabled=false;$('#compact-run').disabled=false;$('#compact-binding').disabled=false}}
}
function credentialField(label,name,tip,preview=''){
 const personal=name.startsWith('key_'),note='La chiave puntinata è solo un’anteprima. Per sostituirla, inserisci una nuova chiave e salva. Lascia il campo vuoto per conservare quella attuale.';
 const field=input(label,name,'','password',preview||'').replace('<input ',personal?`<input aria-describedby="${name}-help" `:'<input ');
 return `<div class="credential-field"><div class="credential-input-row">${field}<button type="button" class="icon-button" data-action="clear-key" data-field="${name}" data-tip="${esc(tip)}" aria-label="${esc(tip)}">${icon('trash-2')}</button></div>${personal?`<p id="${name}-help" class="note credential-help">${note}</p>`:''}</div>`;
}
function closeAccountMenu(){
 const menu=$('#account-menu');if(menu?.matches(':popover-open'))menu.hidePopover();$('#account')?.setAttribute('aria-expanded','false');
}
function toggleAccountMenu(){
 hideTooltip();let menu=$('#account-menu');
 if(!menu){
  menu=document.createElement('div');menu.id='account-menu';menu.className='account-menu';menu.setAttribute('popover','manual');menu.setAttribute('role','menu');menu.setAttribute('aria-labelledby','account');
  menu.innerHTML=`<div class="account-version"><span class="account-version-number" data-no-i18n>rightcommit v${esc(authInfo.version||'')}</span><a class="account-site-link" role="menuitem" href="https://rightcommit.com" target="_blank" rel="noopener noreferrer" aria-label="${esc(uiText('Apri il sito rightcommit.com in una nuova scheda'))}" title="${esc(uiText('Apri il sito rightcommit.com in una nuova scheda'))}">${icon('external-link')}</a></div><hr role="separator"><button type="button" role="menuitem" data-action="settings">${icon('settings-2')}<span>Configurazioni personali</span></button>${app.user?.admin?`<button type="button" role="menuitem" data-action="manage-templates">${icon('workflow')}<span>Gestisci template</span></button>`:''}<hr role="separator"><button type="button" role="menuitem" data-action="logout" class="account-signout">${icon('log-out')}<span data-no-i18n>Sign out</span></button>`;
  document.body.append(menu);
  document.addEventListener('pointerdown',e=>{if(menu.matches(':popover-open')&&!e.composedPath().includes(menu)&&!e.composedPath().includes($('#account')))closeAccountMenu()},true);
  window.addEventListener('resize',closeAccountMenu);
  menu.addEventListener('toggle',()=>$('#account').setAttribute('aria-expanded',String(menu.matches(':popover-open'))));
  menu.addEventListener('keydown',e=>{const items=$$('[role="menuitem"]',menu),index=items.indexOf(document.activeElement);let next;if(e.key==='ArrowDown')next=(index+1)%items.length;else if(e.key==='ArrowUp')next=(index+items.length-1)%items.length;else if(e.key==='Home')next=0;else if(e.key==='End')next=items.length-1;else if(e.key==='Escape'){e.preventDefault();closeAccountMenu();$('#account').focus();return}else if(e.key==='Tab'){closeAccountMenu();return}else return;e.preventDefault();items[next].focus()});
 }
 const templateItem=$('[data-action="manage-templates"]',menu);
 if(!app.user?.admin)templateItem?.remove();
 else if(!templateItem){const item=document.createElement('button');item.type='button';item.setAttribute('role','menuitem');item.dataset.action='manage-templates';item.innerHTML=`${icon('workflow')}<span>${uiText('Gestisci template')}</span>`;$('[data-action="settings"]',menu).after(item)}
 if(menu.matches(':popover-open')){closeAccountMenu();return}
 menu.showPopover();const rect=$('#account').getBoundingClientRect();menu.style.left=Math.max(8,Math.min(rect.right-menu.offsetWidth,innerWidth-menu.offsetWidth-8))+'px';menu.style.top=(rect.bottom+10)+'px';$('#account').setAttribute('aria-expanded','true');$('[data-action="settings"]',menu).focus();
}
function accountInput(label,name,type,form,autocomplete){return `<label>${label}<input name="${name}" type="${type}" form="${form}" autocomplete="${autocomplete}" required ${type==='password'?'minlength="12" maxlength="256"':''}></label>`}
async function changeAccountPassword(){
 const form=$('#password-form');if(!form?.reportValidity())return;const button=$('[data-account-password-submit]'),error=$('#password-error');if(button)button.disabled=true;if(error)error.textContent='';
 try{await api('/auth/password',Object.fromEntries(new FormData(form)));closeModal();showAuth();toast('Password aggiornata. Tutte le sessioni web precedenti sono state chiuse.')}catch(e){if(error?.isConnected)error.textContent=e.message;else toast(e.message)}finally{if(button?.isConnected)button.disabled=false}
}
async function createAccountUser(){
 const form=$('#user-form');if(!app.user?.admin||!form?.reportValidity())return;const button=$('[data-create-user]'),error=$('#user-error');button.disabled=true;error.textContent='';
 try{await api('/users',Object.fromEntries(new FormData(form)));form.reset();toast('Utente creato con configurazioni separate');await loadAdminUsers()}catch(e){if(error.isConnected)error.textContent=e.message}finally{if(button.isConnected)button.disabled=false}
}
async function loadAdminUsers(){
 const list=$('#admin-user-list'),search=$('#admin-user-search');if(!app.user?.admin||!list)return;
 const version=list.requestVersion=(list.requestVersion||0)+1;
 list.setAttribute('aria-busy','true');
 try{const result=await api('/users');if(!list.isConnected||list.requestVersion!==version)return;
  const draw=()=>{const term=search.value.trim().toLocaleLowerCase(nodoLocale()),users=result.users.filter(u=>u.email.toLocaleLowerCase(nodoLocale()).includes(term));
   $('#admin-user-count').textContent=users.length+' / '+result.users.length;
   list.innerHTML=users.map(u=>`<article class="admin-user-row" data-user-id="${u.id}"><div class="admin-user-identity"><b data-no-i18n>${esc(u.email)}</b><span class="admin-user-id"><span>${uiText('ID utente')}</span>: <span data-no-i18n>${esc(u.id)}</span></span><span class="admin-user-status ${u.status==='suspended'?'is-suspended':''}">${uiText(u.status==='suspended'?'Sospeso':'Attivo')}${u.id===app.user.id?' · '+uiText('Account corrente'):''}</span></div>${u.id===app.user.id||u.admin?`<span class="small">${uiText('Amministratore')}</span>`:`<div class="admin-user-actions"><button type="button" data-action="admin-user-${u.status==='suspended'?'activate':'suspend'}" data-id="${u.id}" data-email="${esc(u.email)}" data-status="${u.status}">${uiText(u.status==='suspended'?'Riattiva':'Sospendi')}</button><button type="button" class="icon-button danger" data-action="admin-user-remove" data-id="${u.id}" data-email="${esc(u.email)}" data-status="${u.status}" aria-label="${esc(uiText('Rimuovi utente'))}">${icon('trash-2')}</button></div>`}</article>`).join('')||`<p class="note">${uiText('Nessun utente trovato.')}</p>`;
  };search.oninput=draw;draw();
 }catch(e){if(list.isConnected&&list.requestVersion===version)list.textContent=e.message}finally{if(list.isConnected&&list.requestVersion===version)list.removeAttribute('aria-busy')}
}
async function changeAdminUser(button){
 const action=button.dataset.action,uid=button.dataset.id,email=button.dataset.email;
 const config={
  'admin-user-suspend':{title:'Sospendi utente',message:'L’accesso verrà sospeso, le sessioni chiuse e i workflow in esecuzione interrotti. Potrai riattivare l’utente in qualsiasi momento.',accept:'Sospendi',status:'suspended'},
  'admin-user-activate':{title:'Riattiva utente',message:'L’utente potrà accedere di nuovo con le proprie credenziali. Le vecchie sessioni resteranno revocate.',accept:'Riattiva',status:'active'},
  'admin-user-remove':{title:'Rimuovi utente',message:'L’account verrà rimosso dall’elenco, l’accesso revocato e i workflow in esecuzione interrotti. I dati resteranno conservati e l’indirizzo email resterà riservato. Questa rimozione non può essere annullata dal pannello.',accept:'Rimuovi utente',status:'removed'}
 }[action];if(!config||!app.user?.admin)return;
 if(!await appConfirm({title:uiText(config.title),message:email+'\n\n'+uiText(config.message),accept:uiText(config.accept),danger:action!=='admin-user-activate'}))return;
 const row=button.closest('.admin-user-row');$$('button',row).forEach(b=>b.disabled=true);
 try{await api('/users/'+encodeURIComponent(uid)+(config.status==='removed'?'':'/status'),{status:config.status,expected_status:button.dataset.status,acknowledge:true},config.status==='removed'?'DELETE':'POST');await loadAdminUsers();toast('Utente aggiornato')}
 catch(e){toast(e.message);if(row.isConnected)$$('button',row).forEach(b=>b.disabled=false)}
}
function updateTimeoutHint(){
 const field=$('#settings-form [name="timeout"]'),hint=$('#timeout-duration');if(!field||!hint)return;
 const seconds=Number(field.value);hint.hidden=!Number.isFinite(seconds)||seconds<60;
 hint.textContent=hint.hidden?'':new Intl.NumberFormat(nodoLocale(),{style:'unit',unit:seconds>=3600?'hour':'minute',unitDisplay:'long',maximumFractionDigits:2}).format(seconds/(seconds>=3600?3600:60));
}
window.addEventListener('nodo-language-changed',updateTimeoutHint);
async function settingsDialog(activeTab='general'){
  app.settings=await api('/settings');const s=app.settings;
  let body=`<form id="settings-form"><label class="settings-appearance">Aspetto<jui-select data-appearance-theme aria-label="Aspetto">${[['light','Chiaro'],['dark','Scuro'],['system','Sistema']].map(([value,label])=>`<jui-option value="${value}" ${appearanceTheme===value?'selected':''}>${label}</jui-option>`).join('')}</jui-select></label><div class="tabs settings-tabs" role="tablist" aria-label="Configurazioni personali">${[['general','Generali'],['execution','Esecuzione'],['agents','Agenti'],['compaction','Compattazione'],['connections','Connessioni'],['mcp','MCP'],['sources','Sorgenti /sources'],['activity','Registro attività'],['environment','Environment'],['cli','Check CLI'],...(app.user.admin?[['admin','Admin']]:[])].map(([id,label],i)=>`<button type="button" role="tab" id="settings-tab-${id}" data-settings-tab="${id}" aria-controls="settings-panel-${id}" aria-selected="${i===0}" tabindex="${i===0?0:-1}" class="${i===0?'active':''} ${['connections','activity','admin'].includes(id)?'settings-nav-break':''}">${label}</button>`).join('')}</div><section id="settings-panel-agents" class="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-agents"><p class="note">Seleziona la configurazione Default per la chat del manager e per gli agenti che usano le preferenze personali.</p><p class="note settings-note">Le credenziali sono personali. Lascia un campo vuoto per conservare quella salvata; usa il cestino per rimuoverla.</p><h3 class="settings-group-title">Agenti</h3>`;
  for(const rt of ['codex','claude','pi']){
    const a=s.agents[rt]||{};
    body+=`<section class="settings-section" data-service="${rt}">${serviceHeading(rt,!!a.configured,true)}<label class="settings-default"><span>Default</span><jui-switch exclusive aria-label="Default ${esc(serviceInfo[rt].name)}" name="default_runtime" value="${rt}" ${s.default_runtime===rt?'checked':''} required></jui-switch></label><div class="form-grid">${input('Modello','model_'+rt,a.model||'','text',({codex:'gpt-6.1-sol',claude:'opus'})[rt]||'')}${select('Ragionamento','effort_'+rt,a.effort||'low',['low','medium','high','xhigh'])}${credentialField('API key personale','key_'+rt,'Rimuovi credenziale salvata',a.api_key_preview)}${rt==='pi'?select('Provider Pi','provider_pi',a.provider||'anthropic',[['anthropic','Anthropic'],['openai','OpenAI'],['openrouter','OpenRouter']]):''}</div>${rt==='pi'?`<label class="settings-switch"><span>Abilita codemode</span><jui-switch name="codemode_pi" aria-label="Abilita codemode" ${a.codemode?'checked':''}></jui-switch></label><p class="note">Nei Task il modello può scrivere script JavaScript che chiamano gli altri strumenti, anche in parallelo, e filtrano l’output prima che entri nel contesto. Gli script girano in una sandbox senza rete né file system; ogni chiamata agli strumenti compare nell’Activity Log.</p>`:''}${compactionFields(rt,s.compaction?.[rt]||{})}${app.user.host_import&&rt!=='pi'?`<button type="button" data-action="import-host" data-runtime="${rt}">Importa login locale ${esc(rt)}</button>`:''}</section>`;
  }
  body+=`<div class="settings-execution-limits"><section class="settings-section"><h3>Timeout del turno</h3><p class="note">Tempo massimo per un nodo che usa un agente. Ogni nuovo tentativo ha il proprio limite.</p><label>Timeout turno in secondi<input name="timeout" type="number" min="1" aria-describedby="timeout-duration" step="1" required value="${s.task_timeout_secs}"></label><p id="timeout-duration" class="small" aria-live="polite" hidden></p></section><section class="settings-section"><h3>Workflow paralleli</h3><p class="note">Numero massimo di workflow eseguiti contemporaneamente dal tuo account. Gli altri attendono in coda. Ridurre il limite non interrompe quelli già attivi.</p><label>Workflow paralleli per utente<input name="workflow_parallelism" type="number" min="1" step="1" required value="${s.workflow_parallelism??5}"></label></section></div>${compactionSessionPanel()}</section><section id="settings-panel-mcp" class="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-mcp" hidden>${mcpSettingsContent(s)}<div class="settings-mcp-actions"><button type="button" class="primary" data-action="mcp-edit">${icon('plus')}Aggiungi server MCP</button><button type="button" data-action="mcp-example">${icon('folder-git-2')}Esempio locale</button></div></section><section id="settings-panel-connections" class="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-connections" hidden>`;
  for(const p of ['github','azure','servicenow']){
    const c=s.connections[p]||{};
    body+=`<section class="settings-section" data-service="${p}">${serviceHeading(p,!!c.configured,false)}<div class="form-grid"><div class="service-url">${input('Base URL HTTPS','base_'+p,c.base_url||(p==='github'?'https://api.github.com':''),'url')}</div>${credentialField('Token personale','token_'+p,'Rimuovi token salvato')}${input(p==='github'?'Repository owner/repo':'Repository','repo_'+p,c.repository||'')}${input('Progetto / organizzazione','project_'+p,c.project||'')}</div></section>`;
  }
  body+=`<section class="settings-section" data-service="atlassian"><header class="service-heading"><span class="service-mark">${icon('plug')}</span><div class="service-title"><h3>Atlassian Teamwork Graph CLI</h3><p>Jira, Confluence, Bitbucket e altri prodotti Atlassian</p></div></header><p class="note">Accesso OAuth personale. Bitbucket richiede una configurazione separata.</p><button type="button" data-action="twg-setup">${icon('plug')}Configura accesso Atlassian</button></section>`;
  body+=`<section class="settings-section"><h3>Sorgenti /sources</h3><label>Limite copia sorgenti (MiB)<input name="source_copy_limit_mib" type="number" min="1" max="17592186044415" step="1" required value="${s.source_copy_limit_mib??10}"></label><p class="note">Vale sia come limite per file sia come soglia dell’avviso per cartella, solo per le copie da /sources. La modifica permanente e i repository remoti non hanno questa soglia.</p></section>`;
  body+=`</section><section id="settings-panel-activity" class="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-activity" hidden><section class="settings-section"><h3>Registro attività</h3><label>Esecuzioni per pagina<input name="activity_log_page_size" type="number" min="1" max="100" step="1" required value="${s.activity_log_page_size??10}"></label><p class="note">Da 1 a 100; valore predefinito 10. Il registro mostra le ultime 100 esecuzioni del workflow, oltre all’esecuzione aperta se precedente.</p>${select('Verbosità','activity_log_verbosity',s.activity_log_verbosity||'preview',[['preview','Primi 30 caratteri'],['full','Testo completo']])}<p class="note">L’anteprima mostra i primi 30 caratteri dei dettagli. Il testo completo mostra tutti i dettagli disponibili. La cronologia salvata resta invariata.</p></section></section><p id="settings-error" class="error" role="alert"></p></form><form id="password-form" hidden></form>${app.user.admin?'<form id="user-form" hidden></form>':''}`;
  modal('Configurazioni personali',body,'<button type="button" data-action="close-dialog">Chiudi</button><button type="button" data-action="save-settings" class="primary">Salva configurazioni</button>');
  const form=$('#settings-form');
  $('[name=timeout]',form).oninput=updateTimeoutHint;updateTimeoutHint();
  const navigation=$('.settings-tabs',form);
  navigation.setAttribute('aria-orientation','vertical');
  const content=document.createElement('div');content.className='settings-content';
  const panel=(id)=>$('#settings-panel-'+id,form);
  const addPanel=id=>{
    const section=document.createElement('section');section.id='settings-panel-'+id;
    section.className='settings-tab-panel';section.setAttribute('role','tabpanel');
    section.setAttribute('aria-labelledby','settings-tab-'+id);section.hidden=true;
    form.append(section);return section;
  };
  const general=addPanel('general'),compaction=addPanel('compaction'),sources=addPanel('sources');
  addPanel('environment');addPanel('cli');
  general.innerHTML=`<section class="settings-section account-summary"><h3>Account personale</h3><p class="account-email" data-no-i18n>${esc(app.user.email)}</p><p class="account-user-id"><span>ID utente</span>: <span data-no-i18n>${esc(app.user.id)}</span></p><p class="note">I tuoi workflow, esecuzioni e configurazioni sono separati dagli altri account.</p></section>`;
  general.append($('.settings-appearance',form));
  const previews=document.createElement('section');previews.className='settings-section';
  previews.innerHTML=`<h3>Anteprima file</h3><label>Limite anteprima file (MB)<input name="file_preview_limit_mb" type="number" min="1" max="100" step="1" required value="${s.file_preview_limit_mb??5}"></label><p class="note">Da 1 a 100 MB. Si applica ai file di testo e Markdown in Storage e Artefatti. Predefinito: 5 MB.</p>`;
  general.append(previews);
  const security=document.createElement('section');security.className='settings-section account-security';
  security.innerHTML=`<h3>Cambia password</h3><div class="form-grid">${accountInput('Password attuale','current','password','password-form','current-password')}${accountInput('Nuova password (almeno 12 caratteri)','new','password','password-form','new-password')}</div><p id="password-error" class="error" role="alert"></p><button type="submit" form="password-form" data-account-password-submit>Aggiorna e accedi di nuovo</button>`;
  // Existing passwords may predate the current minimum length policy.
  $('[name="current"]',security).removeAttribute('minlength');
  general.append(security);
  const execution=addPanel('execution');execution.append($('.settings-execution-limits',form));
  $('#password-form').onsubmit=e=>{e.preventDefault();changeAccountPassword()};
  if(app.user.admin){
   const admin=addPanel('admin');
   admin.innerHTML=`<section class="settings-section admin-users"><h3>Utenti</h3><label>Cerca utenti<input id="admin-user-search" type="search" placeholder="Cerca per email…" autocomplete="off"></label><div class="admin-users-heading"><p class="small" id="admin-user-count" role="status"></p><button type="button" data-action="admin-users-refresh">Aggiorna elenco</button></div><div id="admin-user-list" class="admin-user-list" aria-label="${esc(uiText('Elenco utenti'))}"><p class="note">Caricamento…</p></div></section><section class="settings-section"><h3>Crea utente</h3><div class="form-grid">${accountInput('Email','email','email','user-form','off')}${accountInput('Password iniziale (minimo 12 caratteri)','password','password','user-form','new-password')}</div><p id="user-error" class="error" role="alert"></p><button type="submit" form="user-form" class="primary" data-create-user>${icon('plus')}Crea utente</button></section>`;
   $('#user-form').onsubmit=e=>{e.preventDefault();createAccountUser()};
  }
  for(const rt of ['codex','claude','pi']){
    const card=document.createElement('section');card.className='settings-section';
    card.innerHTML=`<h3>${esc(serviceInfo[rt].name)}</h3>`;
    const fields=$(`[data-service="${rt}"] .compaction-settings`,form);fields.open=true;
    card.append(fields);compaction.append(card);
  }
  compaction.append($('#compact-run',form).closest('.settings-section'));
  sources.append($('[name="source_copy_limit_mib"]',form).closest('.settings-section'));
  for(const tab of $$('[data-settings-tab]',form)){
    const section=panel(tab.dataset.settingsTab),heading=document.createElement('h3');
    heading.className='settings-panel-title';heading.textContent=tab.textContent;
    section.prepend(heading);content.append(section);
  }
  $('.settings-group-title',form)?.remove();
  content.append($('#settings-error',form));
  form.prepend(navigation);form.append(content);
  $('[data-appearance-theme]',form).onchange=e=>{
    applyAppearanceTheme(e.target.value);
    try{localStorage.setItem('nodo.theme',appearanceTheme)}catch{}
  };
  const tabs=$$('[data-settings-tab]',form);
  const activate=tab=>{
    if(tab.dataset.settingsTab==='environment')showEnvironment(panel('environment'));
    if(tab.dataset.settingsTab==='cli')showCliCapabilities(panel('cli'));
    if(tab.dataset.settingsTab==='admin')loadAdminUsers();
    tabs.forEach(b=>{const selected=b===tab;b.classList.toggle('active',selected);b.setAttribute('aria-selected',String(selected));b.tabIndex=selected?0:-1;$('#'+b.getAttribute('aria-controls')).hidden=!selected});
  };
  tabs.forEach((tab,i)=>{
    tab.onclick=()=>activate(tab);
    tab.onkeydown=e=>{let next;if(e.key==='ArrowRight'||e.key==='ArrowDown')next=(i+1)%tabs.length;else if(e.key==='ArrowLeft'||e.key==='ArrowUp')next=(i+tabs.length-1)%tabs.length;else if(e.key==='Home')next=0;else if(e.key==='End')next=tabs.length-1;else return;e.preventDefault();activate(tabs[next]);tabs[next].focus()};
  });
  activate(tabs.find(tab=>tab.dataset.settingsTab===activeTab)||tabs[0]);
  form.addEventListener('invalid',e=>{const panel=e.target.closest('[role="tabpanel"]');if(panel)activate(tabs.find(t=>t.getAttribute('aria-controls')===panel.id))},true);
  form.elements.provider_pi.addEventListener('change',()=>loadPiModels(form));
  loadPiModels(form);
  loadCompactRuns();
}
async function loadPiModels(form){
  const field=form.elements.model_pi,provider=form.elements.provider_pi.value;
  const version=form.piModelsVersion=(form.piModelsVersion||0)+1;
  form.querySelector('#pi-model-catalog')?.dispose?.();
  form.querySelector('#pi-model-catalog')?.remove();
  field.removeAttribute('list');
  if(provider!=='openrouter')return;
  const catalog=document.createElement('div');
  catalog.id='pi-model-catalog';
  catalog.innerHTML='<span class="note" role="status">Caricamento modelli OpenRouter…</span><button type="button" hidden>Riprova</button>';
  field.parentElement.append(catalog);
  const status=catalog.querySelector('[role=status]'),retry=catalog.querySelector('button');
  retry.addEventListener('click',()=>loadPiModels(form));
  try{
    const data=await api('/models/openrouter');
    if(!form.isConnected||form.piModelsVersion!==version)return;
    if(!Array.isArray(data.models)||!data.models.length)throw new Error('Catalogo OpenRouter vuoto');
    catalog.dispose=bindModelSuggestions(field,catalog,data.models);
    status.textContent=`${data.models.length} modelli OpenRouter disponibili. Digita per cercare.`;
  }catch(e){
    if(!form.isConnected||form.piModelsVersion!==version)return;
    status.textContent=e.message+'. Puoi inserire il modello manualmente.';
    retry.hidden=false;
  }
}
async function showCliCapabilities(host=null){
 if($('#cli-dialog'))return;
 const dialog=document.createElement(host?'div':'dialog'),owner=app.user?.id;
 dialog.id='cli-dialog';dialog.className='app-confirm cli-dialog';dialog.setAttribute('aria-labelledby','cli-dialog-title');
 dialog.innerHTML=`<div class="modal-head"><h2 id="cli-dialog-title">CLI disponibili</h2><div class="spacer"></div><button type="button" class="icon-button" data-cli-close aria-label="${esc(uiText('Chiudi'))}">${icon('x')}</button></div><div class="modal-body"><p class="note" data-cli-status role="status">${uiText('Verifica CLI in corso…')}</p><div data-cli-results></div></div><div class="modal-actions"><button type="button" data-cli-close>${uiText('Chiudi')}</button></div>`;
 dialog.querySelectorAll('[data-cli-close]').forEach(button=>button.onclick=()=>dialog.close());
 dialog.addEventListener('close',()=>dialog.remove(),{once:true});if(host){host.append(dialog)}else{document.body.append(dialog);dialog.showModal()}
 try{const result=await api('/capabilities');if(!dialog.isConnected)return;if(app.user?.id!==owner){if(host)dialog.remove();else dialog.close();return}
  $('[data-cli-results]',dialog).innerHTML=Object.entries(result).map(([name,info])=>`<p><b data-no-i18n>${esc(name)}</b> · <span data-no-i18n>${esc(info.available?info.version:info.error)}</span></p>`).join('');
  $('[data-cli-status]',dialog).hidden=true;
 }catch(error){if(dialog.isConnected){const status=$('[data-cli-status]',dialog);status.textContent=error.message;status.classList.add('error')}}
}
function environmentAuthentication(authentication){
 const methods={chatgpt_login:'Login ChatGPT',api_key:'API key',access_token:'Access token',bearer_token:'Token Bearer',oauth_token:'Token OAuth Claude',auth_headers:'Header di autenticazione',native_login:'Login nativo CLI',native_provider:'Autenticazione del provider nativo',aws_credentials:'Credenziali AWS',google_credentials:'Credenziali Google',azure_credentials:'Credenziali Azure',not_configured:'Non configurato',configuration_error:'Configurazione da verificare'};
 const providers={openai:'OpenAI',anthropic:'Anthropic',gateway:'Gateway Anthropic',bedrock:'Amazon Bedrock',bedrock_mantle:'Amazon Bedrock Mantle',anthropic_aws:'Claude Platform on AWS',vertex:'Google Vertex',foundry:'Microsoft Foundry',unknown:'Provider da verificare'};
 const sources={personal_settings:'Configurazioni personali',native_credentials:'Credenziali native della CLI',native_config:'Configurazione nativa della CLI',login_file:'File di login',none:'Nessuna credenziale selezionata',conflicting_providers:'Provider cloud in conflitto'};
 return `<h3>${uiText('Autenticazione agenti')}</h3><p class="note">${uiText('Configurazione salvata per questo utente. La validità delle credenziali non viene verificata online.')}</p><div class="environment-auth-cards">${['claude','codex'].map(runtime=>{
  const auth=authentication?.[runtime];if(!auth)return '';
  const file=auth.login_file,fileStatus=file?(file.exists?(file.active?'Presente · in uso':'Presente · non usato da questa modalità'):'Assente'):'';
  const source=sources[auth.source]?esc(uiText(sources[auth.source])):`<code data-no-i18n>${esc(auth.source)}</code>`;
  return `<article class="environment-auth-card" data-auth-runtime="${runtime}"><h4>${runtime==='claude'?'Claude Code':'Codex'}</h4><p class="environment-auth-method">${esc(uiText(methods[auth.method]||'Configurazione da verificare'))}</p><p class="note"><span>${uiText('Provider')}</span>: <span data-no-i18n>${esc(providers[auth.provider]||auth.provider)}</span></p><p class="note"><span>${uiText('Origine')}</span>: ${source}</p>${file?`<div class="environment-login-file"><b>${uiText('File di login Codex')}</b><code data-no-i18n>${esc(file.path)}</code><span class="environment-file-status ${file.active?'is-active':''}" data-login-file-status>${esc(uiText(fileStatus))}</span></div>`:''}</article>`;
 }).join('')}</div><h3>${uiText('Variabili d’ambiente')}</h3>`;
}
async function showEnvironment(host=null){
 if($('#environment-dialog'))return;
 const dialog=document.createElement(host?'div':'dialog'),owner=app.user?.id;
 dialog.id='environment-dialog';dialog.className='app-confirm environment-dialog';dialog.setAttribute('aria-labelledby','environment-title');
 dialog.innerHTML=`<div class="modal-head"><h2 id="environment-title">Environment</h2><div class="spacer"></div><button type="button" class="icon-button" data-environment-close aria-label="${esc(uiText('Chiudi'))}">${icon('x')}</button></div><div class="modal-body"><div data-environment-auth></div><p class="note">${uiText('Nomi delle variabili d’ambiente con un valore non vuoto visibili al processo dell’app nel container. I valori non vengono mostrati.')}</p><p class="note" data-environment-status role="status">${uiText('Caricamento…')}</p><ul class="environment-list" data-environment-list data-no-i18n></ul></div><div class="modal-actions"><button type="button" data-environment-close>${uiText('Chiudi')}</button></div>`;
 dialog.querySelectorAll('[data-environment-close]').forEach(button=>button.onclick=()=>dialog.close());
 dialog.addEventListener('close',()=>dialog.remove(),{once:true});
 if(host){host.append(dialog)}else{document.body.append(dialog);dialog.showModal()}
 try{const result=await api('/environment');if(!dialog.isConnected)return;if(app.user?.id!==owner){if(host)dialog.remove();else dialog.close();return}
  $('[data-environment-auth]',dialog).innerHTML=environmentAuthentication(result.authentication);
  $('[data-environment-list]',dialog).innerHTML=result.names.map(name=>`<li><code>${esc(name)}</code></li>`).join('');
  const status=$('[data-environment-status]',dialog);status.hidden=result.names.length>0;status.textContent=uiText('Nessuna variabile disponibile.');
 }catch(error){if(dialog.isConnected){const status=$('[data-environment-status]',dialog);status.textContent=error.message;status.classList.add('error')}}
}
async function saveSettings(){if(!$('#settings-form').reportValidity())return;const f=new FormData($('#settings-form')),s=structuredClone(app.settings);s.activity_log_verbosity=f.get('activity_log_verbosity');s.activity_log_page_size=Number(f.get('activity_log_page_size'));s.default_runtime=f.get('default_runtime');s.task_timeout_secs=Number(f.get('timeout'));s.workflow_parallelism=Number(f.get('workflow_parallelism'));s.compaction={};for(const rt of ['claude','codex','pi']){s.compaction[rt]={...(app.settings.compaction?.[rt]||{})};for(const field of ['context_window_tokens','threshold_tokens','trigger_percent','reserve_tokens','keep_recent_tokens'])if(f.has('compact_'+rt+'_'+field))s.compaction[rt][field]=Number(f.get('compact_'+rt+'_'+field));}s.compaction.claude.context_window_tokens=0;s.compaction.claude.threshold_tokens=0;s.source_copy_limit_mib=Number(f.get('source_copy_limit_mib'));s.file_preview_limit_mb=Number(f.get('file_preview_limit_mb'));for(const rt of ['codex','claude','pi'])s.agents[rt]={model:f.get('model_'+rt),effort:f.get('effort_'+rt),api_key:f.get('key_'+rt),provider:rt==='pi'?f.get('provider_pi'):'anthropic',codemode:rt==='pi'&&f.has('codemode_pi')};for(const p of ['github','azure','servicenow'])s.connections[p]={base_url:f.get('base_'+p),token:f.get('token_'+p),repository:f.get('repo_'+p),project:f.get('project_'+p)};const activeTab=$('[data-settings-tab][aria-selected="true"]')?.dataset.settingsTab||'agents';try{app.settings=await api('/settings',s,'PUT');render();await settingsDialog(activeTab);window.dispatchEvent(new Event('settings-saved'));toast('Configurazioni personali salvate')}catch(e){$('#settings-error').textContent=e.message}}
function filePreviewMessage(data){return data.preview_reason==='size'?uiText('Il file supera il limite di anteprima:')+' '+data.preview_limit_mb+' MB. '+uiText('Puoi modificarlo in Configurazioni personali o scaricare il file.'):uiText('Anteprima non disponibile per questo formato. Puoi scaricare il file.')}
async function artifact(id,mode='preview'){
 const d=await api('/artifacts/'+id),a=d.artifact,versions=app.artifacts.filter(x=>artifactKey(x)===artifactKey(a)).sort((a,b)=>a.version-b.version),preview=a.file?.preview!==false;
 let content=preview?(mode==='source'?`<pre class="source-view">${esc(a.content)}</pre>`:a.path.endsWith('.md')?`<div class="markdown">${d.html}</div>`:`<pre class="source-view">${esc(a.content)}</pre>`):`<p class="note">${esc(filePreviewMessage(d))}</p>`;
 if(mode==='diff'&&preview){const previous=versions.filter(x=>x.version<a.version).at(-1);if(previous){const b=(await api('/artifacts/'+previous.id)).artifact;if(b.file?.preview===false)content=`<p class="note">${uiText('Confronto non disponibile per questa versione.')}</p>`;else{const before=b.content.split('\n'),after=a.content.split('\n');let lines='';for(let i=0;i<Math.max(before.length,after.length);i++){if(before[i]===after[i])lines+='  '+esc(after[i]||'')+'\n';else{if(before[i]!==undefined)lines+=`<span class="diff-del">- ${esc(before[i])}</span>`;if(after[i]!==undefined)lines+=`<span class="diff-add">+ ${esc(after[i])}</span>`;}}content=`<p class="note">${uiText('Confronto per riga')} · v${b.version} → v${a.version}</p><pre class="source-view">${lines}</pre>`}}else content=`<p>${uiText('Nessuna versione precedente.')}</p>`}
 modal(a.path,`<div class="artifact-toolbar"><span class="artifact-origin" data-no-i18n>${artifactRoot(a).slice(1)}</span><jui-select id="artifact-version" aria-label="Versione">${versions.map(v=>`<jui-option value="${v.id}" ${v.id===a.id?'selected':''}>${uiText('Versione')} ${v.version} · ${esc(artifactDate(v))}</jui-option>`).join('')}</jui-select>${preview?`<div class="tabs">${['preview','source','diff'].map(m=>`<button class="${mode===m?'active':''}" data-action="artifact-mode" data-id="${a.id}" data-mode="${m}">${icon(m==='preview'?'eye':m==='source'?'code':'git-compare-arrows')}${uiText(m==='preview'?'Anteprima':m==='source'?'Sorgente':'Confronto')}</button>`).join('')}</div>`:''}</div>${a.file?.location?`<label>${uiText('Percorso completo')}<input id="artifact-location" data-no-i18n readonly value="${esc(a.file.location)}"></label>`:''}${content}`,`<a id="artifact-download" class="artifact-download" href="/api/artifacts/${encodeURIComponent(a.id)}/download" download data-no-i18n>${icon('download')}Download</a><button class="primary" data-action="close-dialog">${uiText('Chiudi')}</button>`,true);
 const location=$('#artifact-location');if(location)location.onclick=()=>location.select();
}

async function pending(){if(app.reviewRun&&app.reviewRun!==app.run?.id){await loadRun(app.reviewRun)}const p=app.run?.pending;if(!p)return;const q=p.kind==='question'||p.kind==='gate';modal(q?'Decisione richiesta':'Review in attesa',`<p class="small">Run ${esc(app.run.id.slice(0,8))} · ${esc(p.node)} · ${esc(app.user.email)}</p>${p.questions.map(x=>`<p data-no-i18n>${esc(x)}</p>`).join('')}${p.proposal?`<section class="supervisor-proposal"><h3>${icon('bot')}Proposta del supervisore AI</h3><b>${esc(p.proposal.action)}</b><p data-no-i18n>${esc(p.proposal.rationale)}</p><p class="small">Sessione <span data-no-i18n>${esc(p.proposal.session)}</span> · turno ${p.proposal.turn}</p></section>`:''}<label>${q?'Risposta obbligatoria':'Motivazione / note'}<textarea id="decision-answer" rows="5" ${q?'required':''}>${esc(p.proposal?.answer||'')}</textarea></label><p id="decision-error" class="error" role="alert"></p><p class="note">${q?'La risposta riprende il lavoro nella sessione nativa già salvata.':'La decisione sarà registrata con i riferimenti agli artefatti correnti.'}</p>`,q?'<button class="primary" data-action="decide" data-value="answer">Invia risposta</button>':'<button class="danger" data-action="decide" data-value="reject">Rifiuta</button><button class="primary" data-action="decide" data-value="approve">Approva</button>');}
function reviewHistory(node){const ds=app.decisions.filter(d=>!node||d.node===node).slice().reverse();modal('Registro decisioni',ds.map(d=>`<article class="review-record"><b>${icon(d.actor==='ai'||d.actor==='ai_recommendation'?'bot':'clipboard-check')}${d.actor==='ai'?'Decisione AI':d.actor==='ai_recommendation'?'Proposta AI':'Decisione umana'} · <span data-no-i18n>${esc(d.decision)}</span></b><p data-no-i18n>${esc(d.node)} · ${esc(d.user)}</p>${d.supervisor?`<p data-no-i18n>${esc(d.supervisor.rationale)}</p><p class="small code">${esc(d.supervisor.session)} · turno ${d.supervisor.turn}</p>`:''}<p>${new Date(d.created*1000).toLocaleString(nodoLocale())} · run ${esc(d.run.slice(0,8))}</p><p>${esc(d.answer||'Nessuna nota')}</p><details class="decision-artifacts"><summary><span>Artefatti considerati</span> · ${d.artifacts.length}</summary>${d.artifacts.map(id=>{const a=app.artifacts.find(a=>a.id===id);return a?`<button class="artifact-item" data-action="artifact" data-id="${a.id}">${icon('file-text')} ${esc(a.path)} · v${a.version}</button>`:''}).join('')}</details></article>`).join('')||'<p>Nessuna decisione registrata.</p>');}
function sessions(){const ss=Object.entries(app.run?.sessions||{});modal('Sessioni native persistenti',ss.map(([binding,s])=>`<article class="session-record"><b>${esc(binding)}</b><p class="small">${esc(s.runtime)} · ${s.turns} turni completati · ${esc(s.model||'Modello predefinito')}</p><code class="code">${esc(s.native_id||'In attesa del primo identificativo')}</code></article>`).join('')||'<p>Le sessioni saranno create al primo turno reale.</p>');}
function newNode(kind,position=null){app.selectedNodes=[];app.selectedEdge=null;if(kind==='RAG')ragRefresh().then(()=>{if(app.selected)renderInspector()}).catch(e=>toast(e.message));if(app.run)return toast('Torna alla progettazione per aggiungere nodi');const n={id:kind.toLowerCase()+'-'+crypto.randomUUID().slice(0,8),kind,title:'Nuovo '+kind,phase:'Workflow',x:Math.round((120-app.pan.x)/app.zoom)+(app.wf.nodes.length%4)*175,y:Math.round((150-app.pan.y)/app.zoom)+Math.floor(app.wf.nodes.length/4)*190,agent:app.wf.nodes.find(n=>n.kind==='Agent')?.id||'',runtime:'',prompt:'',command:'',mode:'agent_and_human',source_type:'example',source_mode:'direct',provider:'local',operation:'test',outcomes:kind==='Gate'?['pass','rework']:[],max_rework:2};if(position){n.x=Math.round(position.x-58);n.y=Math.round(position.y-44)}if(kind==='Comment'){Object.assign(n,{title:'Commento',phase:'Note',text:'',width:300,height:220,markdown:false,agent:''});if(position){n.x=Math.round(position.x-150);n.y=Math.round(position.y-20)}}if(kind==='Memory')n.agent='';app.wf.nodes.push(n);if(!app.wf.entry&&!['Agent','Source','MCP','RAG','Comment'].includes(kind))app.wf.entry=n.id;app.selected=n.id;dirty();render();if(kind==='Comment')requestAnimationFrame(()=>$('.comment-node[data-id="'+n.id+'"] textarea')?.focus())}
function graphEdgeKey(edge){return JSON.stringify([edge.from,edge.to,edge.kind,edge.outcome||''])}
function highlightGraphEdge(key){
 const edge=key&&currentWorkflow()?.edges.find(e=>graphEdgeKey(e)===key);
 for(const path of $$('#edges .edge[data-edge-key]')){
  const highlighted=!!edge&&path.dataset.edgeKey===key;
  path.classList.toggle('hovered',highlighted);
  path.setAttribute('marker-end',`url(#arrow-${highlighted||path.dataset.edgeKey===app.selectedEdge?'active':path.dataset.edgeKind})`);
 }
 for(const node of $$('#nodes .node'))node.classList.toggle('edge-endpoint',!!edge&&(node.dataset.id===edge.from||node.dataset.id===edge.to));
}
$('#edges').addEventListener('pointerover',e=>{const hit=e.target.closest('[data-edge-key]');if(hit)highlightGraphEdge(hit.dataset.edgeKey)});
$('#edges').addEventListener('pointerout',e=>{const next=e.relatedTarget?.closest?.('[data-edge-key]');highlightGraphEdge(next?.dataset.edgeKey||null)});
function updateEdgeSelection(){for(const path of $$('#edges .edge[data-edge-key]')){const selected=path.dataset.edgeKey===app.selectedEdge;path.classList.toggle('selected',selected);path.setAttribute('marker-end',`url(#arrow-${selected?'active':path.dataset.edgeKind})`)}for(const hit of $$('#edges .edge-hit'))hit.setAttribute('aria-pressed',String(hit.dataset.edgeKey===app.selectedEdge))}
function selectGraphEdge(key,focus=true){if(!currentWorkflow()?.edges.some(e=>graphEdgeKey(e)===key))return;closeNodeMenu();hideTooltip();app.selectedNodes=[];app.selected=null;app.selectedEdge=key;$$('.node.selected').forEach(n=>n.classList.remove('selected'));renderInspector();updateEdgeSelection();if(focus)$$('#edges .edge-hit').find(p=>p.dataset.edgeKey===key)?.focus({preventScroll:true})}
function deleteGraphEdge(key){if(app.run||!app.wf)return;const index=app.wf.edges.findIndex(e=>graphEdgeKey(e)===key);if(index<0)return;cancelEdgeDrag();closeNodeMenu();deletedNodes=deletedNodes.filter(d=>d.workflow===app.wf.id);deletedNodes.push({type:'edge',workflow:app.wf.id,index,edge:structuredClone(app.wf.edges[index])});if(deletedNodes.length>50)deletedNodes.shift();app.wf.edges.splice(index,1);app.selectedEdge=null;dirty();render();toast('Collegamento cancellato · Ctrl+Z / Cmd+Z per annullare')}

let deletedNodes=[];
function deleteNode(id){
  if(app.run||!app.wf?.nodes.some(n=>n.id===id))return;
  cancelEdgeDrag();closeNodeMenu();
  deletedNodes=deletedNodes.filter(d=>d.workflow===app.wf.id);
  deletedNodes.push({workflow:app.wf.id,node:structuredClone(app.wf.nodes.find(n=>n.id===id)),index:app.wf.nodes.findIndex(n=>n.id===id),edges:app.wf.edges.flatMap((e,index)=>e.from===id||e.to===id?[{edge:structuredClone(e),index}]:[]),references:app.wf.nodes.filter(n=>n.agent===id||n.target===id).map(n=>({id:n.id,agent:n.agent===id,target:n.target===id})),entry:app.wf.entry===id});
  if(deletedNodes.length>50)deletedNodes.shift();
  app.wf.nodes=app.wf.nodes.filter(n=>n.id!==id);
  app.wf.edges=app.wf.edges.filter(e=>e.from!==id&&e.to!==id);
  for(const n of app.wf.nodes){if(n.agent===id)n.agent='';if(n.target===id)n.target=''}
  if(app.wf.entry===id)app.wf.entry='';
  if(app.link===id)app.link=null;
  app.selected=null;app.selectedEdge=null;dirty();render();toast('Nodo cancellato · Ctrl+Z / Cmd+Z per annullare');
}
function deleteSelectedNodes(){
 const ids=new Set(selectedNodeIds());if(app.run||!app.wf||!ids.size)return;
 if(ids.size===1)return deleteNode([...ids][0]);
 cancelGraphGesture();cancelEdgeDrag();closeNodeMenu();
 const w=app.wf;
 deletedNodes=deletedNodes.filter(d=>d.workflow===w.id);
 deletedNodes.push({type:'nodes',workflow:w.id,nodes:w.nodes.flatMap((node,index)=>ids.has(node.id)?[{node:structuredClone(node),index}]:[]),edges:w.edges.flatMap((edge,index)=>ids.has(edge.from)||ids.has(edge.to)?[{edge:structuredClone(edge),index}]:[]),references:w.nodes.filter(n=>!ids.has(n.id)&&(ids.has(n.agent)||ids.has(n.target))).map(n=>({id:n.id,agent:ids.has(n.agent)?n.agent:null,target:ids.has(n.target)?n.target:null})),entry:ids.has(w.entry)?w.entry:null});
 if(deletedNodes.length>50)deletedNodes.shift();
 w.nodes=w.nodes.filter(n=>!ids.has(n.id));w.edges=w.edges.filter(e=>!ids.has(e.from)&&!ids.has(e.to));
 for(const n of w.nodes){if(ids.has(n.agent))n.agent='';if(ids.has(n.target))n.target=''}
 if(ids.has(w.entry))w.entry='';
 setNodeSelection([]);dirty();render();toast(uiText('Selezione cancellata · Ctrl+Z / Cmd+Z per annullare'));
}
function undoNodeDeletion(){
  if(app.run)return;
  const d=deletedNodes.at(-1);if(!d||d.workflow!==app.wf.id)return;
  deletedNodes.pop();
  if(d.type==='nodes'){
   if(d.nodes.some(({node})=>app.wf.nodes.some(n=>n.id===node.id)))return;
   closeNodeMenu();
   for(const {node,index} of d.nodes)app.wf.nodes.splice(index,0,node);
   for(const {edge,index} of d.edges)if(app.wf.nodes.some(n=>n.id===edge.from)&&app.wf.nodes.some(n=>n.id===edge.to)&&!app.wf.edges.some(e=>graphEdgeKey(e)===graphEdgeKey(edge)))app.wf.edges.splice(index,0,edge);
   for(const ref of d.references){const node=app.wf.nodes.find(n=>n.id===ref.id);if(node){if(ref.agent&&!node.agent)node.agent=ref.agent;if(ref.target&&!node.target)node.target=ref.target}}
   if(d.entry&&!app.wf.entry)app.wf.entry=d.entry;
   setNodeSelection(d.nodes.map(({node})=>node.id));dirty();render();toast(uiText('Selezione e collegamenti ripristinati'));return;
  }

  if(d.type==='edge'){if(!app.wf.nodes.some(n=>n.id===d.edge.from)||!app.wf.nodes.some(n=>n.id===d.edge.to))return;closeNodeMenu();if(!app.wf.edges.some(e=>graphEdgeKey(e)===graphEdgeKey(d.edge)))app.wf.edges.splice(d.index,0,d.edge);app.selected=null;app.selectedEdge=graphEdgeKey(d.edge);dirty();render();toast('Collegamento ripristinato');return}
  if(app.wf.nodes.some(n=>n.id===d.node.id))return;
  closeNodeMenu();app.wf.nodes.splice(d.index,0,d.node);
  for(const {edge,index} of d.edges){if(app.wf.nodes.some(n=>n.id===edge.from)&&app.wf.nodes.some(n=>n.id===edge.to)&&!app.wf.edges.some(e=>JSON.stringify(e)===JSON.stringify(edge)))app.wf.edges.splice(index,0,edge)}
  for(const ref of d.references){const n=app.wf.nodes.find(n=>n.id===ref.id);if(n){if(ref.agent&&!n.agent)n.agent=d.node.id;if(ref.target&&!n.target)n.target=d.node.id}}
  if(d.entry&&!app.wf.entry)app.wf.entry=d.node.id;
  app.selected=d.node.id;app.selectedEdge=null;dirty();render();toast('Nodo e collegamenti ripristinati');
}
let nodeMenu=null,graphContextTarget=null;
document.addEventListener('pointerdown',e=>{
 if(e.button!==2)return;
 const node=e.target.closest('.node,.step-row'),edge=e.target.closest('.edge-hit,.edge-label[data-edge-key]');
 graphContextTarget=!e.target.closest('.comment-body')&&(node||edge)?{node:node?.dataset.id,edge:edge?.dataset.edgeKey,workflow:currentWorkflow()?.id,run:app.run?.id,at:Date.now()}:null;
},true);
function closeNodeMenu(){nodeMenu?.remove();nodeMenu=null}
document.addEventListener('contextmenu',e=>{
  const pointer=graphContextTarget;graphContextTarget=null;
  if(e.target.closest('.comment-body')&&!pointer)return;
  closeNodeMenu();
  let node=e.target.closest('.node,.step-row'),edge=e.target.closest('.edge-hit,.edge-label[data-edge-key]');
  if(!node&&!edge&&pointer&&Date.now()-pointer.at<1500&&pointer.workflow===currentWorkflow()?.id&&pointer.run===app.run?.id){
   if(pointer.node)node=$$('#nodes .node,.step-row').find(n=>n.dataset.id===pointer.node);
   else if(pointer.edge)edge=$$('#edges .edge-hit,.edge-label[data-edge-key]').find(el=>el.dataset.edgeKey===pointer.edge);
  }
  if((!node&&!edge)||$('#dialog').open)return;
  e.preventDefault();if(edge)selectGraphEdge(edge.dataset.edgeKey);else{if(!selectedNodeIds().includes(node.dataset.id))setNodeSelection([node.dataset.id]);render()}hideTooltip();
  nodeMenu=document.createElement('div');nodeMenu.className='node-context-menu';nodeMenu.setAttribute('role','menu');
  nodeMenu.innerHTML=(app.run?`<p class="node-context-note">${esc(uiText('Esecuzione in sola lettura. Torna alla progettazione per cancellare nodi e collegamenti.'))}</p><button type="button" role="menuitem" data-action="edit-run-workflow">${icon('pencil-ruler')}<span>${esc(uiText('Torna alla progettazione'))}</span></button>`:'')+(node&&selectedNodeIds().length<2?`<button type="button" role="menuitem" data-action="continue-run-node" data-node="${esc(node.dataset.id)}" ${app.run&&app.continueNodes.includes(node.dataset.id)?'':'disabled'}>${icon('play')}<span>Continue run</span></button>`:'')+`<button type="button" role="menuitem" ${app.run?'disabled':''} data-action="${edge?'delete-link':selectedNodeIds().length>1?'delete-selected-nodes':'delete-node'}" ${edge?`data-edge-key="${esc(edge.dataset.edgeKey)}"`:`data-id="${esc(app.selected)}"`}>${icon('trash-2')}<span>${selectedNodeIds().length>1?esc(uiText('Cancella selezione'))+' ('+selectedNodeIds().length+')':esc(uiText('Cancella'))}</span><kbd>⌫</kbd></button>`;
  document.body.append(nodeMenu);
  const r=nodeMenu.getBoundingClientRect();
  nodeMenu.style.left=Math.max(4,Math.min(e.clientX,innerWidth-r.width-4))+'px';
  nodeMenu.style.top=Math.max(4,Math.min(e.clientY,innerHeight-r.height-4))+'px';
  nodeMenu.querySelector('button:not(:disabled)')?.focus({preventScroll:true});
});
document.addEventListener('pointerdown',e=>{if(!e.target.closest('.node-context-menu'))closeNodeMenu()});
window.addEventListener('resize',closeNodeMenu);
document.addEventListener('scroll',closeNodeMenu,true);
function controlCycle(from,to){const pending=[to],seen=new Set();while(pending.length){const id=pending.pop();if(id===from)return true;if(seen.has(id))continue;seen.add(id);pending.push(...app.wf.edges.filter(e=>e.kind==='control'&&e.from===id).map(e=>e.to))}return false}
function edgeType(from,to){const a=app.wf?.nodes.find(n=>n.id===from),b=app.wf?.nodes.find(n=>n.id===to);if(!a||!b||from===to||a.kind==='Comment'||b.kind==='Comment')return null;if(a.kind==='Source')return ['Agent','Connection','Workflow','A2A'].includes(b.kind)?'context':null;if(['MCP','RAG'].includes(a.kind))return b.kind==='Agent'?'context':null;if(a.kind==='Agent')return b.kind==='Task'&&b.agent===a.id?'context':null;if(['Source','Agent','MCP','RAG'].includes(b.kind))return null;if(controlCycle(from,to))return ['Gate','Review'].includes(a.kind)&&['Task','Workflow'].includes(b.kind)?'rework':null;return 'control'}
function addGraphEdge(edge){if(app.run)return false;const kind=edgeType(edge.from,edge.to),a=app.wf.nodes.find(n=>n.id===edge.from),b=app.wf.nodes.find(n=>n.id===edge.to);const valid=kind&&(edge.kind==='context'?kind==='context':edge.kind==='control'?kind!=='context'&&!controlCycle(edge.from,edge.to):edge.kind==='rework'&&['Gate','Review'].includes(a.kind)&&['Task','Workflow'].includes(b.kind)&&edge.outcome==='rework');if(!valid){toast('Queste porte non sono compatibili');return false}if(app.wf.edges.some(e=>e.from===edge.from&&e.to===edge.to&&e.kind===edge.kind&&(e.outcome||'')===(edge.outcome||''))){toast('Collegamento già presente');return false}app.wf.edges.push(edge);app.selected=null;app.selectedEdge=graphEdgeKey(edge);dirty();render();return true}
function connect(from,to){if(app.run)return;const a=app.wf.nodes.find(n=>n.id===from),b=app.wf.nodes.find(n=>n.id===to),kind=edgeType(from,to);if(!kind)return toast('Queste porte non sono compatibili');if(!['Gate','Review'].includes(a.kind)){addGraphEdge({from,to,kind,outcome:''});return}const choices=a.kind==='Gate'?(a.outcomes||[]).map(o=>[o,o]):[['','Continua'],['rework','rework']];const outcome=kind==='rework'?'rework':a.kind==='Gate'?(a.outcomes||[]).find(o=>o!=='rework'&&!app.wf.edges.some(e=>e.from===from&&e.outcome===o))||(a.outcomes||[])[0]||'pass':'';modal('Esito collegamento',`<p>${esc(a.title)} → ${esc(b.title)}</p><form id="edge-form">${select('Esito','outcome',outcome,choices)}<input type="hidden" name="kind" value="${kind}"><input type="hidden" name="from" value="${esc(from)}"><input type="hidden" name="to" value="${esc(to)}"></form>`,'<button class="primary" data-action="save-edge">Collega</button>');const updateKind=()=>{$('#edge-form [name=kind]').value=$('#edge-form [name=outcome]').value==='rework'&&['Task','Workflow'].includes(b.kind)?'rework':'control'};$('#edge-form [name=outcome]').onchange=updateKind;updateKind()}
function edgePoint(e){const r=$('#graph-canvas').getBoundingClientRect();return {x:(e.clientX-r.left-app.pan.x)/app.zoom,y:(e.clientY-r.top-app.pan.y)/app.zoom}}
function edgeDropPort(e){const el=document.elementFromPoint(e.clientX,e.clientY)?.closest(edgeDrag?.direction==='out'?'.port.in':'.port.out');return el&&$('#nodes').contains(el)?el:null}
function edgeDragPair(port){if(!edgeDrag||!port)return null;return edgeDrag.direction==='out'?{from:edgeDrag.node,to:port.dataset.id}:{from:port.dataset.id,to:edgeDrag.node}}
function drawEdgePreview(){if(!edgeDrag)return;const node=app.wf?.nodes.find(n=>n.id===edgeDrag.node);if(!node)return;const target=edgeDrag.target&&app.wf.nodes.find(n=>n.id===edgeDrag.target);const a=edgeDrag.direction==='out'?{x:node.x+103,y:node.y+45}:target?{x:target.x+103,y:target.y+45}:edgeDrag.point;const b=edgeDrag.direction==='in'?{x:node.x+7,y:node.y+45}:target?{x:target.x+7,y:target.y+45}:edgeDrag.point;const c=Math.max(60,Math.abs(b.x-a.x)/2);let path=$('#edge-preview');if(!path){path=document.createElementNS('http://www.w3.org/2000/svg','path');path.id='edge-preview';path.setAttribute('class','edge-preview');path.setAttribute('marker-end','url(#arrow-active)');$('#edges').append(path)}const pair=target?(edgeDrag.direction==='out'?{from:node.id,to:target.id}:{from:target.id,to:node.id}):null;const geometry=pair&&edgeGeometry(app.wf,{...pair,kind:edgeType(pair.from,pair.to)});path.setAttribute('d',geometry?.d||`M${a.x} ${a.y}C${a.x+c} ${a.y},${b.x-c} ${b.y},${b.x} ${b.y}`);path.classList.toggle('invalid',edgeDrag.invalid)}
function cancelEdgeDrag(){const active=edgeDrag;edgeDrag=null;$('#edge-preview')?.remove();$('#graph-canvas').classList.remove('linking','link-invalid');$$('.port.link-compatible,.port.drop-target,.port.chosen').forEach(p=>p.classList.remove('link-compatible','drop-target','chosen'));if(active&&$('#graph-canvas').hasPointerCapture(active.pointer))$('#graph-canvas').releasePointerCapture(active.pointer)}
function moveEdgeDrag(e){if(!edgeDrag||edgeDrag.pointer!==e.pointerId)return;edgeDrag.moved ||= Math.hypot(e.clientX-edgeDrag.startX,e.clientY-edgeDrag.startY)>5;edgeDrag.point=edgePoint(e);const port=edgeDropPort(e),pair=edgeDragPair(port),valid=pair&&edgeType(pair.from,pair.to);edgeDrag.target=valid?port.dataset.id:null;edgeDrag.invalid=!!port&&!valid;$$('.port.drop-target').forEach(p=>p.classList.remove('drop-target'));if(valid)port.classList.add('drop-target');$('#graph-canvas').classList.toggle('link-invalid',edgeDrag.invalid);drawEdgePreview()}
$('#palette-items').innerHTML=Object.keys(icons).map(k=>`<button data-action="add-node" data-kind="${k}" data-tip="${k}" aria-label="Aggiungi ${k}">${icon(icons[k])}</button>`).join('');
// Keep pointer focus on the editor; keyboard users retain the palette focus ring.
let paletteDrag=null,paletteClickSuppressed=false;
function cancelPaletteDrag(){const active=paletteDrag;if(active?.moved)paletteClickSuppressed=true;paletteDrag=null;document.querySelector('.palette-drag-preview')?.remove();document.body.classList.remove('dragging-palette');$('#graph-canvas').classList.remove('palette-drop-ready');if(active?.button.hasPointerCapture(active.pointer))active.button.releasePointerCapture(active.pointer)}
function paletteDropAllowed(e){const canvas=$('#graph-canvas'),target=document.elementFromPoint(e.clientX,e.clientY);return !app.run&&app.view==='graph'&&!$('#dialog').open&&canvas.contains(target)&&!target.closest('.graph-tools,.zoom-controls')}
$('#palette-items').addEventListener('pointerdown',e=>{const button=e.target.closest('[data-action="add-node"]');if(!button||e.button!==0)return;paletteClickSuppressed=false;e.preventDefault();button.blur();if(app.run||app.view!=='graph')return;cancelEdgeDrag();hideTooltip();paletteDrag={button,kind:button.dataset.kind,pointer:e.pointerId,x:e.clientX,y:e.clientY,moved:false};button.setPointerCapture(e.pointerId)});
$('#palette-items').addEventListener('pointermove',e=>{if(!paletteDrag||paletteDrag.pointer!==e.pointerId)return;const active=paletteDrag;active.moved ||= Math.hypot(e.clientX-active.x,e.clientY-active.y)>5;if(!active.moved)return;hideTooltip();let preview=document.querySelector('.palette-drag-preview');if(!preview){preview=document.createElement('div');preview.className='palette-drag-preview';preview.setAttribute('aria-hidden','true');preview.innerHTML=icon(icons[active.kind]);document.body.append(preview);document.body.classList.add('dragging-palette')}preview.style.left=e.clientX+'px';preview.style.top=e.clientY+'px';const allowed=paletteDropAllowed(e);preview.classList.toggle('invalid',!allowed);$('#graph-canvas').classList.toggle('palette-drop-ready',allowed)});
$('#palette-items').addEventListener('pointerup',e=>{if(!paletteDrag||paletteDrag.pointer!==e.pointerId)return;const active=paletteDrag,allowed=paletteDropAllowed(e),position=edgePoint(e);cancelPaletteDrag();if(!active.moved)return;paletteClickSuppressed=true;if(allowed){newNode(active.kind,position);$('#graph-canvas').setAttribute('tabindex','-1');$('#graph-canvas').focus({preventScroll:true})}});
$('#palette-items').addEventListener('click',e=>{if(paletteClickSuppressed&&e.detail!==0){paletteClickSuppressed=false;e.preventDefault();e.stopPropagation()}});
$('#palette-items').addEventListener('pointercancel',cancelPaletteDrag);
$('#palette-items').addEventListener('lostpointercapture',e=>{if(paletteDrag?.pointer===e.pointerId)cancelPaletteDrag()});
window.addEventListener('blur',cancelPaletteDrag);
$('#auth-form').addEventListener('submit',async e=>{e.preventDefault();const b=Object.fromEntries(new FormData(e.target));$('#auth-error').textContent='';$('#auth-submit').disabled=true;try{await api(registerMode?'/auth/register':'/auth/login',b);await boot()}catch(e){$('#auth-error').textContent=e.message}finally{$('#auth-submit').disabled=false}});
$('#auth-switch').onclick=()=>{registerMode=!registerMode;$('#auth-title').textContent=registerMode?'Crea un account':'Accedi al workspace';$('#auth-submit').textContent=registerMode?'Registrati':'Accedi'};
$('#workflow-select').onclick=()=>{const menu=$('#workflow-topic-menu');menu.hidden=!menu.hidden;$('#workflow-select').setAttribute('aria-expanded',String(!menu.hidden))};
document.addEventListener('click',e=>{if(!e.target.closest('.workflow-topic-picker'))closeWorkflowMenu()});
document.addEventListener('keydown',e=>{if(e.key==='Escape'&&!$('#workflow-topic-menu').hidden){closeWorkflowMenu();$('#workflow-select').focus()}});
$('#new-workflow').onclick=showWorkflowCatalog;
$('#duplicate').onclick=async()=>{try{const w=structuredClone(currentWorkflow());w.id=crypto.randomUUID();w.created=0;w.name+=' · copia';leaveDraftContext();app.editBase=null;app.run=null;disconnect();app.wf=w;await saveWorkflow();render();toast('Copia personale creata')}catch(e){toast(e.message)}};
$('#edit-continue').onclick=beginRunRevision;$('#revision-preview').onclick=()=>previewRevision();$('#revision-cancel').onclick=async()=>{const base=app.editBase?.id;if(!base)return;try{if(app.dirty)await saveWorkflow();await loadRun(base)}catch(error){toast(error.message)}};
$('#draft-retry')?.addEventListener('click',()=>saveWorkflow().catch(error=>toast(error.message)));
$('#download-workflow').onclick=downloadWorkflow;
$('#save').onclick=()=>saveWorkflow().then(()=>toast('Workflow salvato')).catch(e=>toast(e.message));
$('#validate').onclick=()=>api('/workflows/validate',currentWorkflow()).then(()=>toast('Grafo valido: collegamenti e handoff coerenti')).catch(e=>toast(e.message));
$('#settings').onclick=()=>settingsDialog().catch(e=>toast(e.message));$('#account').onclick=()=>toggleAccountMenu();
$('#execution-policy').onclick=policyDialog;
$('#execute').onclick=async()=>{try{if(app.editBase){await previewRevision();return}if(app.run&&app.run.status==='running')return; if(!app.run)await saveWorkflow();app.runs=await api('/runs');modal('Avvia esecuzione reale',`<p class="note">I nodi invocheranno agenti reali usando le tue credenziali. Le cartelle in /sources seguono la modalità scelta nel nodo: modifica permanente o copia. I repository remoti vengono sempre clonati nel workspace. Scegli come gestire review e domande per questa esecuzione.</p><form id="run-form">${policyFields(app.run?.execution||app.wf.execution)}<label>Richiesta e contesto aggiuntivo<textarea name="input" rows="3"></textarea></label>${select('Base del repository','base_run','',[['','Source configurato nel workflow'],...app.runs.filter(r=>r.status==='completed').map(r=>[r.id,r.name+' · '+r.id.slice(0,8)])])}</form>`,'<button class="primary" data-action="start-run">Avvia con agenti reali</button>')}catch(e){toast(e.message)}};
$('#stop').onclick=async()=>{const id=app.run.id;$('#stop').disabled=true;try{await api('/runs/'+id+'/stop',{});toast('Run interrotta');if(app.run?.id===id)await refreshRun();}catch(e){toast(e.message);}finally{$('#stop').disabled=false;}};
$('#pause').onclick=()=>api('/runs/'+app.run.id+'/pause',{}).then(()=>toast('Pausa richiesta al termine del turno attivo')).catch(e=>toast(e.message));
$('#resume').onclick=async()=>{if(['failed','interrupted'].includes(app.run.status)&&!(await appConfirm({title:'Riprendi esecuzione',message:'Il turno precedente può avere modificato file o sistemi esterni. Hai verificato gli effetti e vuoi riprenderlo nella stessa sessione?',accept:'Riprendi nella stessa sessione'})))return;try{await api('/runs/'+app.run.id+'/start',{acknowledge:true});await refreshRun()}catch(e){toast(e.message)}};
$('#add-context').onclick=()=>modal('Chiarisci il contesto',`<p class="note">La richiesta originale resta conservata. Il chiarimento sarà registrato e passato ai turni successivi nella stessa sessione.</p><label>Nuove istruzioni<textarea id="run-note" rows="5"></textarea></label>`,'<button class="primary" data-action="save-note">Registra chiarimento</button>');$('#close-run').onclick=()=>selectWorkflow(app.run.workflow.id);$('#review-banner').onclick=pending;
for(const [id,view]of [['graph-tab','graph'],['list-tab','list'],['history-tab','history']])$('#'+id).onclick=async()=>{app.view=view;render();if(view==='history')try{await refreshHistory()}catch(e){toast(e.message)}};
$('#fit').onclick=fit;$('#follow').onclick=()=>centerNode(app.run?.current||app.selected||app.wf.entry);$('#expand').onclick=()=>{const expanded=document.body.classList.toggle('expanded');$('#expand').setAttribute('aria-pressed',String(expanded));$('#expand').innerHTML=icon(expanded?'resize':'expand');const label=uiText(expanded?'Ripristina':'Espandi');$('#expand').dataset.tip=label;$('#expand').setAttribute('aria-label',label);hideTooltip()};$('#sessions').onclick=sessions;
function logs(open){$('#logs').hidden=!open;document.body.classList.toggle('has-logs',open);if(open){renderActivityLog();loadActivityRuns().catch(error=>toast(error.message))}}$('#toggle-logs').onclick=()=>logs($('#logs').hidden);$('#logs-close').onclick=()=>logs(false);
$('#zoom-in').onclick=()=>{app.zoom=Math.min(1.8,app.zoom*1.15);transform()};$('#zoom-out').onclick=()=>{app.zoom=Math.max(.18,app.zoom/1.15);transform()};
function updateWheelZoomToggle(){
  $('#wheel-zoom-toggle').setAttribute('aria-checked',String(wheelZoomEnabled));
  $('#graph-navigation-hint').textContent=wheelZoomEnabled?'Trascina per spostare · Rotellina per zoom':'Trascina per spostare';
}
$('#wheel-zoom-toggle').onclick=()=>{
  wheelZoomEnabled=!wheelZoomEnabled;
  try{localStorage.setItem('nodo.wheelZoom',String(wheelZoomEnabled))}catch{}
  updateWheelZoomToggle();
};
updateWheelZoomToggle();
$('#graph-canvas').addEventListener('wheel',e=>{if(e.target.closest('.comment-body'))return;e.preventDefault();if(!wheelZoomEnabled||edgeDrag||e.target.closest('.graph-tools,.zoom-controls'))return;const r=e.currentTarget.getBoundingClientRect(),x=e.clientX-r.left,y=e.clientY-r.top,old=app.zoom;app.zoom=Math.max(.18,Math.min(1.8,old*(e.deltaY>0?.92:1.08)));app.pan.x=x-(x-app.pan.x)*app.zoom/old;app.pan.y=y-(y-app.pan.y)*app.zoom/old;transform()},{passive:false});
let drag=null,suppressClick=false;
function removeMarquee(){$('#selection-marquee')?.remove();$('#graph-canvas').classList.remove('marquee-selecting')}
function cancelGraphGesture(){
 if(drag?.box){setNodeSelection(drag.before);removeMarquee();paintNodeSelection();renderInspector()}
 else if(drag?.members&&drag.moved){for(const item of drag.members){item.node.x=item.x;item.node.y=item.y;const el=$('#nodes .node[data-id="'+item.node.id+'"]');if(el){el.style.left=item.x+'px';el.style.top=item.y+'px'}}drawEdges()}
 drag=null;
}
$('#graph-canvas').addEventListener('pointerdown',e=>{
 if(e.button===2){if(!e.target.closest('.comment-body'))e.preventDefault();return}
 if(e.button!==0)return;
 const hit=e.target.closest('.edge-hit,.edge-label[data-edge-key]');if(hit){e.preventDefault();selectGraphEdge(hit.dataset.edgeKey);return}
 const port=e.target.closest('.port');if(port){if(app.run)return;e.preventDefault();hideTooltip();app.selectedEdge=null;updateEdgeSelection();app.link=null;const direction=port.classList.contains('out')?'out':'in';edgeDrag={node:port.dataset.id,direction,pointer:e.pointerId,startX:e.clientX,startY:e.clientY,point:edgePoint(e),target:null,invalid:false,moved:false};port.classList.add('chosen');$('#graph-canvas').classList.add('linking');const selector=direction==='out'?'.port.in':'.port.out';$$(selector).forEach(p=>{const pair=edgeDragPair(p);p.classList.toggle('link-compatible',!!edgeType(pair.from,pair.to))});$('#graph-canvas').setPointerCapture(e.pointerId);drawEdgePreview();return}
 if(e.target.closest('.graph-tools,.zoom-controls,.comment-body,.comment-actions')||e.target.closest('.comment-node')&&!e.target.closest('.comment-header'))return;
 const el=e.target.closest('.node'),node=el&&currentWorkflow().nodes.find(n=>n.id===el.dataset.id),before=selectedNodeIds();
 drag={pointer:e.pointerId,x:e.clientX,y:e.clientY,pan:{...app.pan},node,moved:false,before,box:!app.run&&!node&&e.shiftKey,toggle:!app.run&&!!node&&e.shiftKey};
 if(drag.box){e.preventDefault();hideTooltip();app.link=null;$('#graph-canvas').classList.add('marquee-selecting');$('#graph-canvas').setAttribute('tabindex','-1');$('#graph-canvas').focus({preventScroll:true});drag.add=e.ctrlKey||e.metaKey;const box=document.createElement('div');box.id='selection-marquee';box.setAttribute('aria-hidden','true');$('#graph-canvas').append(box)}
 else if(node&&!app.run&&!drag.toggle){if(!before.includes(node.id))setNodeSelection([node.id]);drag.members=selectedNodeIds().map(id=>app.wf.nodes.find(n=>n.id===id)).filter(Boolean).map(n=>({node:n,x:n.x,y:n.y}));paintNodeSelection();renderInspector()}
 $('#graph-canvas').setPointerCapture(e.pointerId);
});
$('#graph-canvas').addEventListener('pointermove',e=>{
 if(edgeDrag){moveEdgeDrag(e);return}if(!drag||drag.pointer!==e.pointerId)return;
 const dx=e.clientX-drag.x,dy=e.clientY-drag.y;if(Math.abs(dx)+Math.abs(dy)>5)drag.moved=true;if(!drag.moved||drag.toggle)return;
 if(drag.box){
  const rect=$('#graph-canvas').getBoundingClientRect(),left=Math.min(drag.x,e.clientX),top=Math.min(drag.y,e.clientY),right=Math.max(drag.x,e.clientX),bottom=Math.max(drag.y,e.clientY),box=$('#selection-marquee');
  Object.assign(box.style,{left:(left-rect.left)+'px',top:(top-rect.top)+'px',width:(right-left)+'px',height:(bottom-top)+'px'});
  const hits=$$('#nodes .node').filter(el=>{const r=el.getBoundingClientRect();return r.width&&r.height&&r.right>=left&&r.left<=right&&r.bottom>=top&&r.top<=bottom}).map(el=>el.dataset.id);
  setNodeSelection(drag.add?[...drag.before,...hits]:hits);paintNodeSelection();return;
 }
 if(drag.members){for(const item of drag.members){item.node.x=item.x+dx/app.zoom;item.node.y=item.y+dy/app.zoom;const el=$('#nodes .node[data-id="'+item.node.id+'"]');el.style.left=item.node.x+'px';el.style.top=item.node.y+'px'}drawEdges()}
 else{app.pan.x=drag.pan.x+dx;app.pan.y=drag.pan.y+dy;transform()}
});
$('#graph-canvas').addEventListener('pointerup',e=>{
 if(edgeDrag&&edgeDrag.pointer===e.pointerId){moveEdgeDrag(e);const active=edgeDrag,port=edgeDropPort(e),pair=edgeDragPair(port);cancelEdgeDrag();suppressClick=true;setTimeout(()=>suppressClick=false,0);if(active.moved){if(pair)connect(pair.from,pair.to)}else if(active.direction==='out'){app.link=active.node;renderGraph();toast('Trascina la linea verso una porta di ingresso')}return}
 if(!drag||drag.pointer!==e.pointerId)return;
 const gesture=drag;drag=null;suppressClick=true;
 if(gesture.box){removeMarquee();if(!gesture.moved)setNodeSelection([]);paintNodeSelection();renderInspector()}
 else if(gesture.toggle){setNodeSelection(gesture.before.includes(gesture.node.id)?gesture.before.filter(id=>id!==gesture.node.id):[...gesture.before,gesture.node.id]);paintNodeSelection();renderInspector()}
 else if(gesture.moved&&gesture.members){dirty();renderInspector()}
 else if(!gesture.moved){setNodeSelection(gesture.node?[gesture.node.id]:[]);render()}
 if($('#graph-canvas').hasPointerCapture(e.pointerId))$('#graph-canvas').releasePointerCapture(e.pointerId);
 setTimeout(()=>suppressClick=false,0);
});
$('#graph-canvas').addEventListener('pointercancel',()=>{cancelEdgeDrag();cancelGraphGesture()});
$('#graph-canvas').addEventListener('lostpointercapture',e=>{if(edgeDrag?.pointer===e.pointerId)cancelEdgeDrag();if(drag?.pointer===e.pointerId)cancelGraphGesture()});
window.addEventListener('blur',()=>{cancelEdgeDrag();cancelGraphGesture()});
function updateInspectorField(field){
 if(app.run||!field.form?.isConnected||field.disabled||!field.name||field.form?.id!=='node-form'||$('#inspector').hidden||field.form.dataset.workflowId!==app.wf?.id||field.form.dataset.nodeId!==app.selected)return;
 const n=app.wf?.nodes.find(n=>n.id===field.form.dataset.nodeId);if(!n)return;
 const k=field.name,v=field.value;
 const value=k==='mcp_tools'?v.split(/[\s,]+/).filter(Boolean):k==='outcomes'?v.split(',').map(s=>s.trim()).filter(Boolean):['max_rework','rag_top_k'].includes(k)?Number(v):['require_tests','require_tasks_closed'].includes(k)?v==='true':v;
 if(JSON.stringify(n[k])!==JSON.stringify(value)){n[k]=value;dirty()}
}
function captureInspectorFields(){const form=$('#node-form');if(form&&!app.run)for(const field of form.elements)updateInspectorField(field)}
// Keep edits in the workflow draft without replacing the focused inspector fields.
document.addEventListener('input',e=>updateInspectorField(e.target));
document.addEventListener('focusout',e=>{if(!app.run&&e.target.form?.id==='node-form'&&e.target.form.isConnected){updateInspectorField(e.target);renderGraph();renderList()}});
document.addEventListener('change',e=>updateInspectorField(e.target));
document.addEventListener('submit',e=>{if(e.target.id==='node-form'){e.preventDefault();if(app.run)return;captureInspectorFields();saveWorkflow().catch(error=>toast(error.message))}});
document.addEventListener('change',e=>{if(e.target.matches('#node-form [name=provider]')){const f=e.target.form,n=currentWorkflow().nodes.find(n=>n.id===app.selected);f.twgCommand=f.elements.command?.value??f.twgCommand??n.command;$('#connection-fields').innerHTML=connectionFields({...n,provider:e.target.value,command:f.twgCommand});captureInspectorFields()}if(e.target.matches('#node-form [name=source_type]')){$('#source-mode-fields').hidden=e.target.value!=='folder';loadSourceFolderOptions(e.target.form)}if(e.target.matches('#node-form [name=source_mode]')&&e.target.value==='copy'&&e.target.form.elements.location.value){const form=e.target.form;confirmSourceCopy({location:form.elements.location.value}).then(ok=>{if(!ok){e.target.value='direct';updateInspectorField(e.target)}}).catch(error=>{e.target.value='direct';updateInspectorField(e.target);toast(error.message)})}if(e.target.id==='artifact-version')artifact(e.target.value).catch(e=>toast(e.message))});
document.addEventListener('click',async e=>{const b=e.target.closest('[data-action]');if(!b||suppressClick)return;try{switch(b.dataset.action){
case 'import-workflow':importWorkflowDialog();break;
case 'empty-workflow':emptyWorkflowDialog();break;
case 'manage-templates':await manageTemplates();break;
case 'create-template':confirmTemplateWorkflow(b.dataset.templateId);break;
case 'template-back':if(!creatingWorkflow)await showWorkflowCatalog();break;
case 'save-template':await createTemplateWorkflow(b.dataset.templateId);break;
case 'decision-history':reviewHistory();break;
case 'toggle-external-writes':{const enabled=b.getAttribute('aria-checked')!=='true';b.setAttribute('aria-checked',String(enabled));b.closest('.execution-policy-fields').querySelector('[name=allow_external_writes]').value=String(enabled);break;}
case 'save-execution-policy':{const f=$('#execution-policy-form');if(app.run||!f.reportValidity())break;app.wf.execution=readPolicy(f);dirty();closeModal();render();break;}
case 'twg-setup':twgSetup();break;
case 'twg-check':await twgCheck(b);break;
case 'save-inspector':if(!app.run){b.disabled=true;try{await saveWorkflow();toast('Workflow salvato')}finally{b.disabled=false}}break;
case 'close-dialog':closeModal();break;case 'close-inspector':app.selected=null;render();break;
case 'filter-steps':if(stepFilters.some(f=>f.id===b.dataset.filter)){app.stepFilter=b.dataset.filter;renderGraph();renderList()}break;
case 'select-node':setNodeSelection([b.dataset.id]);render();break;
case 'select-edge':selectGraphEdge(b.dataset.edgeKey);break;
case 'delete-link':deleteGraphEdge(b.dataset.edgeKey);break;
case 'add-node':newNode(b.dataset.kind);break;
case 'port-out':if(app.run)break;app.link=b.dataset.id;renderGraph();toast('Trascina la linea verso una porta di ingresso');break;
case 'port-in':if(app.link){const from=app.link;app.link=null;connect(from,b.dataset.id);renderGraph()}break;
case 'save-edge':{const f=Object.fromEntries(new FormData($('#edge-form')));if(addGraphEdge(f))closeModal();break;}
case 'delete-edge':{const edge=app.wf.edges[Number(b.dataset.index)];if(edge)deleteGraphEdge(graphEdgeKey(edge));break;}
case 'rag-node-manage':{const id=$('#node-form [name=rag_index]')?.value;const i=ragCatalog.find(i=>i.id===id);if(i)await openRag(i.root,i.path);else await openStorage();break;}
case 'browse-source':{if(app.run)break;const form=$('#node-form');const n=app.wf.nodes.find(n=>n.id===app.selected&&n.kind==='Source');if(n){const root=['example','examples'].includes(n.source_type)?'examples':'sources';const path=n.source_type==='example'?'helpdesk':n.location==='.'?'':n.source_type==='git'?'':n.location||'';await browseSourceStorage(path,{form,node:n,workflow:app.wf.id,root})}break;}
case 'continue-run-node':{
 const run=app.run,node=b.dataset.node;if(!run||!app.continueNodes.includes(node))break;
 closeNodeMenu();
 if(!await appConfirm({title:'Riprendi esecuzione',message:uiText('Riprendere da questo nodo? I passi successivi saranno rieseguiti sui file correnti. Risultati e approvazioni a valle saranno invalidati; i file non vengono ripristinati.'),accept:'Riprendi esecuzione'}))break;
 if(app.run?.id!==run.id)break;
 await api('/runs/'+encodeURIComponent(run.id)+'/continue',{node,acknowledge:true});await loadRun(run.id);break;
}
case 'clear-node-selection':setNodeSelection([]);render();break;case 'delete-selected-nodes':deleteSelectedNodes();break;case 'edit-run-workflow':if(app.run)selectWorkflow(app.run.workflow.id);break;case 'delete-node':deleteNode(b.dataset.id);break;
case 'set-entry':app.wf.entry=b.dataset.id;dirty();toast('Ingresso impostato');break;
case 'create-workflow':{leaveDraftContext();app.editBase=null;const name=$('[name="workflow_name"]').value.trim();if(!name)throw Error('Nome obbligatorio');disconnect();app.run=null;app.wf={id:crypto.randomUUID(),name,description:'',entry:'',nodes:[],edges:[]};app.selected=null;await saveWorkflow();render();closeModal();break;}
case 'clear-key':{const field=$(`[name="${b.dataset.field}"]`);field.value='__CLEAR__';field.removeAttribute('placeholder')}
toast('La credenziale sarà rimossa al salvataggio');break;case 'password-dialog':await settingsDialog('general');break;case 'change-password':await changeAccountPassword();break;case 'compact-session':await compactSession();break;case 'settings':await settingsDialog();break;case 'save-settings':await saveSettings();break;
case 'import-host':await api('/auth/import-host',{runtime:b.dataset.runtime});toast('Login locale importato solo per questo account');await settingsDialog();break;
case 'environment':await showEnvironment();break;
case 'capabilities':await showCliCapabilities();break;
case 'admin-user-suspend':case 'admin-user-activate':case 'admin-user-remove':await changeAdminUser(b);break;case 'admin-users-refresh':await loadAdminUsers();break;case 'add-user':await createAccountUser();break;
case 'logout':closeAccountMenu();app.editBase=null;await api('/auth/logout',{});app.run=null;showAuth();break;
case 'start-run':{if(!$('#run-form').reportValidity())break;b.disabled=true;const f=Object.fromEntries(new FormData($('#run-form')));let index=0,allowLargeCopy=false;for(const n of app.wf.nodes.filter(n=>n.kind==='Source')){if(n.source_type==='folder'&&(n.source_mode||'copy')!=='direct'&&!(index===0&&f.base_run)){if(!await confirmSourceCopy(n,()=>{allowLargeCopy=true})){b.disabled=false;return}}index++}const r=await api('/runs',{workflow_id:app.wf.id,input:f.input,base_run:f.base_run||null,allow_large_source_copy:allowLargeCopy,execution:readPolicy($('#run-form'))});closeModal();await loadRun(r.id);logs(true);break;}
case 'load-run':closeModal();await loadRun(b.dataset.id);break;
case 'child-runs':showRunFamily();break;
case 'save-note':await api('/runs/'+app.run.id+'/note',{text:$('#run-note').value});closeModal();await refreshRun();toast('Chiarimento registrato. Riprendi quando sei pronto.');break;case 'pending':pending();break;
case 'decide':{const answer=$('#decision-answer').value;if(b.dataset.value==='answer'&&!answer.trim()){ $('#decision-error').textContent='Inserisci una risposta';break}await api('/runs/'+app.run.id+'/decide',{action:b.dataset.value,answer});closeModal();await refreshRun();break;}
case 'artifact':await artifact(b.dataset.id);break;case 'artifact-mode':await artifact(b.dataset.id,b.dataset.mode);break;
case 'review-history':reviewHistory(b.dataset.id);break;
}}catch(err){toast(err.message);if(b.dataset.action==='start-run')b.disabled=false}});
let tip;
function hideTooltip(){tip?.remove();tip=null}
document.addEventListener('pointerover',e=>{const el=e.target.closest('[data-tip]');if(!el)return;hideTooltip();tip=document.createElement('div');tip.className='tooltip';tip.textContent=el.dataset.tip;document.body.append(tip);const r=el.getBoundingClientRect(),t=tip.getBoundingClientRect();tip.style.left=Math.max(6,Math.min(innerWidth-t.width-6,r.left+r.width/2-t.width/2))+'px';tip.style.top=(r.bottom+8+t.height<innerHeight?r.bottom+8:r.top-t.height-8)+'px'});document.addEventListener('pointerout',e=>{if(e.target.closest('[data-tip]')){hideTooltip()}});
document.addEventListener('keydown',e=>{if(e.target.closest('.app-confirm'))return;if(e.key==='Escape'&&$('#dialog').open)return;if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='z'&&!e.shiftKey&&!e.altKey&&!e.defaultPrevented&&!app.run&&app.view!=='history'&&!$('#app').hidden&&!$('#dialog').open&&!e.target.closest('input,textarea,jui-select,jui-switch,[contenteditable]:not([contenteditable="false"]),[role="textbox"]')){e.preventDefault();if(!e.repeat)undoNodeDeletion();return}if(['Backspace','Delete'].includes(e.key)&&!e.defaultPrevented&&!e.repeat&&!e.metaKey&&!e.ctrlKey&&!e.altKey&&!app.run&&(app.selected||app.selectedEdge)&&app.view!=='history'&&!$('#app').hidden&&!$('#dialog').open&&!e.target.closest('input,textarea,jui-select,jui-switch,[contenteditable]:not([contenteditable="false"]),[role="textbox"]')){e.preventDefault();if(app.selectedEdge)deleteGraphEdge(app.selectedEdge);else if(selectedNodeIds().length>1)deleteSelectedNodes();else deleteNode(app.selected)}if(e.key==='Escape'){cancelGraphGesture();setNodeSelection([]);renderInspector();cancelPaletteDrag();app.selectedEdge=null;cancelEdgeDrag();closeNodeMenu();app.link=null;hideTooltip();renderGraph()}if((e.metaKey||e.ctrlKey)&&e.key==='s'){e.preventDefault();if(!app.run)saveWorkflow().then(()=>toast('Salvato')).catch(e=>toast(e.message))}});
document.addEventListener('focusin',e=>{const hit=e.target.closest('.edge-hit');if(hit&&app.selectedEdge!==hit.dataset.edgeKey)selectGraphEdge(hit.dataset.edgeKey,false)});
document.addEventListener('keydown',e=>{const hit=e.target.closest('.edge-hit');if(hit&&(e.key==='Enter'||e.key===' ')){e.preventDefault();selectGraphEdge(hit.dataset.edgeKey)}});
window.addEventListener('beforeunload',e=>{if(app.dirty){e.preventDefault();e.returnValue=''}});
init().catch(e=>toast(e.message));

let storagePath='',storageBusy=false;
const storageJoin=(a,b)=>[a,b].filter(Boolean).join('/');
let storageShowHidden=false;
try{storageShowHidden=localStorage.getItem('nodo.storage.showHidden')==='true'}catch{}
function storageHiddenAttrs(entry){return `data-hidden-entry="${entry.name.startsWith('.')}" ${!storageShowHidden&&entry.name.startsWith('.')?'hidden':''}`}
function applyStorageHidden(){
 $$('[data-storage-show-hidden]').forEach(button=>button.setAttribute('aria-checked',String(storageShowHidden)));
 $$('.storage-table').forEach(table=>{const rows=$$('[data-hidden-entry]',table);rows.forEach(row=>row.hidden=!storageShowHidden&&row.dataset.hiddenEntry==='true');const empty=$('[data-storage-empty]',table);if(empty)empty.hidden=rows.some(row=>!row.hidden)});
}
function bindStorageHidden(){
 applyStorageHidden();
 $$('[data-storage-show-hidden]').forEach(button=>button.onclick=()=>{storageShowHidden=!storageShowHidden;try{localStorage.setItem('nodo.storage.showHidden',String(storageShowHidden))}catch{}applyStorageHidden()});
}
window.addEventListener('storage',e=>{if(e.key==='nodo.storage.showHidden'){storageShowHidden=e.newValue==='true';applyStorageHidden()}});
function storageHiddenControl(){return `<div class="storage-hidden-control"><span id="storage-hidden-label">Mostra nascosti</span><button type="button" role="switch" class="storage-hidden-toggle" data-storage-show-hidden aria-checked="${storageShowHidden}" aria-labelledby="storage-hidden-label"><span aria-hidden="true"></span></button></div>`}
function storageRoots(root,includeRuns=false,actions=storageHiddenControl()){return `<div class="storage-topline"><div class="tabs storage-roots" role="group" aria-label="Archivio">${(includeRuns?['sources','examples','workspaces']:['sources','examples']).map(name=>`<button type="button" data-storage-root="${name}" aria-pressed="${root===name}" class="${root===name?'active':''}">${icon('folder-git-2')}${name==='workspaces'?'Esecuzioni':name}</button>`).join('')}</div>${actions}</div>`}
async function openStorage(path='',root='sources'){
 try{await ragRefresh();const d=await api('/storage?path='+encodeURIComponent(path)+'&root='+encodeURIComponent(root));storagePath=path;
 const parts=path.split('/').filter(Boolean);const crumbs=[['',root==='workspaces'?uiText('Esecuzioni'):root],...parts.map((p,i)=>[parts.slice(0,i+1).join('/'),p])];
 const ragAction=(root!=='workspaces'||path)&&!path.split('/').includes('.nodo')?`<div class="rag-folder-action">${ragButton(root,path,uiText('Indice RAG'))}</div>`:'';
 const sourceNote=root==='sources'?'<p class="note storage-source-note">Le Source in modalità permanente vengono modificate direttamente. Le altre usano copie nel workspace.</p>':'';
 const toolbar=storageRoots(root,true,ragAction)+`<nav class="storage-breadcrumb" aria-label="Percorso">${crumbs.map(([p,n])=>`<button data-storage-path="${esc(p)}">${esc(n)}</button>`).join('<span>/</span>')}</nav>`+(d.file?sourceNote:'');
 const resultDownload=root==='workspaces'&&parts.length?`<a class="storage-picker" href="/api/storage/download?root=workspaces&path=${encodeURIComponent(parts[0])}" download>${icon('download')}Scarica risultato ZIP</a>`:'';
 const controls=root==='workspaces'?'<p class="note">File delle tue esecuzioni. In modalità permanente mostrano il contenuto attuale condiviso della sorgente. Consultazione e download; build e cartelle Git esclusi dallo ZIP.</p>':root==='examples'?'<p class="note">Examples precaricati: navigazione e download disponibili, archivio di sola lettura.</p>':`<div class="storage-actions"><label class="storage-picker">${icon('file-text')}Carica file<input id="storage-files" type="file" multiple hidden></label><label class="storage-picker">${icon('folder-git-2')}Carica cartella<input id="storage-folder" type="file" webkitdirectory multiple hidden></label><span class="small">Massimo 50 MB per file · nomi esistenti preservati</span></div>`;
 modal('Storage',toolbar+resultDownload+(d.file?`<div class="storage-file-head"><span>${icon('file-text')}${esc(parts.at(-1))}</span><a class="storage-picker" href="/api/storage/download?path=${encodeURIComponent(path)}&root=${root}" download>Scarica ZIP</a></div>${d.preview===null?`<p class="note">${esc(filePreviewMessage(d))}</p>`:d.html?`<div class="markdown storage-markdown">${d.html}</div>`:`<pre class="source-view storage-preview">${esc(d.preview)}</pre>`}`:controls+(root!=='sources'?'':`<div id="storage-drop" class="storage-drop" tabindex="0"><span>${icon('folder-git-2')}Trascina qui file o cartelle per caricarli in /sources/${esc(path)}</span><div id="storage-progress" role="status" aria-live="polite"></div></div>`)+`<div class="storage-list-controls"><div class="storage-list-actions">${storageHiddenControl()}<button type="button" class="storage-refresh" data-storage-refresh>${icon('refresh-cw')}Aggiorna</button></div>${sourceNote}</div><div class="storage-table">${d.entries.map(e=>{const p=storageJoin(path,e.name);return `<div class="storage-row" ${storageHiddenAttrs(e)}><button class="storage-entry" data-storage-path="${esc(p)}">${icon(e.directory?'folder-git-2':'file-text')}<span>${esc(e.label||e.name)}</span></button><span class="small">${e.status?esc(states[e.status]||e.status)+(e.source_mode==='direct'?' · '+uiText('Sorgente originale — contenuto attuale condiviso'):''):e.directory?'Cartella':storageSize(e.size)}</span>${e.name!=='.nodo'?ragButton(root,p):''}<a class="icon-button" data-tip="Scarica ZIP" aria-label="Scarica ZIP di ${esc(e.name)}" href="/api/storage/download?path=${encodeURIComponent(p)}&root=${root}" download>${icon('download')}</a></div>`}).join('')}<p class="note storage-empty" data-storage-empty>Nessun elemento visibile.</p></div>`),'',true);
 bindStorageHidden();bindRagStorage();
 const refresh=$('[data-storage-refresh]');if(refresh)refresh.onclick=async()=>{
  if(storageBusy)return;refresh.disabled=true;
  const scroll=$('#dialog .modal-body').scrollTop;
  try{await openStorage(path,root);$('#dialog .modal-body').scrollTop=scroll}finally{if(refresh.isConnected)refresh.disabled=false}
 };
 $$('[data-storage-path]').forEach(b=>b.onclick=()=>{if(!storageBusy)openStorage(b.dataset.storagePath,root)});
 $$('[data-storage-root]',$('#dialog')).forEach(b=>b.onclick=()=>{if(!storageBusy)openStorage('',b.dataset.storageRoot)});
 for(const id of ['storage-files','storage-folder']){const input=$('#'+id);if(input)input.onchange=()=>uploadStorage([...input.files].map(file=>({file,path:file.webkitRelativePath||file.name})),path)}
 const drop=$('#storage-drop');if(drop){drop.ondragover=e=>{e.preventDefault();drop.classList.add('dragover')};drop.ondragleave=()=>drop.classList.remove('dragover');drop.ondrop=async e=>{e.preventDefault();drop.classList.remove('dragover');if(storageBusy)return;const entries=[...e.dataTransfer.items].map(i=>i.webkitGetAsEntry?.()).filter(Boolean);try{const files=[];for(const entry of entries)await collectStorage(entry,'',files);if(!entries.length)files.push(...[...e.dataTransfer.files].map(file=>({file,path:file.name})));await uploadStorage(files,path)}catch(err){toast(err.message)}}}
 }catch(e){toast(e.message)}
}
function storageSize(n){return n<1024?n+' B':n<1048576?(n/1024).toFixed(1)+' KB':(n/1048576).toFixed(1)+' MB'}
async function collectStorage(entry,parent,out){if(out.length>=10000)throw Error('Massimo 10000 file per caricamento');const path=storageJoin(parent,entry.name);if(entry.isFile){const file=await new Promise((ok,no)=>entry.file(ok,no));out.push({file,path})}else if(entry.isDirectory){out.push({directory:true,path});const reader=entry.createReader();for(;;){const batch=await new Promise((ok,no)=>reader.readEntries(ok,no));if(!batch.length)break;for(const child of batch)await collectStorage(child,path,out)}}}
async function uploadStorage(files,folder){if(storageBusy)return;storageBusy=true;let ok=0;const errors=[];const progress=$('#storage-progress');try{for(const [i,item] of files.entries()){if(progress)progress.textContent=`Caricamento ${i+1}/${files.length} · ${item.path}`;try{if(!item.directory&&item.file.size>50*1024*1024)throw Error('Supera 50 MB');const res=await fetch('/api/storage/upload?path='+encodeURIComponent(storageJoin(folder,item.path))+(item.directory?'&directory=true':''),{method:'POST',headers:{'Content-Type':'application/octet-stream','X-Nodo-Request':'1'},credentials:'same-origin',body:item.directory?'':item.file});if(!res.ok){const d=await res.json();throw Error(d.error||'Caricamento fallito')}ok++}catch(e){errors.push(item.path+': '+e.message)}}}finally{storageBusy=false}if($('#dialog').open&&$('#storage-drop'))await openStorage(folder);toast(`${ok}/${files.length} elementi caricati${errors.length?' · '+errors.length+' errori':''}`);if(errors.length&&$('#storage-drop')){const report=document.createElement('pre');report.className='storage-errors';report.textContent=errors.join('\n');$('#storage-drop').append(report)}}
$('#storage').onclick=()=>openStorage();

async function browseSourceStorage(path,context,root=context.root||'sources'){
 try{
  const data=await api('/storage?path='+encodeURIComponent(path)+'&root='+encodeURIComponent(root));
  if(data.file)throw Error(uiText('Seleziona una cartella, non un file.'));
  if(!context.form.isConnected||app.run||app.wf.id!==context.workflow)return;
  const parts=path.split('/').filter(Boolean),crumbs=[['',root],...parts.map((p,i)=>[parts.slice(0,i+1).join('/'),p])];
  const folders=data.entries.filter(e=>e.directory);
  modal('Seleziona cartella repository',storageRoots(root)+`<p class="note">Apri una cartella e premi Seleziona cartella per usarla come Source.</p><nav class="storage-breadcrumb" aria-label="Percorso">${crumbs.map(([p,name])=>`<button type="button" data-source-folder="${esc(p)}">${esc(name)}</button>`).join('<span>/</span>')}</nav><div class="source-folder-current" data-no-i18n>/${root}/${esc(path)}</div><div class="storage-table">${folders.map(e=>`<div class="storage-row source-folder-row" ${storageHiddenAttrs(e)}><button type="button" class="storage-entry" data-source-folder="${esc(storageJoin(path,e.name))}">${icon('folder-git-2')}<span>${esc(e.name)}</span></button></div>`).join('')}<p class="note storage-empty" data-storage-empty>Nessuna sottocartella. Puoi selezionare la cartella corrente.</p></div>`,`<button type="button" data-source-cancel>Annulla</button><button type="button" class="primary" data-source-select>${icon('folder-git-2')}Seleziona cartella</button>`,true);
  bindStorageHidden();bindRagStorage();
  $$('[data-source-folder]',$('#dialog')).forEach(button=>button.onclick=()=>browseSourceStorage(button.dataset.sourceFolder,context,root));
  $$('[data-storage-root]',$('#dialog')).forEach(b=>b.onclick=()=>browseSourceStorage('',context,b.dataset.storageRoot));
  $('[data-source-cancel]').onclick=closeModal;
  $('[data-source-select]').onclick=async()=>{
   if(!context.form.isConnected||app.run||app.wf.id!==context.workflow||app.selected!==context.node.id)return;
   const location=path||'.',type=root==='examples'?'examples':'folder';context.form.elements.source_type.value=type;context.form.elements.location.value=location;
   context.node.source_type=type;context.node.location=location;context.node.source_mode=context.form.elements.source_mode.value;dirty();closeModal();renderInspector();if(type==='folder'&&(context.node.source_mode||'copy')==='copy'){try{if(!await confirmSourceCopy(context.node)){context.node.source_mode='direct';dirty();renderInspector()}}catch(error){context.node.source_mode='direct';dirty();renderInspector();toast(error.message)}}toast('Cartella selezionata. Salvataggio automatico.');
  };
 }catch(e){if(path){toast(e.message);await browseSourceStorage('',context,root)}else toast(e.message)}
}

async function deleteWorkflowById(wid){
 const workflow=app.workflows.find(w=>w.id===wid);if(!workflow)return;
 try{
  const summary=await api('/workflows/'+encodeURIComponent(wid)+'/deletion');
  if(summary.active){toast('Metti in pausa le esecuzioni attive e attendi la risposta del project manager.');return}
  const message=uiText('Vuoi eliminare questo workflow?')+'\n'+workflow.name+'\n\n'+uiText('Esecuzioni collegate:')+' '+summary.runs+'\n'+uiText('Artefatti e versioni:')+' '+summary.artifacts+'\n'+uiText('Conversazioni manager:')+' '+(summary.manager_chats||0)+'\n\n'+uiText('Verranno eliminati il workflow, tutte le sue esecuzioni, i workspace, gli artefatti, i log e le sessioni locali associate. Questa operazione non può essere annullata.')+'\n\n'+uiText('I file e le cartelle in /sources vengono sempre conservati.');
  if(!(await appConfirm({title:'Elimina workflow',message,accept:'Elimina definitivamente',danger:true})))return;
  const state=draftSaves.get(draftPrefix()+wid);if(state){state.discarded=true;clearTimeout(state.timer);if(state.promise)await state.promise.catch(()=>{})}
  let result;try{result=await api('/workflows/'+encodeURIComponent(wid),{acknowledge:true,run_ids:summary.run_ids},'DELETE')}catch(error){if(state){state.discarded=false;if(state.version>state.saved)scheduleDraft(state)}throw error}
  if(state){draftSaves.delete(state.key);try{localStorage.removeItem(state.key)}catch{}}
  const wasCurrent=currentWorkflow()?.id===wid;app.workflows=await api('/workflows');await loadWorkflowTopics();await ragRefresh();app.runs=await api('/runs');if(wasCurrent){disconnect();app.run=null;app.view='graph';selectWorkflow(workflowOrganization.recent[0]||app.workflows[0]?.id)}else workflowOptions();
  toast(result.cleanup_pending?'Workflow eliminato. Pulizia dei file residui da completare.':'Workflow ed esecuzioni eliminati');return true;
 }catch(e){toast(e.message)}
}
$('#delete-workflow').onclick=()=>deleteWorkflowById(currentWorkflow().id);

$('#run-family').onclick=showRunFamily;
$('#run-storage').onclick=()=>{if(app.run)openStorage(app.run.id,'workspaces')};

$('#rename-workflow').onclick=()=>{
 const id=currentWorkflow().id,w=app.workflows.find(w=>w.id===id);if(!w)return;
 modal('Rinomina workflow',`<form id="rename-workflow-form"><label>Nome workflow<input name="workflow_name" type="text" required maxlength="160" value="${esc(w.name)}" autocomplete="off"></label><p id="rename-workflow-error" class="error" role="alert"></p></form>`,'<button type="button" data-action="close-dialog">Annulla</button><button type="submit" form="rename-workflow-form" id="save-workflow-name" class="primary">Salva nome</button>');
 const form=$('#rename-workflow-form'),field=$('[name="workflow_name"]',form),button=$('#save-workflow-name');let pending=false;field.focus();field.select();
 form.addEventListener('submit',async e=>{
  e.preventDefault();if(pending)return;pending=true;button.disabled=true;$('#rename-workflow-error').textContent='';
  try{if(app.dirty&&!app.run)await saveWorkflow();const updated=await api('/workflows/'+encodeURIComponent(id),{name:field.value},'PATCH');const index=app.workflows.findIndex(w=>w.id===id);if(index>=0)app.workflows[index]=updated;if(app.wf?.id===id){app.wf.name=updated.name;const state=draftSaves.get(draftPrefix()+id);if(state?.snapshot){state.snapshot.name=updated.name;if(state.editBase||state.version>state.saved)journalDraft(state)}}workflowOptions(currentWorkflow()?.id);if($('#rename-workflow-form')===form)closeModal();toast('Workflow rinominato')}
  catch(error){if($('#rename-workflow-form')===form){$('#rename-workflow-error').textContent=error.message;field.focus()}}
  finally{pending=false;button.disabled=false}
 });
};

document.addEventListener('change',event=>{if(event.target.id==='compact-run')loadCompactBindings()});

window.addEventListener('nodo-language-changed',()=>{if(app.user)renderActivityLog()});

// Keep resize grips outside panel contents, which are replaced on render.
(()=>{
 const container=$('#right-panels');
 const panels=[['inspector',330,320],['manager-panel',380,350]].map(([id,desktop,compact])=>{
  const panel=document.getElementById(id),grip=document.createElement('div');
  grip.className='right-panel-resizer';grip.tabIndex=0;
  grip.setAttribute('role','separator');grip.setAttribute('aria-orientation','vertical');
  grip.setAttribute('aria-controls',id);
  grip.setAttribute('aria-label',id==='inspector'?'Ridimensiona inspector':'Ridimensiona chat');
  panel.before(grip);
  let preferred=null;
  const bounds=()=>{const base=innerWidth<=1000?compact:desktop,min=Math.min(base,innerWidth<=550?innerWidth-28:base);return {min,max:Math.max(min,Math.floor(innerWidth/2))}};
  const apply=width=>{
   const {min,max}=bounds(),value=Math.max(min,Math.min(max,width??min));
   panel.style.setProperty('--right-panel-width',value+'px');
   grip.setAttribute('aria-valuemin',min);grip.setAttribute('aria-valuemax',max);grip.setAttribute('aria-valuenow',value);
   grip.hidden=panel.hidden;
  };
  const resize=width=>{preferred=width;apply(width)};
  grip.addEventListener('pointerdown',event=>{
   if(event.button!==0)return;
   event.preventDefault();grip.focus();
   const start=event.clientX,width=panel.getBoundingClientRect().width;
   grip.setPointerCapture(event.pointerId);document.body.classList.add('resizing-right-panel');
   const move=e=>resize(width+start-e.clientX);
   const end=()=>{grip.removeEventListener('pointermove',move);grip.removeEventListener('lostpointercapture',end);document.body.classList.remove('resizing-right-panel')};
   grip.addEventListener('pointermove',move);grip.addEventListener('lostpointercapture',end);
  });
  grip.addEventListener('keydown',event=>{
   const {min,max}=bounds(),width=panel.getBoundingClientRect().width,step=event.shiftKey?50:10;
   const next={ArrowLeft:width+step,ArrowRight:width-step,Home:min,End:max}[event.key];
   if(next===undefined)return;event.preventDefault();resize(next);
  });
  grip.addEventListener('dblclick',()=>{preferred=null;apply(null)});
  new MutationObserver(()=>apply(preferred)).observe(panel,{attributes:true,attributeFilter:['hidden']});
  apply(preferred);return ()=>apply(preferred);
 });
 window.addEventListener('resize',()=>panels.forEach(update=>update()));
})();
