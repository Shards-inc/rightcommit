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
  // The topbar reserves the prefix width in CSS, including while the app is hidden.
  const fixedPrefix=!!brand.closest('.topbar');
  // The slot is as wide as "right", so the wordmark has no gap; longer words overflow to the left.
  const fit=()=>{if(fixedPrefix)return;const current=reel.firstElementChild;if(reel.children.length!==1||current?.textContent!=='right')return;const range=document.createRange();range.selectNodeContents(current);const width=range.getBoundingClientRect().width;if(width)slot.style.width=width/parseFloat(getComputedStyle(slot).fontSize)+.05+'em'};
  new ResizeObserver(fit).observe(brand);document.fonts?.ready.then(fit);
  fit();
  return {brand,reel,fit,lastRun:null,running:false};
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
   const soften=logo.brand.closest('#auth')&&!reducedMotion.matches&&!document.hidden&&logo.brand.getClientRects().length;
   let fade=null;
   try{
    if(soften){
     fade=logo.brand.animate([{opacity:1},{opacity:0}],{duration:180,easing:'ease-out',fill:'forwards'});
     await fade.finished;
    }
    const row=document.createElement('b');row.textContent='right';logo.reel.replaceChildren(row);
    logo.reel.style.transform='';logo.fit();
    if(soften){
     // Keep the whole logo invisible while its final width is measured and centered.
     const fadeOut=fade;
     fade=logo.brand.animate([
      {opacity:0,transform:'translateX(-32px)',offset:0,easing:'cubic-bezier(.2,.7,.3,1)'},
      {opacity:1,transform:'translateX(12px)',offset:.44,easing:'ease-in-out'},
      {opacity:1,transform:'translateX(-6px)',offset:.66,easing:'ease-in-out'},
      {opacity:1,transform:'translateX(3px)',offset:.82,easing:'ease-in-out'},
      {opacity:1,transform:'translateX(-1px)',offset:.93,easing:'ease-in-out'},
      {opacity:1,transform:'translateX(0)',offset:1}
     ],{duration:900,easing:'linear',fill:'forwards'});
     fadeOut.cancel();
     await fade.finished;
    }
   }finally{
    fade?.cancel();logo.running=false;
   }
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
const states={starting:'In attesa di risorse',queued:'In coda',running:'In esecuzione',completed:'Completato',paused:'In pausa',waiting_user:'Attende risposta',waiting_review:'Attende review',waiting_external:'Servizio in corso',waiting_child:'Workflow figlio in corso',failed:'Errore',interrupted:'Interrotto',bypassed:'Saltato',skipped:'Non percorso',rework:'Rework'};
const app={user:null,settings:null,workflows:[],wf:null,run:null,editBase:null,continueNodes:[],artifacts:[],decisions:[],children:[],reviewRun:null,runs:[],selected:null,selectedNodes:[],selectedEdge:null,view:'graph',stepFilter:'all',stepScope:null,dirty:false,zoom:.85,pan:{x:30,y:40},link:null,events:[],stream:null,lastEvent:0,refresh:null};
let edgeDrag=null;
let latestRun=null,latestRunWorkflow='',latestRunLoading=false,latestRunRequest=0,openingLatestRun=false;
let wheelZoomEnabled=true;
try{wheelZoomEnabled=localStorage.getItem('nodo.wheelZoom')!=='false'}catch{}
function icon(name,cls=''){return `<svg class="icon ${cls}" aria-hidden="true"><use href="/icons.svg#${name}"></use></svg>`}
function nodeService(n){const id=n.kind==='Agent'?(n.runtime||app.settings?.default_runtime):n.kind==='Connection'?n.provider:null;return Object.hasOwn(serviceInfo,id)?id:null}
function serviceIdentity(service){const info=serviceInfo[service];if(!info)return '';const picture=icon(info.icon,`service-icon service-icon-${service} ${info.icon.startsWith('brand-')?'brand-icon':''}`);return info.text?`<span class="service-identity" data-service-identity="${service}" data-no-i18n>${picture}<span class="service-identity-text">${esc(info.text)}</span></span>`:picture}
function piProviderLabel(n){
 const config=app.settings?.agents?.pi||{};
 // Runs retain the provider chosen at launch; drafts follow the active configuration.
 const provider=(app.run||config.multi_provider)?n.pi_provider||config.provider:config.provider||n.pi_provider;
 return (app.settings?.pi_providers||[]).find(item=>item.id===provider)?.label||provider||'';
}
function nodeIcon(n,showProvider=false){
 const service=nodeService(n);
 if(service==='pi'&&showProvider){const label=piProviderLabel(n);if(label)return `<span class="service-identity" data-service-identity="pi" data-no-i18n>${serviceIdentity(service)}<span class="service-identity-text">${esc(label)}</span></span>`}
 return service?serviceIdentity(service):icon(icons[n.kind]);
}
function nodeLabel(n){const service=nodeService(n);return service?serviceInfo[service].name:n.kind}
function associatedAgent(n){return ['Task','Review','Gate'].includes(n.kind)?currentWorkflow().nodes.find(a=>a.id===n.agent&&a.kind==='Agent'):null}
function associatedAgentBadge(agent){return agent?`<span class="node-agent-badge" data-agent="${esc(agent.id)}" data-tip="${esc(agent.title)} · ${esc(nodeLabel(agent))}" aria-hidden="true">${nodeIcon(agent)}</span>`:''}
function hydrate(root=document){$$('[data-icon]',root).forEach(e=>{e.innerHTML=icon(e.dataset.icon);e.removeAttribute('data-icon')})}
async function api(path,body,method){const res=await fetch('/api'+path,{method:method||(body===undefined?'GET':'POST'),headers:{'Content-Type':'application/json','X-Nodo-Request':'1'},credentials:'same-origin',...(body!==undefined?{body:JSON.stringify(body)}:{})});let data;try{data=await res.json()}catch{throw Error('Risposta server non valida')}if(!res.ok){if(res.status===401&&path!='/auth/login')showAuth();throw Error(data.error||`HTTP ${res.status}`)}return data}
let toastTimer;function toast(s){const el=$('#toast');el.textContent=s;el.hidden=false;if(el.showPopover){if(el.matches(':popover-open'))el.hidePopover();el.showPopover()}clearTimeout(toastTimer);toastTimer=setTimeout(()=>{if(el.matches(':popover-open'))el.hidePopover();el.hidden=true},6500)}
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
function resetAuthForm(){
 const form=$('#auth-form');form.reset();
 for(const name of ['email','password'])form.elements[name].value='';
 $('#auth-error').textContent='';
 form.elements.email.autocomplete='section-rightcommit-account username';
 form.elements.password.autocomplete='section-rightcommit-account '+(registerMode?'new-password':'current-password');
}
function updateAuthMode(){
 $('#auth-title').textContent=uiText(registerMode?(authInfo.setup_required?'Crea il tuo workspace':'Crea un account'):'Accedi al workspace');
 $('#auth-submit').textContent=uiText(registerMode?(authInfo.setup_required?'Crea amministratore':'Registrati'):'Accedi');
 $('#auth-note').textContent=uiText(authInfo.setup_required?'Il primo account può creare gli altri utenti.':'Ogni account ha workflow,\nsessioni e credenziali separati.');
 $('#auth-switch').textContent=uiText(registerMode?'Torna al login':'Crea un account');
 $('#auth-switch').hidden=!authInfo.registration_open||authInfo.setup_required;
 resetAuthForm();
}
function authCredentials(form){
 if(!form.reportValidity())return null;
 return {email:form.elements.email.value,password:form.elements.password.value};
}
function showAuth(){closeAccountMenu();window.resetTemplateManager?.();app.stream?.close();$('#dialog').close();$$('dialog.app-confirm').forEach(dialog=>dialog.close());$('#app').hidden=true;$('#auth').hidden=false;resetAuthForm()}
let registerMode=false,authInfo={};async function init(){hydrate();try{authInfo=await api('/auth/status');registerMode=authInfo.setup_required;updateAuthMode();app.user=await api('/auth/me');await boot()}catch(e){showAuth();if(e.message!=='Accedi per continuare')$('#auth-error').textContent=''}}
async function boot(){resetActivityLogs();latestRun=null;latestRunWorkflow='';++latestRunRequest;app.user=await api('/auth/me');app.settings=await api('/settings');app.workflows=await api('/workflows');await loadWorkflowTopics();await ragRefresh();await a2aRefresh();app.runs=await api('/runs');await refreshSchedules();$('#auth').hidden=true;$('#app').hidden=false;$('#account').textContent=app.user.email[0].toUpperCase();$('#account').dataset.tip=app.user.email;workflowOptions();selectWorkflow(pendingDraftId()||workflowOrganization.recent[0]);showAgentSetupNotice();void refreshActiveWorkflowCount();}
const agentSetupNoticeStorageKey='rightcommit.hideAgentSetupNotice';
function agentSetupNoticeHidden(){try{return localStorage.getItem(agentSetupNoticeStorageKey)==='true'}catch{return false}}
function setAgentSetupNoticeHidden(hidden){try{localStorage.setItem(agentSetupNoticeStorageKey,String(hidden));return true}catch{toast('Impossibile salvare la preferenza in questo browser.');return false}}
function showAgentSetupNotice(){
 if(agentSetupNoticeHidden())return;
 if(['codex','claude','pi'].some(runtime=>app.settings.agents?.[runtime]?.configured))return;
 modal('Nessun agente configurato',`<div class="agent-setup-notice" data-agent-setup-notice><span class="agent-setup-icon" aria-hidden="true">${icon('bot-off')}</span><p>Non ci sono agenti configurati. Apri le configurazioni personali per configurare un agente e iniziare a eseguire i workflow.</p><label class="agent-setup-dismiss"><span>Non visualizzare più</span><jui-switch data-agent-setup-dismiss aria-label="Non visualizzare più"></jui-switch></label></div>`,'<button type="button" data-action="close-dialog">Chiudi</button><button type="button" class="primary" data-action="configure-agents" autofocus>Apri configurazioni personali</button>');
 const preference=$('[data-agent-setup-dismiss]');
 preference.addEventListener('change',()=>{if(!setAgentSetupNoticeHidden(preference.checked))preference.checked=agentSetupNoticeHidden()});
}
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
 modal('Elenco workflow',`<div class="workflow-list"><div class="workflow-list-toolbar"><label class="workflow-search">Cerca workflow<input type="search" id="workflow-search" placeholder="Cerca per nome…" autocomplete="off"></label><button type="button" id="manage-topics-toggle" aria-expanded="false" aria-controls="workflow-topic-settings">${icon('settings-2')}Gestisci topic</button></div><section id="workflow-topic-settings" hidden><form id="create-topic-form"><label>Nuovo topic<input id="new-topic-name" required maxlength="80" autocomplete="off"></label><button type="submit">${icon('plus')}Crea topic</button></form><div id="topic-management-rows"></div><p class="small">I primi cinque topic sono visibili nel selettore. Il topic dell’ultimo workflow usato viene mostrato per primo automaticamente.</p></section><nav id="workflow-topic-filters" aria-label="Filtra per topic"></nav><div class="workflow-bulk" hidden><span id="workflow-selection-count" role="status"></span><div class="workflow-move-destination"><label for="workflow-topic-destination">Sposta in</label><jui-select id="workflow-topic-destination" aria-label="Sposta in"></jui-select></div><div class="workflow-bulk-actions"><button type="button" id="move-topic-workflows">${icon('move-right')}<span>Sposta</span></button><button type="button" id="clear-workflow-selection">${icon('broom')}<span>Deseleziona</span></button></div></div><div class="workflow-list-heading"><jui-switch id="workflow-select-all" aria-label="Seleziona tutti i risultati"></jui-switch><span class="small" id="workflow-result-count" role="status" aria-live="polite"></span></div><div class="workflow-picker-table"><table><tbody id="workflow-picker-rows"></tbody></table></div><p class="note" id="workflow-picker-empty" hidden>Nessun workflow trovato.</p></div>`,'<button type="button" data-action="close-dialog">Chiudi</button>',true);
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
 workflowTemplates=catalog.templates.map(t=>({...t,setup:catalogText(t).setup,workflow:catalogWorkflow(t)}));
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
 $('#workspace-mode').textContent=app.run?uiText('Esecuzione in corso')+' · '+app.run.id.slice(0,8):uiText('Progettazione in corso');
 context.title=app.run?app.run.id:app.editBase?.id||'';
 if(app.editBase)$('#workspace-mode').textContent=uiText('Progettazione in corso')+' · '+app.editBase.id.slice(0,8);
 $('#edit-continue').title=app.workspaceSuccessor?uiText('Continua dall’ultima esecuzione del progetto; questa run è nella cronologia'):'';$('#edit-continue').hidden=!app.run;$('#edit-continue').disabled=!!app.run?.parent||!!app.workspaceSuccessor||editingRun;$('#revision-preview').hidden=!app.editBase;$('#revision-cancel').hidden=!app.editBase;$('#execute').innerHTML=icon(app.editBase?'git-compare-arrows':'play')+esc(uiText(app.editBase?'Anteprima ripartenza':'Esegui workflow'));
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
function revisionFileList(title,count,kind){return `<details data-revision-files="${kind}"><summary>${uiText(title)} (${count})</summary><ul class="revision-files"></ul><button type="button" data-revision-more hidden>${uiText('Mostra altri file')}</button></details>`}
function bindRevisionFileLists(plan){
 for(const details of document.querySelectorAll('[data-revision-files]')){
  const artifacts=details.dataset.revisionFiles==='artifacts',items=artifacts?plan.artifacts:plan.workspace_files,list=details.querySelector('ul'),more=details.querySelector('[data-revision-more]');let shown=0;
  const append=()=>{const end=Math.min(shown+100,items.length),fragment=document.createDocumentFragment();for(;shown<end;shown++){const item=items[shown],li=document.createElement('li');li.dataset.noI18n='';li.textContent=artifacts?`${item.path} · v${item.version}`:item;fragment.append(li)}list.append(fragment);more.hidden=shown>=items.length};
  details.addEventListener('toggle',()=>{if(details.open&&!shown)append()});more.onclick=append;
 }
}
async function previewRevision(node=null){
 if(!app.editBase)return;try{await saveWorkflow()}catch(error){toast(error.message);return}const base=app.editBase.id,workflow=structuredClone(app.wf),version=++revisionRequest;
 try{
  const p=await api('/runs/'+base+'/revision/preview',{workflow,node});
  if(version!==revisionRequest||app.editBase?.id!==base)return;
  const body=`<p class="note">${uiText('Continuerai nella stessa cartella del progetto, senza copie. La run precedente resta nella cronologia e gli artefatti già salvati restano immutabili. I file del progetto continueranno a cambiare con il nuovo lavoro.')}</p><label>${uiText('Riparti dal nodo')}<jui-select id="revision-node"><jui-option value="">${uiText('Automatico: primo passaggio da eseguire')}</jui-option>${p.candidates.map(n=>`<jui-option data-no-i18n value="${esc(n.id)}" ${node===n.id?'selected':''}>${esc(n.title)}</jui-option>`).join('')}</jui-select></label><p class="note">${p.restart?uiText('La ripartenza inizierà da')+' <strong data-no-i18n>'+esc(workflow.nodes.find(n=>n.id===p.restart)?.title||p.restart)+'</strong>':uiText('Tutti i passaggi del percorso sono già completati. Scegli un nodo precedente da rieseguire.')}</p><div class="revision-columns"><section><h3>${uiText('Risultati mantenuti')} (${p.kept.length})</h3>${revisionSteps(p.kept)}</section><section><h3>${uiText('Nodi da eseguire di nuovo')} (${p.rerun.length})</h3>${revisionSteps(p.rerun)}<p class="note">${uiText('I rami condizionali verranno seguiti secondo gli esiti della nuova run.')}</p></section></div>${p.removed.length?`<section><h3>${uiText('Nodi rimossi')} (${p.removed.length})</h3>${revisionSteps(p.removed)}</section>`:''}${revisionFileList('Versioni degli artefatti mantenute',p.artifacts.length,'artifacts')}${revisionFileList('File del progetto condiviso',p.workspace_files.length,'workspace')}<p class="note">${uiText('Le approvazioni dei passaggi da rieseguire e le autorizzazioni a operazioni esterne vanno richieste di nuovo. Le sessioni degli agenti e i contatori della nuova run ripartono da zero; i risultati mantenuti restano disponibili come contesto.')}</p><p id="revision-error" class="error" role="alert"></p>`;
  modal('Anteprima ripartenza',body,`<button type="button" data-action="close-dialog">${uiText('Torna alle modifiche')}</button><div class="spacer"></div><button type="button" class="primary" id="revision-confirm" ${p.restart?'':'disabled'}>${icon('play')}${uiText('Crea run e continua')}</button>`,true);
  bindRevisionFileLists(p);
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
function memoryReportTruncated(node){
 return node.kind==='Memory'&&app.run?.outputs?.[node.id]?.details_truncated===true;
}
function aiDecisionBadge(node){const count=app.decisions.filter(d=>d.node===node&&d.actor==='ai').length;return count?`<span class="ai-decision-badge">${icon('bot')}<span>AI · ${count}</span></span>`:''}
function renderFooterInfo(){
 const label=app.run?.error
  ? 'Errore nella run · Vedi il registro attività'
  : app.user?.isolated
   ? 'Workspace personali isolati · Sessioni persistenti'
   : 'Modalità sviluppo · Isolamento dei file non attivo';
 $('#footer-info').textContent=uiText(label);
}
function render(){if(!app.wf)return;renderFooterInfo();if(!app.selected||!app.selectedNodes.includes(app.selected))app.selectedNodes=[];$('#execution-mode-label').textContent=executionModes[(app.run?.execution||app.wf.execution)?.mode||'interactive'];$('#delete-workflow').disabled=!app.workflows.some(w=>w.id===currentWorkflow().id);$('#rename-workflow').disabled=$('#delete-workflow').disabled;$('#dirty').hidden=!app.dirty;$('#execute').disabled=!!app.run&&['running','starting'].includes(app.run.status);$('#save').disabled=!!app.run;$('#pause').hidden=!app.run||!['running','starting','waiting_child','waiting_review','waiting_user'].includes(app.run.status);$('#stop').hidden=$('#pause').hidden;$('#resume').hidden=!app.run||!['paused','failed','interrupted'].includes(app.run.status);$('#close-run').hidden=!app.run;$('#run-storage').hidden=!app.run;$('#run-family').hidden=!app.run||(!app.run.parent&&!app.children.length);$('#add-context').hidden=!app.run;$('#add-context').disabled=!app.run||!['paused','waiting_review','waiting_user'].includes(app.run.status);updateViewContext();if(latestRunWorkflow!==currentWorkflow().id)refreshLatestRun(currentWorkflow().id).catch(e=>toast(e.message));const pending=app.run?.pending;$('#review-banner').hidden=!pending&&!app.reviewRun;if(!pending&&app.reviewRun)$('#review-banner span:last-child').textContent='Review nel workflow figlio';$('#review-banner').disabled=!!pending&&app.run.status==='running'&&(app.run.execution?.mode||'interactive')!=='interactive';if(pending)$('#review-banner span:last-child').textContent=app.run.status==='running'&&app.run.execution?.mode!=='interactive'?'Supervisore AI in valutazione':pending.proposal?'Proposta AI da confermare':pending.kind==='question'?'Domanda in attesa':pending.kind==='gate'?'Gate in attesa':'Review richiesta';$('#graph').hidden=app.view!=='graph';$('#execution-list').hidden=app.view!=='list';$('#run-history').hidden=app.view!=='history';$('#palette').hidden=app.view==='history';$('#graph-tab').classList.toggle('active',app.view==='graph');$('#list-tab').classList.toggle('active',app.view==='list');$('#history-tab').classList.toggle('active',app.view==='history');for(const id of ['graph','list','history'])$('#'+id+'-tab').setAttribute('aria-selected',String(app.view===id));renderGraph();renderList();renderHistory();renderInspector();renderActivityLog();window.nodoManagerContextChanged?.();}
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
    const reportWarning=memoryReportTruncated(n)?' · '+uiText('Report troncato'):'';
    const badge=n.kind==='Task'&&n.skip?'fast-forward':stepMatchesFilter(n,'running')?'loader-circle':s==='completed'?'check':['failed','interrupted'].includes(s)?'x':stepMatchesFilter(n,'review')?'circle-help':null;
    return `<div class="node ${n.kind==='Task'&&n.skip?'task-bypassed':''} ${selectedNodeIds().includes(n.id)?'selected':''} ${filterClass}" data-id="${esc(n.id)}" data-kind="${n.kind}" data-status="${s}" data-filter-match="${matches}" style="left:${n.x}px;top:${n.y}px"><button class="node-main" data-action="select-node" data-id="${esc(n.id)}" aria-label="${esc(n.title)} · ${esc(nodeLabel(n)+agentLabel)} · ${esc(states[s])}${n.id===w.entry?' · '+uiText('Inizio del workflow'):''}${app.stepFilter!=='all'&&!matches?' · Fuori filtro':''}${esc(reportWarning)}" data-tip="${esc(n.title)} · ${esc(nodeLabel(n)+agentLabel+reportWarning)}">${n.id===w.entry?`<span class="node-entry-marker" data-tip="${uiText('Inizio del workflow')}" aria-label="${uiText('Inizio del workflow')}">${icon('workflow-start')}</span>`:''}${nodeIcon(n,true)}${associatedAgentBadge(agent)}${reportWarning?'<span class="node-badge memory-report-warning" aria-hidden="true">!</span>':badge?`<span class="node-badge">${n.kind==='Task'&&n.skip&&['bypassed','completed'].includes(s)?icon('check','task-skip-check'):''}${icon(badge)}</span>`:''}</button><button class="port in" data-action="port-in" data-id="${esc(n.id)}" aria-label="Collega a ${esc(n.title)}"></button><button class="port out ${app.link===n.id?'chosen':''}" data-action="port-out" data-id="${esc(n.id)}" aria-label="Collega da ${esc(n.title)}"></button><span class="node-title">${esc(n.title)}${n.kind==='Task'&&n.skip?`<span class="task-skip-label">${uiText('Saltato')}</span>`:''}</span><span class="node-meta">${esc(nodeLabel(n))}</span>${s.startsWith('waiting')?`<span class="node-wait">${esc(states[s])}</span>`:''}${aiDecisionBadge(n.id)}</div>`;
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
    const s=state(n),agent=associatedAgent(n),running=stepMatchesFilter(n,'running'),pending=stepMatchesFilter(n,'review'),statusIcon=running?'loader-circle':pending?'circle-help':n.kind==='Task'&&n.skip?'fast-forward':s==='completed'?'check':['failed','interrupted'].includes(s)?'x':null;
    const agentBadge=agent?`<span class="list-agent-badge" data-agent="${esc(agent.id)}" data-tip="${esc(agent.title)} · ${esc(nodeLabel(agent))}">${nodeIcon(agent)}<span>${esc(nodeLabel(agent))}</span></span>`:'';
    return `<button class="step-row ${agent?'has-agent':''} ${pending?'pending':''} ${running?'running':''}" data-action="select-node" data-id="${esc(n.id)}" data-kind="${esc(n.kind)}" data-status="${esc(s)}">${nodeIcon(n)}<span class="step-copy"><b>${esc(n.title)}</b><small>${esc([nodeService(n)?nodeLabel(n):'',n.command||n.operation||n.kind].filter(Boolean).join(' · '))}</small></span>${agentBadge}${aiDecisionBadge(n.id)}<span class="step-state">${memoryReportTruncated(n)?`<span class="memory-report-warning">${esc(uiText('Report troncato'))}</span>`:''}${n.kind==='Task'&&n.skip&&['bypassed','completed'].includes(s)?icon('check','task-skip-check'):''}${statusIcon?icon(statusIcon):''}<span>${esc(!app.run&&n.kind==='Task'&&n.skip?uiText('Saltato'):states[s]||s)}</span></span></button>${n.kind==='Workflow'&&app.run?childCalls(n.id):''}`;
  }).join('')}`).join('');
  $('#execution-list').innerHTML=(app.run?.error?`<p class="error" role="alert">${esc(app.run.error)}</p>`:'')+(!app.run?'<p class="note">Avvia il workflow per osservare agenti, sessioni e avanzamento reali.</p>':'')+filters+`<div id="step-results">${visible.length?rows:empty}</div>`;
  if(focusedFilter)$('#execution-list .list-filter[data-filter="'+focusedFilter+'"]').focus({preventScroll:true});
}
function renderHistory(){const workflow=currentWorkflow(),runs=app.runs.filter(r=>r.workflow_id===workflow.id&&!(app.schedules||[]).some(s=>s.schedule.id===r.schedule_id));$('#run-history').innerHTML=(typeof scheduleRows==='function'?scheduleRows(workflow.id):'')+'<h2 class="phase-title">Esecuzioni persistenti</h2>'+`<p class="small" data-no-i18n>${esc(workflow.name)}</p>`+(!runs.length?'<p class="note">Nessuna esecuzione per questo workflow.</p>':'')+runs.map(r=>`<button class="history-row" data-action="load-run" data-id="${r.id}">${icon(r.parent?'git-branch':'workflow')}<span><span data-no-i18n>${esc(r.name)}</span>${r.parent?`<small>Workflow principale · <span data-no-i18n>${esc(r.parent.run.slice(0,8))}</span></small>`:''}<small class="code"> · ${r.id.slice(0,8)} · ${new Date(r.created*1000).toLocaleString(nodoLocale())}</small></span><span class="status-label">${esc(states[r.status]||r.status)}</span></button>`).join('')}
async function showExecutions(){
 const owner=app.user?.id;
 modal('Registro esecuzioni','<p id="executions-loading" role="status">Caricamento esecuzioni…</p>');
 try{
  const [runs]=await Promise.all([api('/runs'),refreshSchedules()]);
  if(app.user?.id!==owner||$('#app').hidden||!$('#dialog').open||!$('#executions-loading'))return;
  const ordered=runs.filter(r=>!(app.schedules||[]).some(s=>s.schedule.id===r.schedule_id)).sort((a,b)=>b.created-a.created);
  const statusOptions=[...new Set(ordered.map(r=>r.status))];
  modal('Registro esecuzioni',`<div id="executions-schedules">${scheduleRows()}</div><div class="form-grid"><label>Cerca workflow<input type="search" id="executions-search" placeholder="Cerca per nome…" autocomplete="off"></label><label>Stato dell’esecuzione<jui-select id="executions-status"><jui-option value="">Tutti gli stati</jui-option>${statusOptions.map(status=>`<jui-option value="${esc(status)}">${esc(uiText(states[status]||status))}</jui-option>`).join('')}</jui-select></label></div><p class="small">Dal più recente al meno recente</p><p class="small" id="executions-count" role="status" aria-live="polite"></p><div id="executions-rows"></div>`,'<button type="button" data-action="close-dialog">Chiudi</button>',true);
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
let activeWorkflowCount=0,activeWorkflowRequest=null;
function renderActiveWorkflowCount(count){
 activeWorkflowCount=count;
 const indicator=$('#running-workflows'),label=uiText('Flussi in esecuzione')+': '+count;
 indicator.hidden=count===0;indicator.setAttribute('aria-label',label);indicator.dataset.tip=label;
 $('.running-workflows-count',indicator).textContent=String(count);
}
async function refreshActiveWorkflowCount(){
 if(!app.user||$('#app').hidden){renderActiveWorkflowCount(0);return}
 if(document.hidden||activeWorkflowRequest)return;
 const owner=app.user.id,request={};activeWorkflowRequest=request;
 try{
  const data=await api('/runs/active-count');
  if(app.user?.id===owner&&!$('#app').hidden&&Number.isSafeInteger(data.count)&&data.count>=0)renderActiveWorkflowCount(data.count);
 }catch{if(app.user?.id===owner)renderActiveWorkflowCount(0)}
 finally{if(activeWorkflowRequest===request)activeWorkflowRequest=null}
}
setInterval(()=>void refreshActiveWorkflowCount(),3000);
window.addEventListener('focus',()=>void refreshActiveWorkflowCount());
document.addEventListener('visibilitychange',()=>{if(!document.hidden)void refreshActiveWorkflowCount()});
window.addEventListener('nodo-language-changed',()=>renderActiveWorkflowCount(activeWorkflowCount));

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
async function refreshHistory(){const id=currentWorkflow().id;const [runs]=await Promise.all([api('/runs?workflow_id='+encodeURIComponent(id)),refreshSchedules()]);if(currentWorkflow().id!==id)return;app.runs=runs;renderHistory()}
function input(label,name,value,type='text',placeholder=''){return `<label>${label}<input name="${name}" autocomplete="${type==='password'?'new-password':'off'}" ${type==='password'?'data-lpignore="true" data-1p-ignore="true" data-bwignore="true"':''} type="${type}" value="${esc(value)}"${placeholder?` placeholder="${esc(placeholder)}"`:''}></label>`}
function select(label,name,value,options){if(value!==''&&value!=null&&!options.some(o=>(Array.isArray(o)?o[0]:o)===value))options=[[value,String(value)+' · '+uiText('Non disponibile')],...options];return `<label>${label}<jui-select name="${name}">${options.map(o=>{if(o===null)return '<jui-separator></jui-separator>';const v=Array.isArray(o)?o[0]:o,t=Array.isArray(o)?o[1]:o;return `<jui-option ${['agent','target','base_run','workflow_id','mcp_id','api_endpoint'].includes(name)&&v?'data-no-i18n':''} value="${esc(v)}" ${v===value?'selected':''}>${esc(t)}</jui-option>`}).join('')}</jui-select></label>`}
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
let artifactShowHidden=false;
const artifactFolders={hidden:new Set(),expanded:new Set(),modified:new Set(),defaultsSeen:new Set()};
function artifactKey(a){return a.file?.change==='report'?'report:'+a.node:a.file?.location||a.path}
function artifactTime(a){return a.file?.modified||a.created*1000||0}
function artifactDate(a){const time=artifactTime(a);return time?new Date(time).toLocaleString(nodoLocale(),{year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false}):'—'}
function artifactRoot(a){return a.file?.root||(app.run?.context?.['direct-primary']?'/sources':'/data')}
const artifactCacheDirectories=new Set(['node_modules','__pycache__','.pytest_cache','.mypy_cache','.ruff_cache','.cache','.hypothesis','.tox','.nox','.parcel-cache','.turbo','.sass-cache']);
function artifactIsCache(a){
 const parts=a.path.split(/[\\/]/),name=parts.at(-1);
 return parts.some(part=>artifactCacheDirectories.has(part))||/\.py[co]$/i.test(name)||['.DS_Store','Thumbs.db'].includes(name);
}
function latestArtifacts(arts){
 const latest=new Map();for(const a of arts){if(artifactIsCache(a))continue;const key=artifactKey(a),old=latest.get(key);if(!old||a.version>=old.version)latest.set(key,a)}
 return [...latest.values()];
}
function artifactFolderPath(a){return artifactRoot(a)+'/'+a.path.split(/[\\/]/).filter(p=>p&&p!=='.').join('/')}
function artifactFolderVisible(path){return ![...artifactFolders.hidden].some(folder=>path===folder||path.startsWith(folder+'/'))}
function applyArtifactFolderDefaults(arts){
 for(const artifact of arts){
  const parts=artifactFolderPath(artifact).split('/').filter(Boolean);parts.pop();
  for(let i=0;i<parts.length-1;i++)if(parts[i]==='target'&&parts[i+1]==='debug'){
   const path='/'+parts.slice(0,i+2).join('/');
   if(!artifactFolders.defaultsSeen.has(path)){artifactFolders.defaultsSeen.add(path);artifactFolders.hidden.add(path)}
  }
 }
}
function artifactFolderTree(){
 const roots=new Map();
 for(const a of latestArtifacts(app.artifacts)){
  const parts=artifactFolderPath(a).split('/').filter(Boolean);parts.pop();let children=roots,path='';
  for(const name of parts){path+='/'+name;if(!children.has(name))children.set(name,{name,path,children:new Map()});children=children.get(name).children}
 }
 return roots;
}
function artifactFolderRows(tree){
 return [...tree.values()].sort((a,b)=>a.name.localeCompare(b.name)).map(folder=>{
  const visible=artifactFolderVisible(folder.path),inherited=[...artifactFolders.hidden].some(parent=>parent!==folder.path&&folder.path.startsWith(parent+'/'));
  const expanded=artifactFolders.expanded.has(folder.path),group='artifact-folder-'+encodeURIComponent(folder.path);
  const label=`${icon('folder-git-2')}<span data-no-i18n title="${esc(folder.path)}">${esc(folder.name)}</span>`;
  return `<li><div class="artifact-folder-row">${folder.children.size?`<button type="button" class="artifact-folder-branch" data-artifact-expand="${esc(folder.path)}" aria-label="${esc(uiText(expanded?'Comprimi cartella':'Espandi cartella')+' '+folder.path)}" aria-expanded="${expanded}" aria-controls="${esc(group)}">${icon('chevron-right')}${label}</button>`:`<div class="artifact-folder-leaf">${label}</div>`}<button type="button" class="storage-hidden-toggle" role="switch" data-artifact-folder="${esc(folder.path)}" aria-label="${esc(uiText('Mostra cartella')+' '+folder.path)}" aria-checked="${visible}" ${inherited?'disabled':''}><span aria-hidden="true"></span></button></div>${folder.children.size?`<ul id="${esc(group)}" data-artifact-children="${esc(folder.path)}" ${expanded?'':'hidden'}>${artifactFolderRows(folder.children)}</ul>`:''}</li>`;
 }).join('');
}
function refreshArtifactFolderDialog(){
 const tree=$('[data-artifact-folders]');if(!tree)return;
 const focused=document.activeElement?.dataset.artifactFolder,scroll=tree.closest('.modal-body').scrollTop;
 tree.innerHTML=artifactFolderRows(artifactFolderTree())||`<li class="note">${uiText('Nessuna cartella disponibile.')}</li>`;
 tree.closest('.modal-body').scrollTop=scroll;
 for(const toggle of $$('[data-artifact-folder]',tree))toggle.onclick=()=>{
  const path=toggle.dataset.artifactFolder;artifactFolders.modified.add(path);
  if(artifactFolderVisible(path))artifactFolders.hidden.add(path);
  else for(const folder of [...artifactFolders.hidden])if(folder===path||folder.startsWith(path+'/'))artifactFolders.hidden.delete(folder);
  bindArtifactSearch();refreshArtifactFolderDialog();
  $$('[data-artifact-folder]',tree).find(el=>el.dataset.artifactFolder===path)?.focus({preventScroll:true});
 };
 for(const button of $$('[data-artifact-expand]',tree))button.onclick=()=>{
  const path=button.dataset.artifactExpand,expanded=!artifactFolders.expanded.has(path);
  if(expanded)artifactFolders.expanded.add(path);else artifactFolders.expanded.delete(path);
  button.setAttribute('aria-expanded',String(expanded));
  button.setAttribute('aria-label',uiText(expanded?'Comprimi cartella':'Espandi cartella')+' '+path);
  document.getElementById(button.getAttribute('aria-controls')).hidden=!expanded;
 };
 if(focused)$$('[data-artifact-folder]',tree).find(el=>el.dataset.artifactFolder===focused)?.focus({preventScroll:true});
}
function artifactFolderDialog(){
 artifactFolders.expanded.clear();
 for(const path of artifactFolders.modified){
  const parts=path.split('/').filter(Boolean);parts.pop();let ancestor='';
  for(const part of parts){ancestor+='/'+part;artifactFolders.expanded.add(ancestor)}
 }
 modal('Filtra artefatti',`<p class="note">${uiText('Scegli le cartelle da mostrare. Nascondere una cartella esclude anche tutte le sue sottocartelle. I file restano conservati.')}</p><ul class="artifact-folder-tree" data-artifact-folders></ul>`,`<button type="button" data-action="close-dialog" class="primary">${uiText('Chiudi')}</button>`);
 refreshArtifactFolderDialog();
}
function inspectorArtifacts(arts){
 const scope=app.user?.id+':'+app.run?.id;if(artifactSearch.scope!==scope){artifactSearch.scope=scope;artifactSearch.value='';artifactFolders.hidden.clear();artifactFolders.modified.clear();artifactFolders.expanded.clear();artifactFolders.defaultsSeen.clear()}
 const all=latestArtifacts(arts).sort((a,b)=>artifactTime(b)-artifactTime(a)||b.created-a.created||a.path.localeCompare(b.path)),reports=all.filter(a=>a.file?.change==='report'||(!a.file&&/^\.(?:rightcommit|nodo)\/reports\//.test(a.path))),files=all.filter(a=>!reports.includes(a));
 applyArtifactFolderDefaults(all);
 const row=a=>`<button type="button" class="artifact-item" data-action="artifact" data-id="${esc(a.id)}" data-artifact-hidden="${a.path.split(/[\\/]/).some(part=>part.startsWith('.')&&part!=='.'&&part!=='..')}" data-artifact-folder-path="${esc(artifactFolderPath(a))}" data-artifact-search="${esc(a.path.toLocaleLowerCase())}" data-tip="${esc(a.file?.location||a.path)}">${icon('file-text')}<span class="artifact-file"><span data-no-i18n>${esc(a.path)}</span><small>${uiText('Versione')} ${a.version}</small><time class="artifact-date" data-no-i18n datetime="${artifactTime(a)?new Date(artifactTime(a)).toISOString():''}" title="${esc(uiText(a.file?.change==='legacy'?'Data della versione salvata':'Ultima modifica del file'))}">${esc(artifactDate(a))}</time></span><span class="artifact-origin" data-no-i18n>${artifactRoot(a).slice(1)}</span></button>`;
 return `<div class="inspector-artifacts-heading"><span class="eyebrow" data-artifact-count>ARTEFATTI · ${files.length}</span><span class="small">${uiText('Tutta la run')}</span></div><input type="search" class="artifact-search" data-artifact-query value="${esc(artifactSearch.value)}" placeholder="${esc(uiText('Cerca artefatti…'))}" aria-label="${esc(uiText('Cerca artefatti'))}"><div class="storage-hidden-control artifact-hidden-control"><button type="button" data-artifact-filter>${uiText('Filtra')}</button><span id="artifact-hidden-label">${uiText('Mostra nascosti')}</span><button type="button" role="switch" class="storage-hidden-toggle" data-artifact-show-hidden aria-checked="${artifactShowHidden}" aria-labelledby="artifact-hidden-label"><span aria-hidden="true"></span></button></div><div data-inspector-artifacts>${files.map(row).join('')}<p class="note" data-artifact-empty>${uiText(files.length?'Nessun artefatto corrispondente.':'I file creati o modificati nella run appariranno qui.')}</p>${reports.length?`<details class="artifact-reports"><summary>${uiText('Report')} · ${reports.length}</summary>${reports.map(row).join('')}</details>`:''}</div>`;
}
function bindArtifactSearch(){
 const input=$('[data-artifact-query]');if(!input)return;
 const apply=()=>{const query=input.value.toLocaleLowerCase().trim();artifactSearch.value=input.value;const rows=$$('[data-artifact-search]',$('#artifacts-panel'));rows.forEach(row=>row.hidden=(!artifactShowHidden&&row.dataset.artifactHidden==='true')||!row.dataset.artifactSearch.includes(query)||!artifactFolderVisible(row.dataset.artifactFolderPath));const files=rows.filter(row=>!row.closest('.artifact-reports')),visible=files.filter(row=>!row.hidden).length;const count=$('[data-artifact-count]');if(count)count.textContent=uiText('ARTEFATTI · '+visible);const empty=$('[data-artifact-empty]');if(empty)empty.hidden=visible>0;const reports=$('.artifact-reports');if(reports)reports.hidden=!rows.some(row=>row.closest('.artifact-reports')&&!row.hidden)};
 const toggle=$('[data-artifact-show-hidden]');if(toggle)toggle.onclick=()=>{artifactShowHidden=!artifactShowHidden;toggle.setAttribute('aria-checked',String(artifactShowHidden));apply()};
 const filter=$('[data-artifact-filter]');if(filter){filter.onclick=artifactFolderDialog;filter.setAttribute('aria-pressed',String(artifactFolders.hidden.size>0))}
 input.oninput=apply;apply();
}
function nodePromptLabel(kind){
 if(kind==='A2A')return 'Richiesta';
 if(kind==='Workflow')return 'Contesto da passare';
 if(['Task','Review','Gate'].includes(kind))return 'Prompt / criteri';
 return '';
}
function editNodePrompt(id){
 if(app.run)return;
 const workflow=app.wf;
 const node=workflow.nodes.find(n=>n.id===id);
 const label=nodePromptLabel(node?.kind);
 if(!label)return;
 captureInspectorFields();
 modal('Imposta prompt',
  `<p class="note" data-no-i18n>${esc(node.title||node.id)}</p><label>${esc(uiText(label))}<textarea id="node-prompt-editor" rows="14" autofocus></textarea></label>`,
  '<button type="button" data-action="close-dialog">Chiudi</button><button type="button" id="save-node-prompt" class="primary">Salva</button>',true);
 const editor=$('#node-prompt-editor');
 editor.value=node.prompt||'';
 $('#save-node-prompt').onclick=async()=>{
  if(app.run||app.wf!==workflow||!workflow.nodes.includes(node)){closeModal();return}
  const value=editor.value;
  closeModal();
  if(value===(node.prompt||''))return;
  node.prompt=value;
  dirty();
  render();
  try{await saveWorkflow();toast('Workflow salvato')}catch(error){toast(error.message)}
 };
}
function renderInspector(){
 const artifactInput=$('[data-artifact-query]'),artifactFocus=artifactInput===document.activeElement,artifactSelection=artifactInput?.selectionStart,artifactScroll=$('[data-inspector-artifacts]')?.scrollTop||0;
 const selected=selectedNodeIds();
 const artifactNode=currentWorkflow().nodes.find(n=>n.id===app.selected),stackVisible=(!app.run&&selected.length>1)||!!(artifactNode&&artifactNode.kind!=='Comment');
 $('#inspector-stack').hidden=!stackVisible;
 $('#artifacts-panel').innerHTML=stackVisible?inspectorArtifacts(app.artifacts):'';if(stackVisible)bindArtifactSearch();refreshArtifactFolderDialog();
 if(!app.run&&selected.length>1){$('#inspector').hidden=false;$('#inspector').innerHTML=`<div class="panel-heading">${icon('workflow')}<b>${selected.length} ${esc(uiText('nodi selezionati'))}</b><div class="spacer"></div><button type="button" class="icon-button" data-action="clear-node-selection" aria-label="${esc(uiText('Deseleziona'))}">${icon('x')}</button></div><p class="note">Trascina uno dei nodi selezionati per spostare il gruppo. Shift + clic aggiunge o rimuove un nodo.</p><button type="button" class="danger" data-action="delete-selected-nodes">${icon('trash-2')}${esc(uiText('Cancella selezione'))}</button>`;return}
 const n=currentWorkflow().nodes.find(n=>n.id===app.selected);$('#inspector').hidden=!n||n.kind==='Comment';if(!n||n.kind==='Comment'){$('#inspector').replaceChildren();return}let fields=input('Nome','title',n.title)+input('Fase','phase',n.phase||'Workflow');const agents=currentWorkflow().nodes.filter(n=>n.kind==='Agent').map(n=>[n.id,n.title]);if(n.kind==='A2A')fields+=select('Agente remoto','a2a_id',n.a2a_id||'',[['','Seleziona connessione'],...a2aConnections.map(c=>[c.id,c.name])])+`<label>${esc(uiText(nodePromptLabel(n.kind)))}<textarea name="prompt" rows="6">${esc(n.prompt||'')}</textarea></label><button type="button" data-action="a2a-connections">Connessioni A2A</button><button type="button" data-action="a2a-probe">Verifica Agent Card</button><p class="note">Invia richiesta, input della run e contesto a monte. I chiarimenti riprendono lo stesso task remoto.</p>`;if(n.kind==='Agent')fields+=select('Runtime','runtime',n.runtime||'', [['','Preferenza personale'],['codex','Codex'],['claude','Claude Code'],['pi','Pi Agent']])+piNodeFields(n)+input('Modello (vuoto = configurazione personale)','model',n.model||'');if(['Task','Gate'].includes(n.kind)||n.kind==='Review')fields+=select('Agent','agent',n.agent||'', [['','Seleziona Agent'],...agents]);if(n.kind==='Task')fields+=`<label class="task-skip-switch"><span>Salta task</span><jui-switch name="skip" aria-label="Salta task" ${n.skip?'checked':''}></jui-switch></label><p class="note">Il task non esegue operazioni e passa al nodo successivo. Non produce output; la configurazione resta salvata.</p>`+input('Skill Spec Kit (es. implement)','command',n.command||'');if(['Task','Gate','Review'].includes(n.kind))fields+=`<label>${esc(uiText(nodePromptLabel(n.kind)))}<textarea name="prompt" rows="7">${esc(n.prompt)}</textarea></label>`;if(n.kind==='Gate')fields+=input('Esiti separati da virgola','outcomes',(n.outcomes||[]).join(', '))+input('Limite rework','max_rework',n.max_rework??2,'number')+select('Richiede test deterministici','require_tests',String(!!n.require_tests),[['false','No'],['true','Sì']]);if(n.kind==='Gate')fields+=select('Verifica chiusura task','require_tasks_closed',String(!!n.require_tasks_closed),[['false','No'],['true','Sì']])+input('Percorso tasks.md (vuoto = feature unica)','location',n.location||'');if(n.kind==='Review')fields+=select('Review','mode',n.mode||'agent_and_human',[['human','Umana'],['agent','Agente in contesto separato'],['agent_and_human','Agente + approvazione umana']])+input('Limite rework','max_rework',n.max_rework??2,'number');if(n.kind==='Source')fields+=select('Tipo Source','source_type',n.source_type||'example',[['example','Repository precaricato'],['folder','Folder in /sources'],['examples','Folder in examples'],['git','Repository Git HTTPS']])+sourcePathField(n)+sourceModeFields(n)+`<button type="button" data-action="browse-source">${icon('folder-git-2')}Sfoglia Storage</button>`;if(n.kind==='Memory')fields+=select('Task sorgente','agent',n.agent||'',[['','Seleziona Task'],...currentWorkflow().nodes.filter(n=>n.kind==='Task').map(n=>[n.id,n.title])])+select('Agent destinatario','target',n.target||'',[['','Seleziona Agent'],...agents]);if(n.kind==='Connection')fields+=select('Provider','provider',n.provider||'local',[['local','Git / CI locali'],['github','GitHub'],['azure','Azure DevOps'],['servicenow','ServiceNow'],['atlassian','Bitbucket / Atlassian TWG']])+`<div id="connection-fields">${connectionFields(n)}</div>`;if(n.kind==='RAG')fields+=select('Cartella indicizzata','rag_index',n.rag_index||'',[['','Seleziona indice RAG'],...ragCatalog.filter(i=>i.directory&&i.generation).map(i=>[i.id,i.name+' · '+i.root+'/'+i.path])])+select('Modalità di ricerca','rag_mode',n.rag_mode||'',[['','Configurazione indice'],['hybrid','Hybrid'],['semantic','Semantic'],['keyword','Keyword']])+input('Risultati per ricerca','rag_top_k',n.rag_top_k??8,'number')+`<button type="button" data-action="rag-node-manage">${icon('database-rag')}Gestisci indice</button><p class="note">Collega questo nodo a un Agent con un arco context. Ricerca, stato, elenco documenti e lettura chunk si aggiungono ai tool nativi. L’indice descrive la sorgente, non le modifiche successive nel workspace.</p>`;if(n.kind==='MCP')fields+=select('Server MCP personale','mcp_id',n.mcp_id||'',[['','Seleziona server MCP'],...Object.entries(app.settings.mcp_servers||{}).map(([id,c])=>[id,c.name||id])])+`<label>Tool consentiti (vuoto = tutti)<textarea name="mcp_tools" rows="4">${esc((n.mcp_tools||[]).join('\n'))}</textarea></label><div class="actions"><button type="button" data-action="mcp-node-probe">${icon('list-checks')}Scopri e seleziona tool</button><button type="button" data-action="mcp-settings">${icon('settings-2')}Server MCP personali</button></div><p class="note">Collega la porta di uscita a un Agent con un arco context. I tool nativi restano disponibili.</p>`;if(n.kind==='Workflow')fields+=select('Workflow da richiamare','workflow_id',n.workflow_id||'',[['','Seleziona workflow'],...app.workflows.filter(w=>w.id!==currentWorkflow().id).map(w=>[w.id,w.name])])+`<label>${esc(uiText(nodePromptLabel(n.kind)))}<textarea name="prompt" rows="5">${esc(n.prompt||'')}</textarea></label><p class="note">Sessioni separate. Workspace ereditato dal chiamante; i Source del figlio non vengono ricopiati. Il padre attende il completamento.</p>`;if(n.kind==='Fork')fields+=select('Nodo di ricongiungimento','target',n.target||'',[['','Seleziona Join'],...currentWorkflow().nodes.filter(j=>j.kind==='Join').map(j=>[j.id,j.title])])+`<p class="note">Collega almeno due rami di Task, ciascuno con un Agent distinto. Tutti i rami devono terminare nel Join selezionato. Il piano del Task precedente viene passato automaticamente a ogni agente, senza Memory. Il workspace è condiviso: assegna file diversi ai rami.</p>`;if(n.kind==='Join')fields+=`<p class="note">Attende tutti i rami del Fork e trasferisce i risultati agli agenti dei Task successivi, senza Memory.</p>`;const edges=currentWorkflow().edges.filter(e=>e.from===n.id||e.to===n.id);$('#inspector').innerHTML=`<div class="panel-heading">${nodeIcon(n)}<b>${esc(n.kind)}</b>${nodeService(n)?`<span class="small">${esc(nodeLabel(n))}</span>`:''}<div class="spacer"></div><button class="icon-button" data-action="close-inspector" aria-label="Chiudi dettagli">${icon('x')}</button></div>${app.run?`<p class="note">Run ${esc(app.run.id.slice(0,8))} · ${esc(states[state(n)])}<br>La configurazione del run è congelata.</p>`:''}${memoryReportTruncated(n)?`<p class="note memory-report-warning"><strong>${esc(uiText('Report troncato'))}</strong><br>${esc(uiText('Il report del Task sorgente è stato tagliato a 16.000 caratteri. Il workflow prosegue. Se il Task ha indicato un file con il report completo, consultalo. Se manca un dettaglio necessario, torna al Task sorgente e chiedi di salvare il report completo nel workspace.'))}</p>`:''}<form id="node-form" data-workflow-id="${esc(currentWorkflow().id)}" data-node-id="${esc(n.id)}" class="inspector-form"><fieldset ${app.run?'disabled':''} style="border:0;margin:0;padding:0;display:grid;gap:15px">${fields}</fieldset><div class="actions">${!app.run?`<button type="button" class="icon-button" data-action="save-inspector" data-tip="Salva" aria-label="Salva">${icon('save')}</button><button type="button" data-action="set-entry" data-id="${n.id}" data-tip="Imposta come inizio" aria-label="Imposta come inizio">${icon('log-in')}</button><button type="button" class="danger" data-action="delete-node" data-id="${n.id}" aria-label="Elimina nodo">${icon('trash-2')}</button>`:''}${n.kind==='Review'||app.decisions.some(d=>d.node===n.id)?`<button type="button" data-action="review-history" data-id="${n.id}" data-tip="Storico review" aria-label="Storico review">${icon('history')}</button>`:''}</div></form>${n.kind==='Workflow'&&app.run?childCalls(n.id):''}${app.run?.pending?.node===n.id?'<div class="divider"></div><button data-action="pending" class="review-banner">Apri review o domanda</button>':''}<div class="divider"></div><span class="eyebrow">COLLEGAMENTI</span>${edges.map((e,i)=>`<div class="note code inspector-connection-row"><span>${esc(e.from)} → ${esc(e.to)} · ${esc(e.kind)} ${esc(e.outcome)}</span>${!app.run?`<button type="button" class="icon-button inspector-connection-delete" data-action="delete-edge" data-index="${currentWorkflow().edges.indexOf(e)}" aria-label="Rimuovi arco">${icon('x')}</button>`:''}</div>`).join('')}${!edges.length?'<p class="note">Usa le porte del nodo per collegarlo.</p>':''}`;const artifactList=$('[data-inspector-artifacts]');if(artifactList)artifactList.scrollTop=artifactScroll;if(artifactFocus){const query=$('[data-artifact-query]');query?.focus({preventScroll:true});query?.setSelectionRange(artifactSelection,artifactSelection)}if(n.kind==='Source'){loadSourceFolderOptions($('#node-form'));if(app.editBase){for(const name of ['source_type','source_mode','location'])$('#node-form').elements[name].disabled=true;$('[data-source-folder-toggle]')?.setAttribute('hidden','');const browse=$('[data-action="browse-source"]');if(browse)browse.disabled=true;$('#source-folder-hint').hidden=false;$('#source-folder-hint').textContent=uiText('Le Source restano quelle della run originale. La continuazione usa gli stessi file del progetto.')}}}
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
function activityMessage(event,expanded=false){
 const data=activityData(event);
 if(event.kind==='compaction'&&data){const phase={start:'Compattazione avviata',end:'Compattazione completata',error:'Compattazione fallita'},mode={manual:'Manuale',auto:'Automatica',automatic:'Automatica',threshold:'Soglia automatica',overflow:'Contesto esaurito'};return `${uiText(phase[data.phase]||'Compattazione')} · ${data.runtime||''} · ${uiText(mode[data.mode]||data.mode||'Automatica')}${data.binding?' · '+data.binding:''}${data.tokens_before!=null?'\n'+uiText('Token prima')+': '+data.tokens_before:''}${data.tokens_after_estimate!=null?'\n'+uiText('Token dopo (stima)')+': '+data.tokens_after_estimate:''}${data.message?' · '+data.message:''}`}
 if(event.kind==='activity'&&data){const phase={start:'Avvio strumento',end:'Strumento completato',error:'Errore strumento',agent_start:'Agente avviato'};return `${uiText(phase[data.phase]||'Attività')} · ${data.runtime||''}${data.tool?' · '+data.tool:''}${data.detail?' · '+(expanded||data.phase==='error'?data.detail:activityDetail(data.detail)):''}${data.exit_code!=null?' · Exit: '+data.exit_code:''}`}
 if(event.kind==='output'){const match=event.text.match(/<nodo_result>([\s\S]*?)<\/nodo_result>/);if(match){try{const r=JSON.parse(match[1]);return [event.text.slice(0,match.index).trim(),r.summary,r.details].filter(Boolean).join(' · ')}catch{}}}
 if(event.kind==='usage'&&data)return `${data.runtime||''} · ${data.model||''}\n${uiText('Token in ingresso')}: ${data.input??'—'} · ${uiText('Token in uscita')}: ${data.output??'—'}${data.usd!=null?' · USD '+Number(data.usd).toFixed(4):''}`;
 if(event.kind==='agent_turn'&&data)return `${data.runtime||''} · ${uiText('Turno agente terminato')}`;
 if(event.kind==='state')return uiText(states[event.text]||event.text);
 return String(event.text??'');
}
function activityPreview(event){
 const message=activityMessage(event).replace(/\s+/g,' ').trim(),full=activityMessage(event,true).replace(/\s+/g,' ').trim();
 const extended=message.length>240||String(event.text??'').length>240||message!==full;
 return `<div class="activity-event-preview"><pre data-no-i18n>${esc(message.length>240?message.slice(0,240).trimEnd()+'…':message)}</pre>${extended?`<button type="button" class="activity-read" data-log-read="${esc(event.seq)}" aria-label="${esc(uiText('Leggi messaggio completo'))}">${uiText('Leggi')}</button>`:''}</div>`;
}
function readActivityEvent(runId,seq){
 const entry=activityLog.runs.get(runId),event=entry&&[...entry.events.values()].find(e=>String(e.seq)===String(seq));if(!event)return;
 const message=activityMessage(event,true),data=activityData(event),body=data!==null?JSON.stringify(data,null,2):String(event.text??'');
 const section=(title,text,key)=>`<section><div class="activity-message-heading"><h3>${uiText(title)}</h3><button type="button" class="activity-copy" data-log-copy="${key}" aria-label="${esc(uiText(key==='message'?'Copia messaggio completo':'Copia corpo esteso'))}">${uiText('Copia')}</button></div><pre data-no-i18n>${esc(text)}</pre></section>`;
 modal(uiText('Dettaglio messaggio'),`<div class="activity-message-dialog"><dl class="activity-message-meta"><div><dt>${uiText('Esecuzione')}</dt><dd data-no-i18n>${esc(entry.meta.name)} · ${esc(runId)}</dd></div><div><dt>${uiText('Nodo')}</dt><dd data-no-i18n>${esc(event.node||'run')}</dd></div><div><dt>${uiText('Data e ora')}</dt><dd data-no-i18n>${esc(new Date(event.created*1000).toLocaleString(nodoLocale()))}</dd></div></dl>${section('Messaggio completo',message,'message')}${body!==message?section('Corpo esteso',body,'body'):''}</div>`,`<button type="button" data-action="close-dialog">${uiText('Chiudi')}</button>`,true);
 $$('[data-log-copy]',$('#dialog')).forEach(button=>button.onclick=async()=>{
  const text=button.dataset.logCopy==='message'?message:body;
  try{
   if(navigator.clipboard?.writeText)await navigator.clipboard.writeText(text);
   else{
    const field=document.createElement('textarea');field.value=text;field.readOnly=true;field.style.cssText='position:fixed;opacity:0;pointer-events:none';$('#dialog').append(field);
    try{field.select();if(!document.execCommand('copy'))throw Error('copy failed')}finally{field.remove();button.focus({preventScroll:true})}
   }
   button.textContent=uiText('Copiato');
  }catch{toast(uiText('Impossibile copiare il testo'))}
 });
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
  return `${current?`<p class="activity-current" data-no-i18n>${esc(uiText('In corso'))}: ${esc(current.replace(/\s+/g,' ').trim())}</p>`:''}${entry.error?`<p class="error">${esc(entry.error)}</p><button type="button" data-log-retry="${esc(r.id)}">${uiText('Riprova')}</button>`:''}${entry.loading?`<p class="note">${uiText('Caricamento…')}</p>`:''}${events.map(event=>{const kind={activity:'Strumenti',output:'Risposta agente',state:'Stato',error:'Errore',usage:'Consumi',agent_turn:'Turno agente',compaction:'Compattazione',restart:'Riavvio server'}[event.kind]||event.kind;const title=app.run?.id===r.id?app.run.workflow.nodes.find(n=>n.id===event.node)?.title:null;return `<article class="activity-event" data-log-seq="${event.seq}"><div class="activity-event-meta"><time data-no-i18n>${esc(new Date(event.created*1000).toLocaleTimeString(nodoLocale()))}</time> · <span data-no-i18n>${esc(title||event.node||'run')}</span> · <span>${esc(uiText(kind))}</span></div>${activityPreview(event)}</article>`}).join('')}${!events.length&&!entry.loading&&!entry.error?`<p class="note">${uiText('Nessun messaggio disponibile.')}</p>`:''}`;
 })()}</div>`:`<p class="note">${uiText('Nessuna esecuzione per questo workflow.')}</p>`;
 const content=$('#activity-panel',panel);if(content){content.scrollTop=active.scroll+(sameRun&&active.scroll>0?Math.max(0,content.scrollHeight-oldHeight):0);content.onscroll=()=>{active.scroll=content.scrollTop}}
 const tabs=$$('[data-log-run]',panel);
 const tablist=$('.activity-tabs',panel);if(tablist)tablist.scrollLeft=tabScroll;
 if(focusedTab)tabs.find(tab=>tab.dataset.logRun===focusedTab)?.focus({preventScroll:true});
 const activate=tab=>{const entry=activityLog.runs.get(tab.dataset.logRun);activityLog.selected=entry.meta.id;renderActivityLog();$('#activity-tab-'+entry.meta.id)?.focus({preventScroll:true});if(!entry.loading)loadActivityEvents(entry.meta.id)};
 tabs.forEach((tab,i)=>{tab.onclick=()=>activate(tab);tab.onkeydown=e=>{let next;if(e.key==='ArrowRight')next=(i+1)%tabs.length;else if(e.key==='ArrowLeft')next=(i+tabs.length-1)%tabs.length;else if(e.key==='Home')next=0;else if(e.key==='End')next=tabs.length-1;else return;e.preventDefault();activate(tabs[next])}});
 $$('[data-log-page]',panel).forEach(button=>button.onclick=()=>{const index=(activityLog.page+Number(button.dataset.logPage))*pageSize;activityLog.selected=entries[index].meta.id;renderActivityLog();loadActivityEvents(activityLog.selected)});
 $$('[data-log-retry]',panel).forEach(button=>button.onclick=()=>loadActivityEvents(button.dataset.logRetry));
 $$('[data-log-read]',panel).forEach(button=>button.onclick=()=>readActivityEvent(active.meta.id,button.dataset.logRead));
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

async function loadRun(id,expectedWorkflow=null){if(!app.run&&app.dirty)await saveWorkflow();const owner=app.user?.id,d=await api('/runs/'+id);if(expectedWorkflow&&(owner!==app.user?.id||currentWorkflow().id!==expectedWorkflow))return false;leaveDraftContext();app.editBase=null;app.dirty=false;app.run=d.run;app.workspaceSuccessor=d.workspace_successor||null;app.selectedNodes=[];app.continueNodes=d.continue_nodes||[];app.children=d.children||[];app.reviewRun=d.review_run;workflowOptions(app.run.workflow.id);app.artifacts=d.artifacts;app.decisions=d.decisions;app.selected=null;app.selectedEdge=null;app.view=app.view==='list'?'list':'graph';app.events=[];app.lastEvent=0;logRun({id:app.run.id,name:app.run.workflow.name,workflow_id:app.run.workflow.id,status:app.run.status,created:app.run.created});activityLog.selected=app.run.id;latestRunWorkflow='';render();centerNode(app.run.current);connectStream(id);void refreshActiveWorkflowCount();return true}
function disconnect(){app.stream?.close();app.stream=null;clearTimeout(app.refresh)}
async function refreshRun(){if(!app.run)return;const prev=app.run.current,prevStatus=app.run.status,id=app.run.id;try{const d=await api('/runs/'+id);if(app.run?.id!==id)return;app.run=d.run;if(prevStatus!==app.run.status)void refreshActiveWorkflowCount();app.workspaceSuccessor=d.workspace_successor||null;app.continueNodes=d.continue_nodes||[];app.children=d.children||[];app.reviewRun=d.review_run;app.artifacts=d.artifacts;app.decisions=d.decisions;render();if(prev!==app.run.current&&app.view==='graph')centerNode(app.run.current);if(app.view==='history')await refreshHistory();else app.runs=await api('/runs')}catch(e){toast(e.message)}}
function connectStream(id){disconnect();const stream=new EventSource('/api/runs/'+id+'/stream');app.stream=stream;stream.onopen=()=>{if(app.stream!==stream)return;$('#connection-state').textContent='Live · Connesso';$('#graph').classList.remove('connection-lost');refreshRun();const entry=activityLog.runs.get(id);if(entry)loadActivityEvents(id)};stream.onerror=()=>{if(app.stream!==stream)return;$('#connection-state').textContent='Riconnessione…';$('#graph').classList.add('connection-lost')};stream.addEventListener('refresh',()=>{refreshRun();const entry=activityLog.runs.get(id);if(entry)loadActivityEvents(id)});stream.onmessage=e=>{if(app.stream!==stream||app.run?.id!==id)return;const v=JSON.parse(e.data);if(v.seq<=app.lastEvent)return;app.lastEvent=v.seq;app.events.push(v);if(app.events.length>500)app.events.shift();recordActivityEvent(id,v);if(['state','error','created'].includes(v.kind)){clearTimeout(app.refresh);app.refresh=setTimeout(refreshRun,120)}};}

const serviceInfo={
  // Previous logos: codex icon:'brand-openai'; claude icon:'brand-claude'.
  codex:{name:'Codex',icon:'bot',text:'OpenAI',description:'Sviluppo con sessioni persistenti'},
  claude:{name:'Claude Code',icon:'bot',text:'Claude',description:'Il tuo agente di sviluppo Anthropic'},
  pi:{name:'Pi Agent',icon:'brand-pi',description:'Agente di sviluppo multi-provider'},
  // Previous connection logos: 'brand-bitbucket', 'brand-azuredevops', 'brand-servicenow'.
  atlassian:{name:'Bitbucket / Atlassian TWG',icon:'plug',text:'Atlassian',description:'Jira, Confluence, Bitbucket e altri prodotti Atlassian'},
  github:{name:'GitHub',icon:'brand-github',description:'Issue, repository e pull request'},
  azure:{name:'Azure DevOps',icon:'plug',text:'Azure',description:'Work item, progetti e pull request'},
  servicenow:{name:'ServiceNow',icon:'plug',text:'ServiceNow',description:'Incident e aggiornamenti operativi'}
};
function connectionTargetFields(n){
 if(n.provider==='github')return input('Owner / organizzazione','organization',n.organization||'')+input('Repository','repository',n.repository||n.target||'');
 if(n.provider==='azure')return input('Organizzazione Azure DevOps','organization',n.organization||'')+input('Progetto','project',n.project||'')+input('Repository','repository',n.repository||n.target||'');
 if(n.provider==='atlassian')return input('Repository Bitbucket workspace/repository','repository',n.repository||'');
 return '';
}
function filterGithubEndpoints(catalog,query='',method=''){
 const terms=query.trim().toLowerCase().split(/\s+/).filter(Boolean);
 return catalog.filter(e=>(!method||e.method===method)&&terms.every(term=>(e.id+' '+e.method+' '+e.path+' '+e.label).toLowerCase().includes(term)));
}
function applyGithubEndpoint(n,endpoint){
 n.api_endpoint=endpoint.id;n.api_method=endpoint.method;n.api_path=(endpoint.server||'')+endpoint.path;
 if(endpoint.server==='https://uploads.github.com')n.api_encoding='base64';
 const parameters={};for(const match of endpoint.path.matchAll(/\{([^}]+)\}/g))if(!['owner','repo','org'].includes(match[1]))parameters[match[1]]='';
 n.api_parameters=Object.keys(parameters).length?JSON.stringify(parameters,null,2):'';dirty();renderInspector();
}
function showGithubCatalog(){
 captureInspectorFields();const workflow=currentWorkflow().id,node=app.selected,catalog=window.githubApiCatalog||[];
 modal('Operazioni API GitHub',`<div class="github-api-filters"><label>${uiText('Filtra operazioni')}<input type="search" data-github-search placeholder="${esc(uiText('Nome, metodo o percorso API'))}"></label><label>${uiText('Metodo HTTP')}<jui-select data-github-method><jui-option value="" selected>${uiText('Tutti i metodi')}</jui-option>${['GET','HEAD','OPTIONS','POST','PUT','PATCH','DELETE'].map(m=>`<jui-option value="${m}">${m}</jui-option>`).join('')}</jui-select></label></div><p class="note" data-github-count role="status" aria-live="polite"></p><div class="github-api-results" data-github-results></div><button type="button" data-github-more>${uiText('Mostra altre operazioni')}</button>`,'',true);
 const root=$('#dialog'),search=$('[data-github-search]',root),method=$('[data-github-method]',root),results=$('[data-github-results]',root),more=$('[data-github-more]',root);let limit=100;
 const update=()=>{
  const matches=filterGithubEndpoints(catalog,search.value,method.value),visible=matches.slice(0,limit);
  $('[data-github-count]',root).textContent=matches.length?`${uiText('Operazioni')}: ${matches.length} / ${catalog.length} · ${uiText('Visualizzate')}: ${visible.length}`:uiText('Nessuna operazione corrispondente.');
  results.innerHTML=visible.map(e=>`<button type="button" class="github-api-result" data-github-endpoint="${esc(e.id)}"><span class="code" data-no-i18n>${esc(e.method)}</span><span data-no-i18n><b>${esc(e.label)}</b><code>${esc(e.path)}</code><small>${esc(e.id)}</small></span></button>`).join('');
  more.hidden=visible.length===matches.length;
 };
 search.oninput=method.onchange=()=>{limit=100;update()};more.onclick=()=>{limit+=100;update()};
 results.onclick=e=>{const button=e.target.closest('[data-github-endpoint]');if(!button)return;const endpoint=catalog.find(e=>e.id===button.dataset.githubEndpoint),n=currentWorkflow().nodes.find(n=>n.id===node);if(!app.run&&currentWorkflow().id===workflow&&app.selected===node&&n?.provider==='github'&&endpoint){applyGithubEndpoint(n,endpoint);closeModal()}};
 update();search.focus();
}
function githubRequestFields(n){
 const catalog=window.githubApiCatalog||[],endpoint=catalog.find(e=>e.id===n.api_endpoint);
 const textarea=(label,name,value,placeholder='')=>`<label>${esc(label)}<textarea name="${name}" rows="4" placeholder="${esc(placeholder)}">${esc(value||'')}</textarea></label>`;
 return select('Endpoint GitHub','api_endpoint',n.api_endpoint||'',[['','Percorso personalizzato'],...catalog.map(e=>[e.id,e.id+' · '+e.method+' '+e.path+' · '+e.label])])+`<button type="button" data-action="github-api-catalog">${icon('search')}${uiText('Sfoglia e filtra operazioni')}</button>`+
 select('Metodo HTTP','api_method',n.api_method||'GET',['GET','HEAD','OPTIONS','POST','PUT','PATCH','DELETE'])+
 input('Percorso API','api_path',n.api_path||'/repos/{owner}/{repo}')+
 textarea('Parametri percorso (JSON)','api_parameters',n.api_parameters,'{"issue_number":123}')+
 textarea('Parametri query (JSON)','api_query',n.api_query,'{"per_page":100,"page":1}')+
 textarea('Header aggiuntivi (JSON)','api_headers',n.api_headers,'{"Accept":"application/vnd.github+json"}')+
 select('Formato corpo','api_encoding',n.api_encoding||'json',[['json','JSON'],['text','Testo'],['base64','Binario Base64']])+
 textarea('Corpo richiesta','api_body',n.api_body,'{"title":"Nuova issue","body":"Descrizione"}')+
 `<p class="note">Owner e repo nel percorso provengono dal nodo; completa gli altri parametri nel JSON. Per GraphQL usa POST /graphql e un corpo JSON con query e variables. Le scritture richiedono approvazione. I risultati includono gli header Link per la paginazione; ogni nodo esegue una pagina. Limite richiesta e risposta: 8 MiB.</p><a href="${esc(endpoint?.url||'https://docs.github.com/en/rest')}" target="_blank" rel="noopener noreferrer">Documentazione endpoint GitHub</a>`;
}
function connectionFields(n){
 if(n.provider==='atlassian')return connectionTargetFields(n)+`<input type="hidden" name="operation" value="twg"><label>Argomenti TWG (JSON)<textarea name="command" rows="6" placeholder='["jira","workitem","get","ABC-123"]'>${esc(n.command||'')}</textarea></label><p class="note">Usa i comandi di Jira, Confluence, Bitbucket e degli altri prodotti TWG. Il workflow richiede conferma prima di eseguire il comando.</p><button type="button" data-action="twg-setup">${icon('plug')}Configura credenziali Atlassian</button>`;
 if(n.provider==='servicenow')return select('Operazione','operation',n.operation||'check_connection',[['check_connection','Verifica connessione'],['read_ticket','Leggi incident'],['read_request','Leggi richiesta da configurazione'],['update_ticket','Aggiorna note incident'],['apply_ticket_plan','Esegui piano ITSM'],['apply_ticket_batch','Esegui lotto ITSM'],['create_incident','Crea incident da piano'],['order_catalog','Ordina articolo da piano'],['resolve_ticket','Risolvi incident verificato']])+input(n.operation==='read_request'?'File di configurazione':'sys_id incident oppure file JSON del piano','location',n.location||'')+(n.operation==='resolve_ticket'?select('Nodo di creazione incidente','target',n.target||'',[['','Unico nodo di creazione precedente'],...currentWorkflow().nodes.filter(x=>x.kind==='Connection'&&x.provider==='servicenow'&&['create_incident','apply_ticket_plan','apply_ticket_batch'].includes(x.operation)).map(x=>[x.id,x.title])]):'')+`<p class="note">Le credenziali provengono da Personal Settings → Connessioni → ServiceNow. Le scritture mostrano istanza e contenuto esatto prima della conferma. Per i piani usa un percorso relativo al workspace.</p>`;
 if(n.provider==='github'&&n.operation==='api_request')return connectionTargetFields(n)+select('Operazione','operation','api_request',['read_ticket','read_ci','create_pr',['api_request','Richiesta API GitHub']])+githubRequestFields(n);
 const operations=n.provider==='github'?['read_ticket','read_ci','create_pr','api_request']:n.provider==='azure'?['read_ticket','create_pr']:['read_ticket','test','commit'];
 return connectionTargetFields(n)+select('Operazione','operation',operations.includes(n.operation)?n.operation:operations[0],operations.map(op=>op==='api_request'?[op,'Richiesta API GitHub']:op))+input('Ticket ID / file locale / base branch','location',n.location||'');
}
function twgSetup(){settingsDialog('connections')}
function atlassianDraft(root){const f=settingsDraftData(root);return {base_url:f.get('base_atlassian')||'',username:f.get('username_atlassian')||'',token:f.get('token_atlassian')||'',repository:app.settings.connections.atlassian?.repository||''}}
async function testAtlassian(button){
 const root=$('#settings-form'),status=$('[data-atlassian-status]',root);button.disabled=true;status.textContent=uiText('Verifica connessione in corso…');
 try{const result=await api('/connections/atlassian/status',atlassianDraft(root),'POST');if(status.isConnected)status.textContent=uiText('Accesso Atlassian')+': '+uiText(result.oauth_connected?'Collegato':'Non verificato')+' · Bitbucket: '+uiText(result.bitbucket_connected===null?'Token non configurato':result.bitbucket_connected?'Collegato':'Non verificato');}
 catch(error){if(status.isConnected)status.textContent=error.message}finally{if(button.isConnected)button.disabled=false}
}
async function loginAtlassian(button){
 const root=$('#settings-form'),status=$('[data-atlassian-login-status]',root);
 if(root.settingsEditedFields.has('base_atlassian')){status.textContent=uiText('Salva prima il sito Atlassian.');return;}
 button.disabled=true;status.textContent=uiText('Avvio collegamento Atlassian…');
 try{await api('/connections/atlassian/login',{});const poll=async()=>{if(!status.isConnected)return;const result=await api('/connections/atlassian/login');if(!status.isConnected)return;
 if(result.verification_url){const url=new URL(result.verification_url);if(url.origin!=='https://auth.atlassian.com'||url.pathname!=='/oauth/activate')throw Error('URL autorizzazione non valido');status.innerHTML=`<a href="${esc(url.href)}" target="_blank" rel="noopener noreferrer">${uiText('Apri Atlassian e autorizza il collegamento')}</a>`;}
 if(['starting','waiting'].includes(result.state)){setTimeout(()=>poll().catch(error=>{if(status.isConnected){status.textContent=error.message;button.disabled=false}}),2000);return;}
 status.textContent=uiText(result.state==='connected'?'Account Atlassian collegato.':'Collegamento non completato. Riprova e verifica i permessi.');button.disabled=false;};await poll();}
 catch(error){if(status.isConnected)status.textContent=error.message;if(button.isConnected)button.disabled=false}
}
async function twgCheck(button){
 const output=$('#twg-status');button.disabled=true;output.textContent='Verifica collegamento…';
 try{const s=await api('/connections/atlassian/status');if(output.isConnected)output.textContent=!s.available?'TWG CLI non disponibile. Ricostruisci il container.':s.connected?'Collegamento Atlassian verificato.':'Completa il login e verifica i permessi Atlassian, poi riprova.'}
 catch(e){if(output.isConnected)output.textContent=e.message}finally{if(button.isConnected)button.disabled=false}
}
function serviceHeading(id,configured,isAgent,isLocal=false,isMulti=false){const info=serviceInfo[id],status=configured?(isMulti?'Provider disponibili':isLocal?'Endpoint configurato':isAgent?'Credenziale configurata':'Token salvato'):'Da configurare';return `<header class="service-heading"><span class="service-mark service-${id} ${info.text?'has-service-text':''}">${serviceIdentity(id)}</span><div class="service-title"><h3>${esc(info.name)}</h3><p>${esc(info.description)}</p></div><span class="service-status ${configured?'is-configured':''}" data-tip="${esc(status)}"><i aria-hidden="true"></i>${esc(status)}</span></header>`}
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
  menu.innerHTML=`<div class="account-version"><span class="account-version-number" data-no-i18n>rightcommit v${esc(authInfo.version||'')}</span><a class="account-site-link" role="menuitem" href="https://rightcommit.com" target="_blank" rel="noopener noreferrer" aria-label="${esc(uiText('Apri il sito rightcommit.com in una nuova scheda'))}" title="${esc(uiText('Apri il sito rightcommit.com in una nuova scheda'))}">${icon('external-link')}</a></div><hr role="separator"><button type="button" role="menuitem" data-action="settings">${icon('settings-2')}<span>Configurazioni personali</span></button>${app.user?.admin?`<button type="button" role="menuitem" data-action="manage-templates">${icon('workflow')}<span>Gestisci template</span></button>`:''}<hr role="separator"><button type="button" role="menuitem" data-action="logout" class="account-signout">${icon('log-out')}<span>Esci</span></button>`;
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
function piProviderOptions(){return (app.settings.pi_providers||[]).map(provider=>[provider.id,provider.label])}
function selectedPiProvider(form){const value=$('[name=provider_pi]',form).value;return value==='multi'?$('[name=pi_default_provider]',form).value:value}
function piNodeFields(node){
 const runtime=node.runtime||app.settings.default_runtime;
 if(runtime!=='pi'||!app.settings.agents.pi?.multi_provider)return '';
 const provider=node.pi_provider||app.settings.agents.pi.provider;
 return select('Provider Pi','pi_provider',node.pi_provider||'',[['','Provider personale predefinito'],...piProviderOptions()])+(provider==='local'?input('URL locale (vuoto = .env)','pi_base_url',node.pi_base_url||'','url','PI_LOCAL_BASE_URL'):'')+'<p class="note">Il nodo usa le credenziali e i parametri del provider dalla API key personale, dalla chiave dell’amministratore o da .env. Specifica il modello esatto; le chiavi non vengono salvate nel workflow.</p>';
}
async function settingsDialog(activeTab='general'){
  app.settings=await api('/settings');const s=app.settings;
  let body=`<div id="settings-form" data-settings-root><label class="settings-appearance">Aspetto<jui-select data-appearance-theme aria-label="Aspetto">${[['light','Chiaro'],['dark','Scuro'],['system','Sistema']].map(([value,label])=>`<jui-option value="${value}" ${appearanceTheme===value?'selected':''}>${label}</jui-option>`).join('')}</jui-select></label><div class="tabs settings-tabs" role="tablist" aria-label="Configurazioni personali">${[['general','Generali'],['execution','Esecuzione'],['agents','Agenti'],['manager','Project manager'],['compaction','Compattazione'],['connections','Connessioni'],['mcp','MCP'],['sources','Sorgenti /sources'],['activity','Registro attività'],['environment','Environment'],['cli','Check CLI'],...(app.user.admin?[['admin','Admin']]:[])].map(([id,label],i)=>`<button type="button" role="tab" id="settings-tab-${id}" data-settings-tab="${id}" aria-controls="settings-panel-${id}" aria-selected="${i===0}" tabindex="${i===0?0:-1}" class="${i===0?'active':''} ${['connections','activity','admin'].includes(id)?'settings-nav-break':''}">${label}</button>`).join('')}</div><section id="settings-panel-agents" class="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-agents"><p class="note">Seleziona la configurazione Default per la chat del manager e per gli agenti che usano le preferenze personali.</p><h3 class="settings-group-title">Agenti</h3>`;
  for(const rt of ['codex','claude','pi']){
    const a=s.agents[rt]||{};
    body+=`<section class="settings-section" data-service="${rt}">${serviceHeading(rt,!!a.configured,true,rt==='pi'&&a.provider==='local',rt==='pi'&&a.multi_provider)}<label class="settings-default"><span>Default</span><jui-switch exclusive aria-label="Default ${esc(serviceInfo[rt].name)}" name="default_runtime" value="${rt}" ${s.default_runtime===rt?'checked':''} required></jui-switch></label><div class="form-grid">${input('Modello','model_'+rt,a.model||'','text',({codex:'gpt-6.1-sol',claude:'opus'})[rt]||'')}${select('Ragionamento','effort_'+rt,a.effort||'low',['low','medium','high','xhigh'])}${credentialField('API key personale','key_'+rt,'Rimuovi credenziale salvata',a.api_key_preview)}<p class="note">${uiText(a.uses_admin_api_key?'API key dell’amministratore in uso. Imposta una chiave personale per sostituirla.':'Se non imposti una chiave personale, viene usata quella dell’amministratore per lo stesso agente e provider, se disponibile.')}</p>${rt==='pi'?select('Provider Pi','provider_pi',a.multi_provider?'multi':(a.provider||'anthropic'),[['multi','Multi-provider'],null,...piProviderOptions().flatMap(option=>option[0]==='local'?[null,option]:[option])]):''}</div>${rt==='pi'?`<div data-pi-multi-default ${a.multi_provider?'':'hidden'}>${select('Provider predefinito Pi','pi_default_provider',a.provider||'anthropic',piProviderOptions())}<p class="note">In modalità Multi-provider ogni nodo Pi può scegliere il proprio provider. Il provider e il modello predefiniti si applicano alla chat del manager, al supervisore e ai nodi che ereditano le preferenze personali. Le credenziali provengono dalla chiave dell’amministratore per lo stesso provider o da .env.</p></div><div data-pi-local-url ${a.provider==='local'?'':'hidden'}>${input('URL del server locale','base_url_pi',a.base_url||'','url','http://host.docker.internal:11434/v1')}</div><p data-pi-local-note class="note" ${a.provider==='local'?'':'hidden'}>Inserisci l’URL completo dell’API compatibile OpenAI, incluso /v1 se previsto dal server, e il nome esatto del modello. Lascia la chiave vuota se il server non richiede autenticazione. Dal container, localhost indica il container stesso.</p><p class="note">Per avviare Pi scegli un modello e configura le credenziali del provider. Lo stato indica la presenza di credenziali, senza verificarle online.</p><button type="button" data-action="pi-check">${icon('plug')}Verifica connessione</button><p data-pi-status role="status" aria-live="polite"></p><p class="note">La verifica invia una breve richiesta al modello usando i valori presenti nei campi. Non salva le impostazioni.</p><label class="settings-switch"><span>Abilita codemode</span><jui-switch name="codemode_pi" aria-label="Abilita codemode" ${a.codemode?'checked':''}></jui-switch></label><p class="note">Nei Task il modello può scrivere script JavaScript che chiamano gli altri strumenti, anche in parallelo, e filtrano l’output prima che entri nel contesto. Gli script girano in una sandbox senza rete né file system; ogni chiamata agli strumenti compare nell’Activity Log.</p>`:''}${rt!=='pi'&&app.user.id!==1?`<p class="note">${uiText('Per Claude Code e Codex le configurazioni provider da ambiente sono riservate all’amministratore.')}</p>`:''}${compactionFields(rt,s.compaction?.[rt]||{})}</section>`;
  }
  body+=`<div class="settings-execution-limits"><section class="settings-section"><h3>Timeout del turno</h3><p class="note">Tempo massimo per un nodo che usa un agente. Ogni nuovo tentativo ha il proprio limite.</p><label>Timeout turno in secondi<input name="timeout" type="number" min="1" aria-describedby="timeout-duration" step="1" required value="${s.task_timeout_secs}"></label><p id="timeout-duration" class="small" aria-live="polite" hidden></p></section><section class="settings-section"><h3>Workflow paralleli</h3><p class="note">Numero massimo di workflow eseguiti contemporaneamente dal tuo account. Gli altri attendono in coda. Ridurre il limite non interrompe quelli già attivi.</p><label>Workflow paralleli per utente<input name="workflow_parallelism" type="number" min="1" step="1" required value="${s.workflow_parallelism??5}"></label></section></div>${compactionSessionPanel()}</section><section id="settings-panel-mcp" class="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-mcp" hidden>${mcpSettingsContent(s)}<div class="settings-mcp-actions"><button type="button" class="primary" data-action="mcp-edit">${icon('plus')}Aggiungi server MCP</button><button type="button" data-action="mcp-example">${icon('folder-git-2')}Esempio locale</button></div></section><section id="settings-panel-connections" class="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-connections" hidden>`;
  for(const p of ['github','azure','servicenow']){
    const c=s.connections[p]||{};
    if(p==='servicenow'){
      body+=`<section class="settings-section" data-service="servicenow">${serviceHeading(p,!!c.configured,false)}<div class="form-grid"><div class="service-url">${input('URL istanza ServiceNow','base_servicenow',c.base_url||'','url','https://dev123456.service-now.com')}</div>${select('Autenticazione','sn_auth',c.authentication||(c.configured?'bearer':'basic'),[['basic','Utente e password PDI'],['client_credentials','OAuth · Client Credentials'],['bearer','Bearer token manuale']])}</div><div class="form-grid" data-sn-auth="basic">${input('Utente dell’istanza','sn_username',c.username||'')}${credentialField('Password dell’istanza','sn_password','Rimuovi password salvata')}</div><fieldset class="servicenow-oauth" data-sn-auth="client_credentials"><legend>OAuth</legend><p class="note">rightcommit ottiene il token da ServiceNow e ne richiede uno nuovo prima della scadenza, anche per i workflow schedulati.</p><div class="form-grid">${input('Client ID','sn_client_id',c.client_id||'')}${credentialField('Client Secret','sn_client_secret','Rimuovi Client Secret salvato')}${input('Scope OAuth (opzionale)','sn_oauth_scope',c.oauth_scope||'')}</div><p class="note">Endpoint token: URL dell’istanza + /oauth_token.do. Client Credentials deve essere abilitato sulla PDI con un utente applicativo autorizzato.</p></fieldset><div data-sn-auth="bearer">${credentialField('Token OAuth','token_servicenow','Rimuovi token salvato')}<p class="note">Inserisci il solo access token, senza Bearer. Questa modalità non rinnova automaticamente i token.</p></div><p class="note">Le credenziali sono cifrate e non vengono passate agli agenti. Lascia vuoto un campo segreto per conservarlo; usa il cestino per rimuoverlo. La verifica usa i valori inseriti senza salvarli.</p><button type="button" data-action="servicenow-check">Verifica connessione</button><p data-servicenow-status role="status" aria-live="polite"></p></section>`;
      continue;
    }
    body+=`<section class="settings-section" data-service="${p}">${serviceHeading(p,!!c.configured,false)}<div class="form-grid"><div class="service-url">${input('Base URL HTTPS','base_'+p,c.base_url||(p==='github'?'https://api.github.com':''),'url')}</div>${credentialField('Token personale','token_'+p,'Rimuovi token salvato')}<p class="note">Organizzazione, progetto e repository si configurano nel nodo Connection.</p></div></section>`;
  }
  const atl=s.connections.atlassian||{};
  body+=`<section class="settings-section" data-service="atlassian"><header class="service-heading"><span class="service-mark has-service-text">${serviceIdentity('atlassian')}</span><div class="service-title"><h3>Atlassian</h3><p>Jira, Confluence, Bitbucket e altri prodotti Atlassian</p></div></header><h4>Jira, Confluence e altri prodotti</h4><div class="form-grid">${input('URL sito Atlassian','base_atlassian',atl.base_url||'','url','https://your-site.atlassian.net')}</div><p class="note">Salva il sito, poi collega il tuo account dal browser. I permessi concessi determinano quali prodotti puoi usare.</p><button type="button" data-action="twg-login">Collega account Atlassian</button><div data-atlassian-login-status role="status" aria-live="polite"></div><h4>Bitbucket</h4><p class="note">Bitbucket usa un token API separato. Inserisci l’email del tuo account Atlassian e il token con gli ambiti necessari.</p><div class="form-grid">${input('Email account Atlassian','username_atlassian',atl.username||'','email')}${credentialField('Token API Bitbucket','token_atlassian','Rimuovi token Bitbucket salvato')}</div><p>${uiText(atl.configured?'Token Bitbucket salvato':'Token Bitbucket non configurato')}</p><p class="note">Le credenziali sono cifrate e non vengono passate agli agenti. Lascia vuoto un campo segreto per conservarlo; usa il cestino per rimuoverlo. La verifica usa i valori inseriti senza salvarli.</p><button type="button" data-action="atlassian-check">Test connection</button><p data-atlassian-status role="status" aria-live="polite"></p></section>`;
  body+=`<section class="settings-section"><h3>Sorgenti /sources</h3><label>Limite copia sorgenti (MiB)<input name="source_copy_limit_mib" type="number" min="1" max="17592186044415" step="1" required value="${s.source_copy_limit_mib??10}"></label><p class="note">Vale sia come limite per file sia come soglia dell’avviso per cartella, solo per le copie da /sources. La modifica permanente e i repository remoti non hanno questa soglia.</p></section>`;
  body+=`</section><section id="settings-panel-activity" class="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-activity" hidden><section class="settings-section"><h3>Registro attività</h3><label>Esecuzioni per pagina<input name="activity_log_page_size" type="number" min="1" max="100" step="1" required value="${s.activity_log_page_size??10}"></label><p class="note">Da 1 a 100; valore predefinito 10. Il registro mostra le ultime 100 esecuzioni del workflow, oltre all’esecuzione aperta se precedente.</p>${select('Verbosità','activity_log_verbosity',s.activity_log_verbosity||'preview',[['preview','Primi 30 caratteri'],['full','Testo completo']])}<p class="note">L’anteprima abbrevia a 30 caratteri solo i dettagli dei tool. Errori e risposte dell’agente restano completi. La cronologia salvata resta invariata.</p></section></section><p id="settings-error" class="error" role="alert"></p></div><form id="password-form" hidden></form>${app.user.admin?'<form id="user-form" hidden></form>':''}`;
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
  const manager=addPanel('manager');
  manager.innerHTML=`<section class="settings-section"><h3>Project manager</h3><label class="settings-switch"><span>Abilita alla scrittura ed azioni dispositive</span><jui-switch name="manager_write_enabled" aria-label="Abilita alla scrittura ed azioni dispositive" ${s.manager_write_enabled?'checked':''}></jui-switch></label><p class="note">Attivo per default. Abilita i tool di scrittura, modifica file e comandi shell del Project manager. Dopo il salvataggio si applica dal messaggio successivo, anche nelle chat esistenti.</p></section>`;
  addPanel('environment');addPanel('cli');
  general.innerHTML=`<section class="settings-section account-summary"><h3>Account personale</h3><p class="account-email" data-no-i18n>${esc(app.user.email)}</p><p class="account-user-id"><span>ID utente</span>: <span data-no-i18n>${esc(app.user.id)}</span></p><p class="note">I tuoi workflow, esecuzioni e configurazioni sono separati dagli altri account.</p></section>`;
  general.append($('.settings-appearance',form));
  const notices=document.createElement('section');notices.className='settings-section';
  notices.innerHTML=`<h3>Avvisi nel browser</h3><label class="settings-switch"><span>Mostra l’avviso degli agenti non configurati</span><jui-switch data-agent-setup-enabled aria-label="Mostra l’avviso degli agenti non configurati" ${agentSetupNoticeHidden()?'':'checked'}></jui-switch></label><p class="note">La preferenza si salva subito e vale solo per questo browser.</p>`;
  general.append(notices);
  const agentNotice=$('[data-agent-setup-enabled]',notices);
  agentNotice.addEventListener('change',()=>{if(!setAgentSetupNoticeHidden(!agentNotice.checked))agentNotice.checked=!agentSetupNoticeHidden()});
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
  splitSettingsForms(form);
  trackSettingsEdits(form);
  form.settingsRevision=s._revision;
  for(const name of ['provider_pi','pi_default_provider'])$('[name='+name+']',form).addEventListener('change',()=>{syncPiLocalFields(form);loadPiModels(form)});
  $('[name=sn_auth]',form).addEventListener('change',()=>syncServiceNowFields(form));
  syncServiceNowFields(form);
  syncPiLocalFields(form);
  loadPiModels(form);
  loadCompactRuns();
}
function syncPiLocalFields(form){
 const multi=$('[name=provider_pi]',form).value==='multi',local=selectedPiProvider(form)==='local',field=$('[name=base_url_pi]',form);
 $('[data-pi-multi-default]',form).hidden=!multi;
 const key=$('[name=key_pi]',form),supportsKey=(app.settings.pi_providers||[]).find(provider=>provider.id===selectedPiProvider(form))?.key_env;key.disabled=multi||!supportsKey;key.closest('.credential-field').hidden=key.disabled;
 $('[data-pi-local-url]',form).hidden=!local;$('[data-pi-local-note]',form).hidden=!local;
 field.disabled=!local;field.required=local&&!multi&&!app.settings.pi_local_endpoint_available;
}
async function loadPiModels(form){
  const field=$('[name=model_pi]',form),provider=selectedPiProvider(form);
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
 const methods={external_configuration:'Configurazione esterna',api_key:'API key',bearer_token:'Token Bearer',auth_headers:'Header di autenticazione',native_provider:'Autenticazione del provider nativo',aws_credentials:'Credenziali AWS',google_credentials:'Credenziali Google',azure_credentials:'Credenziali Azure',not_configured:'Non configurato',configuration_error:'Configurazione da verificare'};
 const providers={openai:'OpenAI',anthropic:'Anthropic',gateway:'Gateway Anthropic',bedrock:'Amazon Bedrock',bedrock_mantle:'Amazon Bedrock Mantle',anthropic_aws:'Claude Platform on AWS',vertex:'Google Vertex',foundry:'Microsoft Foundry',unknown:'Provider da verificare'};
 const sources={personal_settings:'Configurazioni personali',administrator_api_key:'API key dell’amministratore',external_configuration:'Configurazione esterna',native_credentials:'Credenziali native della CLI',native_config:'Configurazione nativa della CLI',none:'Nessuna credenziale selezionata',conflicting_providers:'Provider cloud in conflitto'};
 return `<h3>${uiText('Autenticazione agenti')}</h3><p class="note">${uiText('Configurazione salvata per questo utente. La validità delle credenziali non viene verificata online.')}</p><div class="environment-auth-cards">${['claude','codex'].map(runtime=>{
  const auth=authentication?.[runtime];if(!auth)return '';

  const source=sources[auth.source]?esc(uiText(sources[auth.source])):`<code data-no-i18n>${esc(auth.source)}</code>`;
  return `<article class="environment-auth-card" data-auth-runtime="${runtime}"><h4>${runtime==='claude'?'Claude Code':'Codex'}</h4><p class="environment-auth-method">${esc(uiText(methods[auth.method]||'Configurazione da verificare'))}</p><p class="note"><span>${uiText('Provider')}</span>: <span data-no-i18n>${esc(providers[auth.provider]||auth.provider)}</span></p><p class="note"><span>${uiText('Origine')}</span>: ${source}</p></article>`;
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
function piConnectionDraft(form){const data=new FormData(form);return {provider:selectedPiProvider(form)||'anthropic',multi_provider:data.get('provider_pi')==='multi',model:data.get('model_pi')||'',api_key:data.get('key_pi')||'',base_url:data.get('base_url_pi')||''}}
async function checkPiConnection(button){
 const form=button.closest('form'),status=$('[data-pi-status]',form),draft=piConnectionDraft(form);
 status.textContent=uiText('Verifica connessione Pi in corso…');status.classList.remove('error');button.disabled=true;
 try{
  const result=await api('/agents/pi/status',draft,'POST');
  if(!status.isConnected)return;
  if(JSON.stringify(piConnectionDraft(form))!==JSON.stringify(draft)){status.textContent=uiText('I campi Pi sono cambiati durante la verifica. Ripeti la verifica.');return}
  if(result.connected)for(const name of ['provider_pi','model_pi','key_pi','base_url_pi'])form.closest('[data-settings-root]')?.settingsEditedFields?.add(name);
  status.textContent=uiText(result.message);
 }catch(error){if(status.isConnected){status.textContent=error.message;status.classList.add('error')}}finally{if(button.isConnected)button.disabled=false}
}
function syncServiceNowFields(form){
 const method=$('[name=sn_auth]',form).value;
 for(const group of $$('[data-sn-auth]',form))group.hidden=group.dataset.snAuth!==method;
}
function serviceNowDraft(form){
 const f=new FormData(form);
 return {base_url:f.get('base_servicenow')||'',authentication:f.get('sn_auth')||'bearer',username:f.get('sn_username')||'',password:f.get('sn_password')||'',token:f.get('token_servicenow')||'',client_id:f.get('sn_client_id')||'',client_secret:f.get('sn_client_secret')||'',oauth_scope:f.get('sn_oauth_scope')||''};
}
function splitSettingsForms(root){
 let index=0;
 for(const section of root.querySelectorAll('.settings-section')){
  const fields=[...section.querySelectorAll('input,textarea,jui-select,jui-switch')].filter(field=>field.name&&!field.getAttribute('form')&&!field.closest('form'));
  if(!fields.length)continue;
  const form=document.createElement('form');form.id='settings-'+(section.dataset.service||section.closest('[role=tabpanel]')?.id||'section')+'-'+index++;
  form.className='settings-service-form';form.autocomplete='off';form.style.display='contents';
  form.onsubmit=event=>{event.preventDefault();saveSettings()};
  while(section.firstChild)form.append(section.firstChild);
  section.append(form);
 }
}
function settingsFormData(root){
 if(root instanceof HTMLFormElement)return new FormData(root);
 const data=new FormData();
 for(const form of root.querySelectorAll('form.settings-service-form'))for(const [name,value] of new FormData(form))data.append(name,value);
 return data;
}
function trackSettingsEdits(form){
 form.settingsEditedFields=new Set();
 form.settingsInitialData=settingsFormData(form);
 for(const field of form.querySelectorAll('input,textarea,jui-select,jui-switch')){
  if(!field.name||!field.form||!(field.form===form||form.contains(field.form))||!field.matches('input,textarea'))continue;
  if(!['checkbox','radio'].includes(field.type))form.settingsInitialData.set(field.name,field.defaultValue);
  const service=field.closest('[data-service]')?.dataset.service||field.form.id;
  field.id=field.form.id+'-'+field.name;
  field.autocomplete=field.type==='password'?`section-rightcommit-${service} new-password`:'off';
  field.setAttribute('data-lpignore','true');field.setAttribute('data-1p-ignore','true');field.setAttribute('data-bwignore','true');
  if(field.type==='password')field.setAttribute('autocapitalize','off');
 }
 if(form.matches('[data-settings-root]')){
 for(const rt of ['codex','claude','pi']){
  form.settingsInitialData.set('model_'+rt,app.settings.agents[rt]?.model||'');
  form.settingsInitialData.set('key_'+rt,'');
 }
 for(const provider of ['github','azure','servicenow'])form.settingsInitialData.set('token_'+provider,'');
 form.settingsInitialData.set('sn_password','');
 form.settingsInitialData.set('sn_client_secret','');
 form.settingsInitialData.set('sn_client_id',app.settings.connections.servicenow?.client_id||'');
 form.settingsInitialData.set('sn_oauth_scope',app.settings.connections.servicenow?.oauth_scope||'');
 form.settingsInitialData.set('sn_username',app.settings.connections.servicenow?.username||'');
 form.settingsInitialData.set('base_servicenow',app.settings.connections.servicenow?.base_url||'');
 }
 const edited=e=>{
  const field=e.target;
  if(!field.form||!(field.form===form||form.contains(field.form))||!field.name)return;
  // Ignore background autofill into other fields, including hidden tabs.
  if(document.activeElement===field||field.matches(':focus-within'))form.settingsEditedFields.add(field.name);
 };
 form.addEventListener('input',edited);form.addEventListener('change',edited);
}
function settingsDraftData(form){
 const data=settingsFormData(form),initial=form.settingsInitialData;
 if(!initial)return data;
 for(const name of new Set([...data.keys(),...initial.keys()])){
  if(form.settingsEditedFields.has(name))continue;
  data.delete(name);for(const value of initial.getAll(name))data.append(name,value);
 }
 return data;
}
async function saveSettingsPatch(changes,revision){return api('/settings',{revision,changes},'PATCH')}
function settingsChanges(root,next){
 const changes={};
 const put=path=>{let target=changes,source=next;for(const key of path.slice(0,-1)){target=target[key]??={};source=source[key]}target[path.at(-1)]=source[path.at(-1)]};
 const global={manager_write_enabled:'manager_write_enabled',default_runtime:'default_runtime',timeout:'task_timeout_secs',workflow_parallelism:'workflow_parallelism',source_copy_limit_mib:'source_copy_limit_mib',file_preview_limit_mb:'file_preview_limit_mb',activity_log_page_size:'activity_log_page_size',activity_log_verbosity:'activity_log_verbosity'};
 const sn={sn_auth:'authentication',sn_username:'username',sn_password:'password',sn_client_id:'client_id',sn_client_secret:'client_secret',sn_oauth_scope:'oauth_scope'};
 for(const name of root.settingsEditedFields){
  let match;
  if(name==='provider_pi'){put(['agents','pi','multi_provider']);put(['agents','pi','provider'])}
  else if(name==='pi_default_provider')put(['agents','pi','provider']);
  else if(global[name])put([global[name]]);
  else if(sn[name])put(['connections','servicenow',sn[name]]);
  else if((match=name.match(/^(model|effort|key|provider|codemode|base_url)_(codex|claude|pi)$/)))put(['agents',match[2],({key:'api_key'})[match[1]]||match[1]]);
  else if((match=name.match(/^(base|token|repo|project|username)_(github|azure|servicenow|atlassian)$/)))put(['connections',match[2],({base:'base_url',repo:'repository'})[match[1]]||match[1]]);
  else if((match=name.match(/^compact_(codex|claude|pi)_(.+)$/)))put(['compaction',match[1],match[2]]);
 }
 return changes;
}
async function saveSettings(){const root=$('#settings-form');if(root.settingsSaving)return;for(const field of root.querySelectorAll('[name]'))if(root.settingsEditedFields.has(field.name)&&field.reportValidity&&!field.reportValidity())return;const f=settingsDraftData($('#settings-form')),s=structuredClone(app.settings);s.manager_write_enabled=f.has('manager_write_enabled');s.activity_log_verbosity=f.get('activity_log_verbosity');s.activity_log_page_size=Number(f.get('activity_log_page_size'));s.default_runtime=f.get('default_runtime');s.task_timeout_secs=Number(f.get('timeout'));s.workflow_parallelism=Number(f.get('workflow_parallelism'));s.compaction={};for(const rt of ['claude','codex','pi']){s.compaction[rt]={...(app.settings.compaction?.[rt]||{})};for(const field of ['context_window_tokens','threshold_tokens','trigger_percent','reserve_tokens','keep_recent_tokens'])if(f.has('compact_'+rt+'_'+field))s.compaction[rt][field]=Number(f.get('compact_'+rt+'_'+field));}s.compaction.claude.context_window_tokens=0;s.compaction.claude.threshold_tokens=0;s.source_copy_limit_mib=Number(f.get('source_copy_limit_mib'));s.file_preview_limit_mb=Number(f.get('file_preview_limit_mb'));for(const rt of ['codex','claude','pi'])s.agents[rt]={multi_provider:rt==='pi'&&f.get('provider_pi')==='multi',base_url:rt==='pi'?(f.get('base_url_pi')||''):(s.agents[rt]?.base_url||''),model:f.get('model_'+rt),effort:f.get('effort_'+rt),api_key:f.get('key_'+rt),provider:rt==='pi'?(f.get('provider_pi')==='multi'?f.get('pi_default_provider'):f.get('provider_pi')):'anthropic',codemode:rt==='pi'&&f.has('codemode_pi')};for(const p of ['github','azure','servicenow'])s.connections[p]={base_url:f.get('base_'+p),token:f.get('token_'+p),repository:s.connections[p]?.repository||'',project:s.connections[p]?.project||''};s.connections.atlassian={...(s.connections.atlassian||{}),base_url:f.get('base_atlassian')||'',username:f.get('username_atlassian')||'',token:f.get('token_atlassian')||'',repository:app.settings.connections.atlassian?.repository||''};Object.assign(s.connections.servicenow,{authentication:f.get('sn_auth'),username:f.get('sn_username'),password:f.get('sn_password'),client_id:f.get('sn_client_id'),client_secret:f.get('sn_client_secret'),oauth_scope:f.get('sn_oauth_scope')});const activeTab=$('[data-settings-tab][aria-selected="true"]')?.dataset.settingsTab||'agents';try{root.settingsSaving=true;app.settings=await saveSettingsPatch(settingsChanges(root,s),root.settingsRevision);render();await settingsDialog(activeTab);window.dispatchEvent(new Event('settings-saved'));toast('Configurazioni personali salvate')}catch(e){if(root.isConnected)$('#settings-error').textContent=e.message}finally{root.settingsSaving=false}}
function filePreviewMessage(data){return data.preview_reason==='size'?uiText('Il file supera il limite di anteprima:')+' '+data.preview_limit_mb+' MB. '+uiText('Puoi modificarlo in Configurazioni personali o scaricare il file.'):uiText('Anteprima non disponibile per questo formato. Puoi scaricare il file.')}
async function artifact(id,mode='preview'){
 const d=await api('/artifacts/'+id),a=d.artifact,versions=app.artifacts.filter(x=>artifactKey(x)===artifactKey(a)).sort((a,b)=>a.version-b.version),preview=a.file?.preview!==false;
 let content=preview?(mode==='source'?`<pre class="source-view">${esc(a.content)}</pre>`:a.path.endsWith('.md')?`<div class="markdown">${d.html}</div>`:`<pre class="source-view">${esc(a.content)}</pre>`):`<p class="note">${esc(filePreviewMessage(d))}</p>`;
 if(mode==='diff'&&preview){const previous=versions.filter(x=>x.version<a.version).at(-1);if(previous){const b=(await api('/artifacts/'+previous.id)).artifact;if(b.file?.preview===false)content=`<p class="note">${uiText('Confronto non disponibile per questa versione.')}</p>`;else{const before=b.content.split('\n'),after=a.content.split('\n');let lines='';for(let i=0;i<Math.max(before.length,after.length);i++){if(before[i]===after[i])lines+='  '+esc(after[i]||'')+'\n';else{if(before[i]!==undefined)lines+=`<span class="diff-del">- ${esc(before[i])}</span>`;if(after[i]!==undefined)lines+=`<span class="diff-add">+ ${esc(after[i])}</span>`;}}content=`<p class="note">${uiText('Confronto per riga')} · v${b.version} → v${a.version}</p><pre class="source-view">${lines}</pre>`}}else content=`<p>${uiText('Nessuna versione precedente.')}</p>`}
 modal(a.path,`<div class="artifact-toolbar"><span class="artifact-origin" data-no-i18n>${artifactRoot(a).slice(1)}</span><jui-select id="artifact-version" aria-label="Versione">${versions.map(v=>`<jui-option value="${v.id}" ${v.id===a.id?'selected':''}>${uiText('Versione')} ${v.version} · ${esc(artifactDate(v))}</jui-option>`).join('')}</jui-select>${preview?`<div class="tabs">${['preview','source','diff'].map(m=>`<button class="${mode===m?'active':''}" data-action="artifact-mode" data-id="${a.id}" data-mode="${m}">${icon(m==='preview'?'eye':m==='source'?'code':'git-compare-arrows')}${uiText(m==='preview'?'Anteprima':m==='source'?'Sorgente':'Confronto')}</button>`).join('')}</div>`:''}</div>${a.file?.location?`<label>${uiText('Percorso completo')}<input id="artifact-location" data-no-i18n readonly value="${esc(a.file.location)}"></label>`:''}${content}`,`<a id="artifact-download" class="artifact-download" href="/api/artifacts/${encodeURIComponent(a.id)}/download" download data-no-i18n>${icon('download')}Download</a><button class="primary" data-action="close-dialog">${uiText('Chiudi')}</button>`,true);
 const location=$('#artifact-location');if(location)location.onclick=()=>location.select();
}

// Render the exact approved snapshot, including approvals already persisted by older versions.
function serviceNowApproval(question,p){
 if(p.kind!=='effect'||!app.run?.workflow.nodes.some(n=>n.id===p.node&&n.kind==='Connection'&&n.provider==='servicenow'))return null;
 const match=question.match(/^Autorizzare [a-z_]+ su servicenow\? (\{[\s\S]*\}) Controlla target e artefatti nelle impostazioni e nel run\.$/);
 if(!match)return null;
 let effect;try{effect=JSON.parse(match[1])}catch{return null}
 if(!effect||typeof effect.instance!=='string'||typeof effect.method!=='string'||typeof effect.path!=='string'||!effect.body||typeof effect.body!=='object'||Array.isArray(effect.body))return null;
 const create=effect.method==='POST'&&effect.path==='/api/now/table/incident';
 const update=effect.method==='PATCH'&&/^\/api\/now\/table\/incident\/[a-zA-Z0-9]+$/.test(effect.path);
 const resolve=update&&['6',6].includes(effect.body.state);
 const catalog=effect.method==='POST'&&/^\/api\/sn_sc\/servicecatalog\/items\/[a-zA-Z0-9]+\/order_now$/.test(effect.path);
 const title=create?'Crea un incidente su ServiceNow':resolve?'Registra la risoluzione dell’incidente':update?'Aggiorna un incidente su ServiceNow':catalog?'Invia una richiesta al catalogo ServiceNow':effect.method==='NONE'?'Restituisci la risposta KB':'Esegui un’azione su ServiceNow';
 const explanation=create?'Approvando verrà creato un nuovo incidente con i campi riportati qui sotto.':resolve?'Approvando verrà registrata la risoluzione dell’incidente indicato.':update?'Approvando verranno aggiornati i campi dell’incidente indicato.':catalog?'Approvando verrà ordinato l’articolo di catalogo indicato, con le variabili riportate qui sotto.':effect.method==='NONE'?'Il piano restituisce la risposta della KB senza scrivere su ServiceNow.':'Approvando verrà eseguita la richiesta riportata nei dettagli.';
 const labels={short_description:'Descrizione breve',description:'Descrizione',work_notes:'Note interne',impact:'Impatto',urgency:'Urgenza',state:'Stato',close_code:'Codice di risoluzione',close_notes:'Note di risoluzione',correlation_id:'Riferimento di correlazione',sysparm_quantity:'Quantità',variables:'Variabili del catalogo',answer:'Risposta'};
 const value=(v,depth=0)=>{
  if(v&&typeof v==='object'){
   if(depth>=8)return `<details><summary>${esc(uiText('Altri dettagli'))}</summary><pre data-no-i18n>${esc(JSON.stringify(v,null,2))}</pre></details>`;
   if(Array.isArray(v))return `<ol>${v.map(x=>`<li>${value(x,depth+1)}</li>`).join('')}</ol>`;
   return `<dl class="approval-fields">${Object.entries(v).map(([k,x])=>`<div><dt ${labels[k]?'':'data-no-i18n'}>${esc(labels[k]?uiText(labels[k]):k)}</dt><dd>${value(x,depth+1)}</dd></div>`).join('')}</dl>`;
  }
  return `<span data-no-i18n class="approval-value">${esc(v===null?'null':String(v))}</span>`;
 };
 const node=app.run.workflow.nodes.find(n=>n.id===p.node);
 if(effect.method==='BATCH'&&Array.isArray(effect.body.items)&&effect.body.items.every(item=>item&&typeof item.request_id==='string'&&typeof item.category==='string'&&typeof item.action==='string')){
  const actions={create_incident:'Crea o recupera incidente',order_catalog:'Ordina o recupera richiesta',answer_only:'Rispondi dalla KB'};
  return `<section class="approval-summary" aria-label="${esc(uiText('Riepilogo dell’azione'))}"><h3>${icon('plug')}${esc(uiText('Elabora lotto ITSM'))}</h3><p>${esc(uiText('Ogni ticket viene elaborato separatamente. Quelli già registrati vengono riutilizzati; gli esiti incerti richiedono verifica e non vengono reinviati.'))}</p><dl class="approval-target"><div><dt>${esc(uiText('Istanza ServiceNow'))}</dt><dd data-no-i18n>${esc(effect.instance)}</dd></div><div><dt>${esc(uiText('Namespace stabile del lotto'))}</dt><dd data-no-i18n>${esc(effect.body.namespace)}</dd></div><div><dt>${esc(uiText('Ticket nel lotto'))}</dt><dd data-no-i18n>${effect.body.items.length}</dd></div></dl>${effect.body.items.map(item=>`<article class="approval-batch-item"><h4 data-no-i18n>${esc(item.request_id)} · ${esc(item.category)}</h4><b>${esc(uiText(actions[item.action]||item.action))}</b>${item.status==='blocked'?`<p class="manager-warning">${esc(uiText('Da configurare prima dell’invio'))}: <span>${esc(uiText(item.message||''))}</span></p>`:value(item.effect?.body||{})}</article>`).join('')}<details class="approval-json"><summary>${esc(uiText('Dettagli tecnici · JSON completo'))}</summary><pre data-no-i18n>${esc(JSON.stringify(effect,null,2))}</pre></details></section>`;
 }
 return `<section class="approval-summary" aria-label="${esc(uiText('Riepilogo dell’azione'))}"><h3>${icon('plug')}${esc(uiText(title))}</h3><p>${esc(uiText(explanation))}</p><dl class="approval-target"><div><dt>${esc(uiText('Istanza ServiceNow'))}</dt><dd data-no-i18n>${esc(effect.instance)}</dd></div>${update||catalog?`<div><dt>${esc(uiText(catalog?'Articolo di catalogo':'Identificativo incidente'))}</dt><dd data-no-i18n>${esc(effect.path.split('/').at(catalog?-2:-1))}</dd></div>`:''}${node.location?`<div><dt>${esc(uiText('File del piano'))}</dt><dd data-no-i18n>${esc(node.location)}</dd></div>`:''}</dl><h4>${esc(uiText('Campi da inviare'))}</h4>${value(effect.body)}<details class="approval-json"><summary>${esc(uiText('Dettagli tecnici · JSON completo'))}</summary><pre data-no-i18n>${esc(JSON.stringify(effect,null,2))}</pre></details></section>`;
}
function approvalQuestions(p){return p.questions.map(question=>serviceNowApproval(question,p)||`<p data-no-i18n>${esc(question)}</p>`).join('')}
async function pending(){if(app.reviewRun&&app.reviewRun!==app.run?.id){await loadRun(app.reviewRun)}const p=app.run?.pending;if(!p)return;const q=p.kind==='question'||p.kind==='gate';modal(q?'Decisione richiesta':'Review in attesa',`<p class="small">Run ${esc(app.run.id.slice(0,8))} · ${esc(p.node)} · ${esc(app.user.email)}</p>${approvalQuestions(p)}${p.proposal?`<section class="supervisor-proposal"><h3>${icon('bot')}Proposta del supervisore AI</h3><b>${esc(p.proposal.action)}</b><p data-no-i18n>${esc(p.proposal.rationale)}</p><p class="small">Sessione <span data-no-i18n>${esc(p.proposal.session)}</span> · turno ${p.proposal.turn}</p></section>`:''}<label>${q?'Risposta obbligatoria':'Motivazione / note'}<textarea id="decision-answer" rows="5" ${q?'required':''}>${esc(p.proposal?.answer||'')}</textarea></label><p id="decision-error" class="error" role="alert"></p><p class="note">${q?'La risposta riprende il lavoro nella sessione nativa già salvata.':'La decisione sarà registrata con i riferimenti agli artefatti correnti.'}</p>`,q?'<button class="primary" data-action="decide" data-value="answer">Invia risposta</button>':'<button class="danger" data-action="decide" data-value="reject">Rifiuta</button><button class="primary" data-action="decide" data-value="approve">Approva</button>');}
function reviewHistory(node){const ds=app.decisions.filter(d=>!node||d.node===node).slice().reverse();modal('Registro decisioni',ds.map(d=>`<article class="review-record"><b>${icon(d.actor==='ai'||d.actor==='ai_recommendation'?'bot':'clipboard-check')}${d.actor==='ai'?'Decisione AI':d.actor==='ai_recommendation'?'Proposta AI':'Decisione umana'} · <span data-no-i18n>${esc(d.decision)}</span></b><p data-no-i18n>${esc(d.node)} · ${esc(d.user)}</p>${d.supervisor?`<p data-no-i18n>${esc(d.supervisor.rationale)}</p><p class="small code">${esc(d.supervisor.session)} · turno ${d.supervisor.turn}</p>`:''}<p>${new Date(d.created*1000).toLocaleString(nodoLocale())} · run ${esc(d.run.slice(0,8))}</p><p>${esc(d.answer||'Nessuna nota')}</p><details class="decision-artifacts"><summary><span>Artefatti considerati</span> · ${d.artifacts.length}</summary>${d.artifacts.map(id=>{const a=app.artifacts.find(a=>a.id===id);return a?`<button class="artifact-item" data-action="artifact" data-id="${a.id}">${icon('file-text')} ${esc(a.path)} · v${a.version}</button>`:''}).join('')}</details></article>`).join('')||'<p>Nessuna decisione registrata.</p>');}
function sessions(){const ss=Object.entries(app.run?.sessions||{});modal('Sessioni native persistenti',ss.map(([binding,s])=>`<article class="session-record"><b>${esc(binding)}</b><p class="small">${esc(s.runtime)} · ${s.turns} turni completati · ${esc(s.model||'Modello predefinito')}</p><code class="code">${esc(s.native_id||'In attesa del primo identificativo')}</code></article>`).join('')||'<p>Le sessioni saranno create al primo turno reale.</p>');}
function newNode(kind,position=null){app.selectedNodes=[];app.selectedEdge=null;if(kind==='RAG')ragRefresh().then(()=>{if(app.selected)renderInspector()}).catch(e=>toast(e.message));if(app.run)return toast('Torna alla progettazione per aggiungere nodi');const n={id:kind.toLowerCase()+'-'+crypto.randomUUID().slice(0,8),kind,title:'Nuovo '+kind,phase:'Workflow',x:Math.round((120-app.pan.x)/app.zoom)+(app.wf.nodes.length%4)*175,y:Math.round((150-app.pan.y)/app.zoom)+Math.floor(app.wf.nodes.length/4)*190,agent:app.wf.nodes.find(n=>n.kind==='Agent')?.id||'',runtime:'',pi_provider:'',pi_base_url:'',prompt:'',command:'',mode:'agent_and_human',source_type:'example',source_mode:'direct',provider:'local',operation:'test',outcomes:kind==='Gate'?['pass','rework']:[],max_rework:2};if(position){n.x=Math.round(position.x-58);n.y=Math.round(position.y-44)}if(kind==='Comment'){Object.assign(n,{title:'Commento',phase:'Note',text:'',width:300,height:220,markdown:false,agent:''});if(position){n.x=Math.round(position.x-150);n.y=Math.round(position.y-20)}}if(kind==='Memory')n.agent='';app.wf.nodes.push(n);if(!app.wf.entry&&!['Agent','Source','MCP','RAG','Comment'].includes(kind))app.wf.entry=n.id;app.selected=n.id;dirty();render();if(kind==='Comment')requestAnimationFrame(()=>$('.comment-node[data-id="'+n.id+'"] textarea')?.focus())}
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
  const entryNode=node&&currentWorkflow().nodes.find(n=>n.id===node.dataset.id);
  const promptOption=entryNode&&!edge?`<button type="button" role="menuitem" data-action="set-prompt" data-id="${esc(entryNode.id)}" ${app.run||selectedNodeIds().length!==1||!nodePromptLabel(entryNode.kind)?'disabled':''}>${icon('pencil')}<span>${esc(uiText('Imposta prompt'))}</span></button>`:'';
  nodeMenu.innerHTML=(app.run?`<p class="node-context-note">${esc(uiText('Esecuzione in sola lettura. Torna alla progettazione per cancellare nodi e collegamenti.'))}</p><button type="button" role="menuitem" data-action="edit-run-workflow">${icon('pencil-ruler')}<span>${esc(uiText('Torna alla progettazione'))}</span></button>`:'')+(node&&selectedNodeIds().length<2?`<button type="button" role="menuitem" data-action="continue-run-node" data-node="${esc(node.dataset.id)}" ${app.run&&app.continueNodes.includes(node.dataset.id)?'':'disabled'}>${icon('play')}<span>Continue run</span></button>`:'')+promptOption+(!app.run&&!edge&&entryNode&&selectedNodeIds().length===1&&!['Source','Agent','MCP','RAG','Comment'].includes(entryNode.kind)?`<button type="button" role="menuitem" data-action="set-entry" data-id="${esc(entryNode.id)}" ${currentWorkflow().entry===entryNode.id?'disabled':''}>${icon('workflow-start')}<span>${esc(uiText('Imposta come inizio'))}</span></button>`:'')+`<button type="button" role="menuitem" ${app.run?'disabled':''} data-action="${edge?'delete-link':selectedNodeIds().length>1?'delete-selected-nodes':'delete-node'}" ${edge?`data-edge-key="${esc(edge.dataset.edgeKey)}"`:`data-id="${esc(app.selected)}"`}>${icon('trash-2')}<span>${selectedNodeIds().length>1?esc(uiText('Cancella selezione'))+' ('+selectedNodeIds().length+')':esc(uiText('Cancella'))}</span><kbd>⌫</kbd></button>`;
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
// Also cancel native mouse focus: some browsers still focus after pointerdown.
$('#palette-items').addEventListener('mousedown',e=>{if(e.button===0&&e.target.closest('[data-action="add-node"]'))e.preventDefault()});
$('#palette-items').addEventListener('click',e=>{if(e.detail!==0)e.target.closest('[data-action="add-node"]')?.blur();if(paletteClickSuppressed&&e.detail!==0){paletteClickSuppressed=false;e.preventDefault();e.stopPropagation()}});
$('#palette-items').addEventListener('pointercancel',cancelPaletteDrag);
$('#palette-items').addEventListener('lostpointercapture',e=>{if(paletteDrag?.pointer===e.pointerId)cancelPaletteDrag()});
window.addEventListener('blur',cancelPaletteDrag);
const authForm=$('#auth-form');
authForm.addEventListener('submit',async event=>{
 event.preventDefault();$('#auth-error').textContent='';const body=authCredentials(event.target);if(!body)return;
 $('#auth-submit').disabled=true;
 try{await api(registerMode?'/auth/register':'/auth/login',body);registerMode=false;authInfo.setup_required=false;updateAuthMode();await boot()}
 catch(error){$('#auth-error').textContent=error.message}finally{$('#auth-submit').disabled=false}
});
$('#auth-switch').onclick=()=>{registerMode=!registerMode;updateAuthMode()};
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
async function executionWorkflow(){
 if(!app.run){await saveWorkflow();return structuredClone(app.wf)}
 const id=app.run.workflow.id,workflows=await api('/workflows');
 const workflow=workflows.find(w=>w.id===id);
 if(!workflow)throw Error('Il workflow di questa run non è più disponibile. Duplica la run per creare un nuovo workflow.');
 if(!workflow.nodes.length)throw Error('Il workflow salvato è vuoto. Aggiungi i nodi in progettazione oppure duplica la run per recuperarli.');
 return structuredClone(workflow);
}
async function runNowDialog(){
 try{
  if(app.editBase){await previewRevision();return}
  if(app.run&&app.run.status==='running')return;
  const owner=app.user?.id,context=currentWorkflow()?.id,runId=app.run?.id;
  const workflow=await executionWorkflow();app.runs=await api('/runs');
  if(owner!==app.user?.id||context!==currentWorkflow()?.id||runId!==app.run?.id)return;
  modal('Avvia workflow',`<p>Workflow: <strong data-no-i18n>${esc(workflow.name)}</strong></p>${app.run?'<p class="note">La nuova esecuzione usa la versione salvata di questo workflow. La run precedente resta invariata.</p>':''}<p class="note">Il workflow userà gli agenti e le connessioni configurati. Le cartelle in /sources seguono la modalità scelta nel nodo: modifica permanente o copia. I repository remoti vengono sempre clonati nel workspace. Scegli come gestire review e domande per questa esecuzione.</p><form id="run-form">${policyFields(app.run?.execution||workflow.execution)}<label>Richiesta e contesto aggiuntivo<textarea name="input" rows="3"></textarea></label>${select('Base del repository','base_run','',[['','Source configurato nel workflow'],...app.runs.filter(r=>r.status==='completed').map(r=>[r.id,r.name+' · '+r.id.slice(0,8)])])}</form>`,'<button class="primary" data-action="start-run">Avvia</button>');
  $('#run-form').workflowToRun=workflow;$('#run-form').executionOwner=owner;
 }catch(e){toast(e.message)}
}
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
 const value=k==='skip'?field.checked:k==='mcp_tools'?v.split(/[\s,]+/).filter(Boolean):k==='outcomes'?v.split(',').map(s=>s.trim()).filter(Boolean):['max_rework','rag_top_k'].includes(k)?Number(v):['require_tests','require_tasks_closed'].includes(k)?v==='true':v;
 if(JSON.stringify(n[k])!==JSON.stringify(value)){n[k]=value;dirty()}
}
function captureInspectorFields(){const form=$('#node-form');if(form&&!app.run)for(const field of form.elements)updateInspectorField(field)}
// Keep edits in the workflow draft without replacing the focused inspector fields.
document.addEventListener('input',e=>updateInspectorField(e.target));
document.addEventListener('focusout',e=>{if(!app.run&&e.target.form?.id==='node-form'&&e.target.form.isConnected){updateInspectorField(e.target);renderGraph();renderList()}});
document.addEventListener('change',e=>{updateInspectorField(e.target);if(e.target.matches('#node-form [name=skip]')){renderGraph();renderList()}});
document.addEventListener('submit',e=>{if(e.target.id==='node-form'){e.preventDefault();if(app.run)return;captureInspectorFields();saveWorkflow().catch(error=>toast(error.message))}});
document.addEventListener('change',e=>{
 if(e.target.matches('#node-form [name=operation]')){const n=currentWorkflow().nodes.find(n=>n.id===app.selected);if(n?.kind==='Connection'&&n.provider==='github'){captureInspectorFields();if(n.operation==='api_request'){n.api_method||='GET';n.api_path||='/repos/{owner}/{repo}';n.api_encoding||='json';dirty()}renderInspector();return}}
 if(e.target.matches('#node-form [name=api_endpoint]')){const n=currentWorkflow().nodes.find(n=>n.id===app.selected),endpoint=(window.githubApiCatalog||[]).find(x=>x.id===e.target.value);captureInspectorFields();if(n&&endpoint){applyGithubEndpoint(n,endpoint)}return}
 if(e.target.matches('#node-form [name=runtime],#node-form [name=pi_provider]')){const node=currentWorkflow().nodes.find(node=>node.id===app.selected);if(node?.kind==='Agent'){captureInspectorFields();renderGraph();renderList();renderInspector();return}}if(e.target.matches('#node-form [name=provider]')){const f=e.target.form,n=currentWorkflow().nodes.find(n=>n.id===app.selected);captureInspectorFields();f.twgCommand=f.elements.command?.value??f.twgCommand??n.command;$('#connection-fields').innerHTML=connectionFields({...n,provider:e.target.value,command:f.twgCommand});captureInspectorFields();renderGraph();renderInspector()}if(e.target.matches('#node-form [name=source_type]')){$('#source-mode-fields').hidden=e.target.value!=='folder';loadSourceFolderOptions(e.target.form)}if(e.target.matches('#node-form [name=source_mode]')&&e.target.value==='copy'&&e.target.form.elements.location.value){const form=e.target.form;confirmSourceCopy({location:form.elements.location.value}).then(ok=>{if(!ok){e.target.value='direct';updateInspectorField(e.target)}}).catch(error=>{e.target.value='direct';updateInspectorField(e.target);toast(error.message)})}if(e.target.id==='artifact-version')artifact(e.target.value).catch(e=>toast(e.message))});
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
case 'github-api-catalog':showGithubCatalog();break;
case 'twg-setup':twgSetup();break;
case 'twg-check':await twgCheck(b);break;case 'atlassian-check':await testAtlassian(b);break;case 'twg-login':await loginAtlassian(b);break;
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
case 'set-prompt':editNodePrompt(b.dataset.id);break;
case 'set-entry':{if(app.run)break;const n=app.wf.nodes.find(n=>n.id===b.dataset.id);if(!n||['Source','Agent','MCP','RAG','Comment'].includes(n.kind))break;captureInspectorFields();app.wf.entry=n.id;closeNodeMenu();dirty();render();toast('Ingresso impostato');break;}
case 'create-workflow':{leaveDraftContext();app.editBase=null;const name=$('[name="workflow_name"]').value.trim();if(!name)throw Error('Nome obbligatorio');disconnect();app.run=null;app.wf={id:crypto.randomUUID(),name,description:'',entry:'',nodes:[],edges:[]};app.selected=null;await saveWorkflow();render();closeModal();break;}
case 'clear-key':{const field=$(`[name="${b.dataset.field}"]`);field.value='__CLEAR__';(field.closest('[data-settings-root]')||field.form)?.settingsEditedFields?.add(field.name);field.removeAttribute('placeholder')}
toast('La credenziale sarà rimossa al salvataggio');break;case 'password-dialog':await settingsDialog('general');break;case 'change-password':await changeAccountPassword();break;case 'compact-session':await compactSession();break;case 'settings':await settingsDialog();break;case 'configure-agents':await settingsDialog('agents');break;case 'pi-check':await checkPiConnection(b);break;case 'servicenow-check':{const form=b.closest('form'),url=$('[name=base_servicenow]',form);if(!url.reportValidity())break;const status=$('[data-servicenow-status]');if(status)status.textContent=uiText('Verifica connessione in corso…');const draft=serviceNowDraft(form);b.disabled=true;try{const result=await api('/connections/servicenow/status',draft,'POST');if(result.connected&&JSON.stringify(serviceNowDraft(form))===JSON.stringify(draft))for(const name of ['base_servicenow','sn_auth','sn_username','sn_password','token_servicenow','sn_client_id','sn_client_secret','sn_oauth_scope'])(form.closest('[data-settings-root]')||form).settingsEditedFields?.add(name);if(status?.isConnected)status.textContent=result.message}catch(e){if(status?.isConnected)status.textContent=e.message}finally{if(b.isConnected)b.disabled=false}break;}case 'save-settings':await saveSettings();break;
case 'environment':await showEnvironment();break;
case 'capabilities':await showCliCapabilities();break;
case 'admin-user-suspend':case 'admin-user-activate':case 'admin-user-remove':await changeAdminUser(b);break;case 'admin-users-refresh':await loadAdminUsers();break;case 'add-user':await createAccountUser();break;
case 'logout':closeAccountMenu();app.editBase=null;await api('/auth/logout',{});app.run=null;showAuth();break;
case 'start-run':{const form=$('#run-form');if(!form.reportValidity())break;const workflow=form.workflowToRun;if(!workflow||form.executionOwner!==app.user?.id)throw Error('Accedi per continuare');b.disabled=true;const f=Object.fromEntries(new FormData(form));let index=0,allowLargeCopy=false;for(const n of workflow.nodes.filter(n=>n.kind==='Source')){if(n.source_type==='folder'&&(n.source_mode||'copy')!=='direct'&&!(index===0&&f.base_run)){if(!await confirmSourceCopy(n,()=>{allowLargeCopy=true})){b.disabled=false;return}}index++}const r=await api('/runs',{workflow_id:workflow.id,locale:nodoLocale().startsWith('it')?'it':'en',input:f.input,base_run:f.base_run||null,allow_large_source_copy:allowLargeCopy,execution:readPolicy(form)});closeModal();await loadRun(r.id);logs(true);break;}
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
 const resultDownload=root==='workspaces'&&parts.length?`<a class="storage-picker" href="/api/storage/download?root=workspaces&path=${encodeURIComponent(parts[0])}" download>${icon('download')}Scarica risultato ZIP</a>`:'';
 const ragAction=(root!=='workspaces'||path)&&!path.split('/').some(p=>['.rightcommit','.nodo'].includes(p))?`<div class="rag-folder-action">${ragButton(root,path,uiText('Indice RAG'))}</div>`:'';
 const sourceNote=root==='sources'?'<p class="note storage-source-note">Le Source in modalità permanente vengono modificate direttamente. Le altre usano copie nel workspace.</p>':'';
 const toolbar=storageRoots(root,true,`<div class="storage-header-actions">${ragAction}${resultDownload}</div>`)+`<nav class="storage-breadcrumb" aria-label="Percorso">${crumbs.map(([p,n])=>`<button data-storage-path="${esc(p)}">${esc(n)}</button>`).join('<span>/</span>')}</nav>`+(d.file?sourceNote:'');
 const controls=root==='workspaces'?'<p class="note">File delle tue esecuzioni. In modalità permanente mostrano il contenuto attuale condiviso della sorgente. Consultazione e download; build e cartelle Git esclusi dallo ZIP.</p>':root==='examples'?'<p class="note">Examples precaricati: navigazione e download disponibili, archivio di sola lettura.</p>':`<div class="storage-actions"><label class="storage-picker">${icon('file-text')}Carica file<input id="storage-files" type="file" multiple hidden></label><label class="storage-picker">${icon('folder-git-2')}Carica cartella<input id="storage-folder" type="file" webkitdirectory multiple hidden></label><span class="small">Massimo 50 MB per file · nomi esistenti preservati</span></div>`;
 modal('Storage',toolbar+(d.file?`<div class="storage-file-head"><span>${icon('file-text')}${esc(parts.at(-1))} <small class="storage-entry-size" data-no-i18n>${storageSize(d.size)}</small></span><a class="storage-picker" href="/api/storage/download?path=${encodeURIComponent(path)}&root=${root}" download>Scarica ZIP</a></div>${d.preview===null?`<p class="note">${esc(filePreviewMessage(d))}</p>`:d.html?`<div class="markdown storage-markdown">${d.html}</div>`:`<pre class="source-view storage-preview">${esc(d.preview)}</pre>`}`:controls+(root!=='sources'?'':`<div id="storage-drop" class="storage-drop" tabindex="0"><span>${icon('folder-git-2')}Trascina qui file o cartelle per caricarli in /sources/${esc(path)}</span><div id="storage-progress" role="status" aria-live="polite"></div></div>`)+`<div class="storage-list-controls"><div class="storage-list-actions">${storageHiddenControl()}<button type="button" class="storage-refresh" data-storage-refresh>${icon('refresh-cw')}Aggiorna</button></div>${sourceNote}</div><div class="storage-table">${d.entries.map(e=>{const p=storageJoin(path,e.name);return `<div class="storage-row" ${storageHiddenAttrs(e)}><button class="storage-entry" data-storage-path="${esc(p)}">${icon(e.directory?'folder-git-2':'file-text')}<span class="storage-entry-label"><span>${esc(e.label||e.name)}</span><small class="storage-entry-size" data-no-i18n ${e.directory?`data-storage-size="${esc(e.name)}"`:""}>${e.directory?"…":storageSize(e.size)}</small></span></button><span class="small">${e.status?esc(states[e.status]||e.status)+(e.source_mode==='direct'?' · '+uiText('Sorgente originale — contenuto attuale condiviso'):''):e.directory?'Cartella':''}</span>${!['.rightcommit','.nodo'].includes(e.name)?ragButton(root,p):''}<a class="icon-button" data-tip="Scarica ZIP" aria-label="Scarica ZIP di ${esc(e.name)}" href="/api/storage/download?path=${encodeURIComponent(p)}&root=${root}" download>${icon('download')}</a></div>`}).join('')}<p class="note storage-empty" data-storage-empty>Nessun elemento visibile.</p></div>`),'',true);
 bindStorageHidden();bindRagStorage();
 const table=$('.storage-table',$('#dialog'));
 if(table&&table.querySelector('[data-storage-size]'))api('/storage/entry-sizes?root='+encodeURIComponent(root)+'&path='+encodeURIComponent(path)).then(data=>{
  if(!table.isConnected)return;
  table.querySelectorAll('[data-storage-size]').forEach(label=>{const size=data.sizes[label.dataset.storageSize];label.textContent=Number.isFinite(size)?storageSize(size):'—'});
 }).catch(()=>{if(table.isConnected)table.querySelectorAll('[data-storage-size]').forEach(label=>label.textContent='—')});
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
function storageSize(n){if(!Number.isFinite(n))return '—';const units=['B','KB','MB','GB','TB'];let i=0;while(n>=1024&&i<units.length-1){n/=1024;i++}return (i?n.toFixed(1):String(n))+' '+units[i]}
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
  const message=uiText('Vuoi eliminare questo workflow?')+'\n'+workflow.name+'\n\n'+uiText('Esecuzioni collegate:')+' '+summary.runs+'\n'+uiText('Artefatti e versioni:')+' '+summary.artifacts+'\n'+uiText('Conversazioni manager:')+' '+(summary.manager_chats||0)+'\n'+uiText('Pianificazioni collegate:')+' '+(summary.schedules||0)+'\n\n'+uiText('Verranno eliminati il workflow, tutte le sue esecuzioni, i workspace, gli artefatti, i log e le sessioni locali associate. Questa operazione non può essere annullata.')+'\n\n'+uiText('I file e le cartelle in /sources vengono sempre conservati.');
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
 const panels=[['inspector-stack',330,320],['manager-panel',380,350]].map(([id,desktop,compact])=>{
  const panel=document.getElementById(id),grip=document.createElement('div');
  grip.className='right-panel-resizer';grip.tabIndex=0;
  grip.setAttribute('role','separator');grip.setAttribute('aria-orientation','vertical');
  grip.setAttribute('aria-controls',id);
  grip.setAttribute('aria-label',id==='inspector-stack'?'Ridimensiona inspector':'Ridimensiona chat');
  panel.before(grip);
  const storageKey=id==='inspector-stack'?'nodo.inspectorStackWidth':null;
  let preferred=null;
  if(storageKey)try{const saved=Number(localStorage.getItem(storageKey));if(Number.isFinite(saved)&&saved>0)preferred=saved}catch{}
  const bounds=()=>{const base=innerWidth<=1000?compact:desktop,min=Math.min(base,innerWidth<=550?innerWidth-28:base);return {min,max:Math.max(min,Math.floor(innerWidth/2))}};
  const apply=width=>{
   const {min,max}=bounds(),value=Math.max(min,Math.min(max,width??min));
   panel.style.setProperty('--right-panel-width',value+'px');
   grip.setAttribute('aria-valuemin',min);grip.setAttribute('aria-valuemax',max);grip.setAttribute('aria-valuenow',value);
   grip.hidden=panel.hidden;return value;
  };
  const resize=width=>{preferred=apply(width);if(storageKey)try{localStorage.setItem(storageKey,String(preferred))}catch{}};
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
  grip.addEventListener('dblclick',()=>{preferred=null;if(storageKey)try{localStorage.removeItem(storageKey)}catch{}apply(null)});
  new MutationObserver(()=>apply(preferred)).observe(panel,{attributes:true,attributeFilter:['hidden']});
  apply(preferred);return ()=>apply(preferred);
 });
 window.addEventListener('resize',()=>panels.forEach(update=>update()));
})();

// Persist the artifacts height independently from the selected node.
(()=>{
 const stack=$('#inspector-stack'),panel=$('#artifacts-panel'),grip=document.createElement('div');
 const storageKey='nodo.artifactsHeightRatio';let preferred=null;
 try{const saved=localStorage.getItem(storageKey),ratio=Number(saved);if(saved==='collapsed')preferred='collapsed';else if(Number.isFinite(ratio)&&ratio>0&&ratio<1)preferred=ratio}catch{}
 grip.className='artifacts-height-resizer';grip.tabIndex=0;grip.setAttribute('role','separator');
 grip.setAttribute('aria-orientation','horizontal');grip.setAttribute('aria-controls','artifacts-panel');grip.setAttribute('aria-label','Ridimensiona altezza artefatti');panel.before(grip);
 const bounds=()=>{
  const total=stack.clientHeight;if(!total)return null;
  panel.style.flex='';
  const defaultHeight=panel.getBoundingClientRect().height,min=parseFloat(getComputedStyle(panel).minHeight)||60,max=Math.max(min,total-grip.offsetHeight-56);
  return {total,min,max,defaultHeight};
 };
 const apply=(height,save=false)=>{
  const limits=bounds();if(!limits)return;
  const value=Math.max(limits.min,Math.min(limits.max,height??(preferred===null?limits.defaultHeight:preferred==='collapsed'?limits.min:preferred*limits.total)));
  panel.style.flex=`0 0 ${value}px`;
  panel.classList.toggle('artifacts-title-only',value<=limits.min+1);
  grip.setAttribute('aria-valuemin',Math.round(limits.min));grip.setAttribute('aria-valuemax',Math.round(limits.max));grip.setAttribute('aria-valuenow',Math.round(value));
  if(save){preferred=value<=limits.min+1?'collapsed':value/limits.total;try{localStorage.setItem(storageKey,String(preferred))}catch{}}
 };
 grip.addEventListener('pointerdown',event=>{
  if(event.button!==0)return;event.preventDefault();grip.focus();
  const start=event.clientY,height=panel.getBoundingClientRect().height;
  grip.setPointerCapture(event.pointerId);document.body.classList.add('resizing-artifacts-height');
  const move=e=>apply(height+start-e.clientY,true);
  const end=()=>{grip.removeEventListener('pointermove',move);grip.removeEventListener('lostpointercapture',end);document.body.classList.remove('resizing-artifacts-height')};
  grip.addEventListener('pointermove',move);grip.addEventListener('lostpointercapture',end);
 });
 grip.addEventListener('keydown',event=>{
  const limits=bounds();if(!limits)return;
  apply();const height=panel.getBoundingClientRect().height,step=event.shiftKey?50:10;
  const next={ArrowUp:height+step,ArrowDown:height-step,Home:limits.min,End:limits.max}[event.key];
  if(next===undefined)return;event.preventDefault();apply(next,true);
 });
 grip.addEventListener('dblclick',()=>{preferred=null;try{localStorage.removeItem(storageKey)}catch{}apply()});
 new ResizeObserver(()=>apply()).observe(stack);
})();

// Activity Log height is a browser preference, independent from workflow/run data.
(()=>{
 const panel=$('#logs'),grip=document.createElement('div'),storageKey='nodo.activityLogHeight';
 const defaultHeight=320;let preferred=null;
 try{const saved=Number(localStorage.getItem(storageKey));if(Number.isFinite(saved)&&saved>0)preferred=saved}catch{}
 grip.id='activity-log-resizer';grip.tabIndex=0;grip.setAttribute('role','separator');
 grip.setAttribute('aria-orientation','horizontal');grip.setAttribute('aria-controls','logs');
 grip.setAttribute('aria-label','Ridimensiona Activity Log');panel.prepend(grip);
 const bounds=()=>({min:120,max:Math.max(120,Math.floor(innerHeight-$('#workbench').getBoundingClientRect().top-$('footer').offsetHeight-160))});
 const apply=()=>{
  const {min,max}=bounds(),height=Math.round(Math.max(min,Math.min(max,preferred??defaultHeight)));
  document.documentElement.style.setProperty('--activity-log-height',height+'px');
  grip.setAttribute('aria-valuemin',min);grip.setAttribute('aria-valuemax',max);grip.setAttribute('aria-valuenow',height);
  return height;
 };
 const persist=()=>{try{localStorage.setItem(storageKey,String(preferred))}catch{}};
 grip.addEventListener('pointerdown',event=>{
  if(event.button!==0)return;event.preventDefault();grip.focus();
  const start=event.clientY,height=panel.getBoundingClientRect().height;
  grip.setPointerCapture(event.pointerId);document.body.classList.add('resizing-activity-log');
  const move=e=>{preferred=height+start-e.clientY;preferred=apply()};
  const end=()=>{
   grip.removeEventListener('pointermove',move);grip.removeEventListener('lostpointercapture',end);
   document.body.classList.remove('resizing-activity-log');if(preferred!==null)persist();
  };
  grip.addEventListener('pointermove',move);grip.addEventListener('lostpointercapture',end);
 });
 grip.addEventListener('keydown',event=>{
  const {min,max}=bounds(),height=panel.getBoundingClientRect().height,step=event.shiftKey?50:10;
  const next={ArrowUp:height+step,ArrowDown:height-step,Home:min,End:max}[event.key];
  if(next===undefined)return;event.preventDefault();preferred=next;preferred=apply();persist();
 });
 grip.addEventListener('dblclick',()=>{preferred=null;try{localStorage.removeItem(storageKey)}catch{}apply()});
 new MutationObserver(apply).observe(document.body,{attributes:true,attributeFilter:['class']});
 new MutationObserver(apply).observe(panel,{attributes:true,attributeFilter:['hidden']});
 window.addEventListener('resize',apply);apply();
})();
