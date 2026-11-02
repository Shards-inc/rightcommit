/* The shared catalog has explicit locales; never run its content through UI translation. */
function catalogText(item){const lang=nodoLocale().startsWith('it')?'it':'en',selected=item.locales?.[lang]||{},fallback=item.locales?.[item.source_locale||'en']||item.locales?.it||{};return Object.fromEntries([...new Set([...Object.keys(fallback),...Object.keys(selected)])].map(k=>[k,selected[k]||fallback[k]||'']))}
let templateManager=null;
const tmStatus=value=>uiText(({draft:'Bozza',published:'Pubblicato',archived:'Archiviato'})[value]||value);
const tmTranslation=value=>uiText(({current:'Aggiornate',reviewed:'Verificate',stale:'Da rivedere',missing:'Da completare'})[value]||value);
const tmFields=['title','description','setup','objective','prerequisites','outputs','limitations'];
const tmLabels={title:'Titolo',description:'Descrizione breve',setup:'Configurazione',objective:'Obiettivo',prerequisites:'Prerequisiti',outputs:'Risultati attesi',limitations:'Limiti'};
// Only application-owned messages are translated. Workflow text and model output remain verbatim.
function tmFormat(key,values={}){return uiText(key).replace(/\{(\w+)\}/g,(match,key)=>String(values[key]??match))}
function tmFieldLabel(path){const [lang,field]=path.includes('.')?path.split('.'):['',path];return (lang?lang.toUpperCase()+' · ':'')+uiText(tmLabels[field]||({services:'Servizi richiesti',tags:'Tag'})[field]||field)}
function tmMessage(error){
 const message=error.tmOriginal||error.message||String(error);
 if(window.uiHasTranslation(message))return uiText(message);
 if(message.includes('; '))return message.split('; ').map(message=>tmMessage({message,tmFallback:error.tmFallback})).join('; ');
 let match;
 if((match=message.match(/^Localizzazione (it|en) mancante$/)))return tmFormat('Localizzazione {locale} mancante',{locale:match[1].toUpperCase()});
 for(const suffix of ['percorso personale da sostituire','dipendenza privata da configurare','possibile credenziale incorporata'])if(message.endsWith(': '+suffix))return tmFormat('{node}: '+suffix,{node:message.slice(0,-suffix.length-2)});
 for(const prefix of ['Tipo nodo non disponibile','Collegamento non valido','Porte incompatibili','Nodo non raggiungibile','Nodo senza collegamenti','Nodo inesistente','Più rami per lo stesso esito'])if(message.startsWith(prefix+': '))return tmFormat(prefix+': {detail}',{detail:message.slice(prefix.length+2)});
 if((match=message.match(/^Il nodo (.+) richiede un Agent$/)))return tmFormat('Il nodo {node} richiede un Agent',{node:match[1]});
 if((match=message.match(/^Gate (.+) senza ramo (.+)$/)))return tmFormat('Gate {node} senza ramo {outcome}',{node:match[1],outcome:match[2]});
 if((match=message.match(/^(.+) non è collegato al percorso eseguibile$/)))return tmFormat('{node} non è collegato al percorso eseguibile',{node:match[1]});
 if((match=message.match(/^(.+): cambio Agent senza Memory corretto$/)))return tmFormat('{nodes}: cambio Agent senza Memory corretto',{nodes:match[1]});
 if((match=message.match(/^CLI (claude|codex|pi) non disponibile$/)))return tmFormat('L’agente {runtime} non è disponibile. Verifica la configurazione.',{runtime:match[1]});
 if(/Failed to fetch|fetch failed|NetworkError|Load failed/i.test(message))return uiText('Impossibile contattare il server. Controlla la connessione e riprova.');
 return uiText(error.tmFallback||'Operazione sui template non riuscita. Riprova.');
}
async function tmApi(path,body,method){
 try{return await api(path,body,method)}catch(cause){
  const error=new Error('',{cause});error.tmOriginal=cause.message;
  error.tmFallback=path.endsWith('/assist')?'Impossibile completare la richiesta AI. Verifica il modello preferito e le credenziali nelle configurazioni personali.':path.includes('/history')?'Impossibile caricare la cronologia del catalogo. Riprova.':path.endsWith('/validate')?'Il catalogo JSON non è valido o contiene campi mancanti.':path.startsWith('/workflows')?'Impossibile caricare o salvare il workflow. Riprova.':'Impossibile leggere o salvare il catalogo. Riprova.';
  if(path.endsWith('/validate')&&error.tmOriginal==='Risposta server non valida')error.tmOriginal=error.tmFallback;
  error.message=tmMessage(error);throw error;
 }
}
function tmOverlayError(dialog,error){dialog.tmError=error;dialog.querySelector('.error').textContent=tmMessage(error)}
function tmLocalizeOverlays(){
 $$('.tm-overlay').forEach(dialog=>{
  if(dialog.tmError)tmOverlayError(dialog,dialog.tmError);const generate=dialog.querySelector('#tm-section-ai');if(generate)generate.textContent=uiText(generate.disabled?'Elaborazione…':'Genera proposta');
  const request=dialog.querySelector('#tm-section-request');if(request&&request.value===request.dataset.tmDefaultRequest){request.value=uiText('Traduci o migliora titolo e descrizione in italiano e inglese.');request.dataset.tmDefaultRequest=request.value;}
  $$('[data-tm-proposal-label]',dialog).forEach(el=>el.textContent=tmFieldLabel(el.dataset.tmProposalLabel));
  $$('jui-switch[data-tm-proposal-path]',dialog).forEach(el=>el.setAttribute('aria-label',uiText('Applica')+' '+tmFieldLabel(el.dataset.tmProposalPath)));
 });
}
function tmText(){return Object.fromEntries(tmFields.map(k=>[k,'']))}
function tmCurrent(){return templateManager?.catalog.templates.find(t=>t.id===templateManager.selected)}
function tmDirty(){return templateManager&&JSON.stringify(templateManager.catalog)!==templateManager.saved}
function tmGuard(event){if(tmDirty())event.preventDefault()}
async function manageTemplates(){
 if(!app.user?.admin)return;
 const catalog=await tmApi('/admin/templates');
 templateManager={catalog,saved:JSON.stringify(catalog),section:catalog.sections[0]?.id,selected:null,tab:'contents',proposal:null,busy:false,error:null};
 tmRender();
}
function tmRender(){
 const m=templateManager;if(!m)return;const previous=$('#template-manager')?.dataset.templateId;if($('#tm-ai-request')){m.aiDrafts??={};m.aiDrafts[previous]={text:$('#tm-ai-request').value,action:$('#tm-ai-action').value,lang:$('#tm-ai-lang').value};}const c=m.catalog,t=tmCurrent(),section=c.sections.find(s=>s.id===m.section);
 modal('Gestisci template',`<div id="template-manager" data-template-id="${esc(m.selected||'')}"><div class="tm-toolbar"><button data-tm="new-section">${icon('plus')}Nuova sezione</button><button data-tm="from-workflow">Crea da workflow</button><button data-tm="import">Importa catalogo</button><button data-tm="export">Esporta catalogo</button><button data-tm="history">Cronologia</button><span class="small" id="tm-dirty" role="status"></span></div><div class="tm-layout"><aside class="tm-sections"><label>Cerca<input id="tm-search" type="search"></label>${[...c.sections].sort((a,b)=>a.order-b.order).map(s=>`<button draggable="true" data-tm-section="${esc(s.id)}" class="${s.id===m.section?'active':''}"><span data-no-i18n>${esc(catalogText(s).title||s.id)}</span><small>${esc(tmStatus(s.status))}</small></button>`).join('')}</aside><section class="tm-list">${section?`<div class="tm-toolbar"><b data-no-i18n>${esc(catalogText(section).title||section.id)}</b><button data-tm="edit-section" aria-label="Modifica sezione">${icon('pencil')}</button><button data-tm="section-up" aria-label="Sposta sezione su">↑</button><button data-tm="section-down" aria-label="Sposta sezione giù">↓</button></div><p data-no-i18n>${esc(catalogText(section).description)}</p>`:''}${[...c.templates].filter(t=>t.section===m.section).sort((a,b)=>a.order-b.order).map(t=>`<button class="tm-template ${t.id===m.selected?'active':''}" draggable="true" data-tm-template="${esc(t.id)}"><strong data-no-i18n>${esc(catalogText(t).title||t.id)}</strong><small>${esc(tmStatus(t.status))} · IT ${t.locales.it?.title?'✓':'—'} · EN ${t.locales.en?.title?'✓':'—'}</small></button>`).join('')}</section><section class="tm-detail">${t?tmDetail(t):'<p>Seleziona un template o creane uno da un workflow.</p>'}</section></div><p id="tm-error" data-no-i18n class="error" role="alert"></p></div>`, '<button data-tm="close">Chiudi</button><div class="spacer"></div><button data-tm="save" class="primary">Salva bozza</button>',true);
 const dialog=$('#dialog');dialog.classList.add('template-manager-dialog');
 dialog.oncancel=e=>{e.preventDefault();tmClose()};dialog.onkeydown=e=>{if(e.key==='Escape'){e.preventDefault();tmClose()}};
 $$('.modal-head [data-action="close-dialog"]',dialog).forEach(b=>{b.removeAttribute('data-action');b.onclick=tmClose});
 $$('[data-tm]').forEach(b=>b.onclick=()=>tmAction(b.dataset.tm).catch(tmError));
 $$('[data-tm-section]').forEach(b=>b.onclick=()=>{m.section=b.dataset.tmSection;m.selected=null;tmRender()});
 $$('[data-tm-template]').forEach(b=>b.onclick=()=>{m.selected=b.dataset.tmTemplate;m.proposal=null;tmRender()});
 $$('[data-tm-tab]').forEach(b=>b.onclick=()=>{m.tab=b.dataset.tmTab;tmRender()});
 $$('[data-tm-field]').forEach(el=>el.oninput=()=>{t.locales[el.dataset.lang][el.dataset.tmField]=el.value;t.status='draft';if(el.dataset.lang===t.source_locale)t.translation_status='stale';tmMark();const p=t.locales[nodoLocale().startsWith('it')?'it':'en'];if($('#tm-preview'))$('#tm-preview').innerHTML=`<strong>${esc(p.title)}</strong><p>${esc(p.description)}</p><small>${esc(p.setup)}</small>`});
 const form=$('#tm-metadata');if(form)form.onchange=e=>{const n=e.target.name;if(n==='section'){t.section=e.target.value;m.section=t.section}if(n==='source_locale')t.source_locale=e.target.value;if(n==='tags'||n==='services')t[n]=e.target.value.split(',').map(v=>v.trim()).filter(Boolean);if(n==='reviewed')t.translation_status=e.target.checked?'reviewed':'stale';tmMark()};
 $('#tm-search').oninput=e=>{const term=e.target.value.toLocaleLowerCase();$$('[data-tm-template]').forEach(b=>b.hidden=!b.textContent.toLocaleLowerCase().includes(term))};
 $$('[data-tm-section],[data-tm-template]').forEach(el=>{
  el.ondragstart=e=>{e.dataTransfer.setData('application/x-template-catalog',JSON.stringify({section:el.dataset.tmSection,template:el.dataset.tmTemplate}));e.dataTransfer.effectAllowed='move'};
  el.ondragover=e=>{if([...e.dataTransfer.types].includes('application/x-template-catalog'))e.preventDefault()};
  el.ondrop=e=>{e.preventDefault();const drag=JSON.parse(e.dataTransfer.getData('application/x-template-catalog'));if((drag.template&&drag.template===el.dataset.tmTemplate)||(drag.section&&drag.section===el.dataset.tmSection))return;if(drag.template){const item=c.templates.find(t=>t.id===drag.template);const target=c.templates.find(t=>t.id===el.dataset.tmTemplate);item.section=target?.section||el.dataset.tmSection;item.status='draft';if(target){const items=c.templates.filter(t=>t.section===item.section&&t.id!==item.id).sort((a,b)=>a.order-b.order);items.splice(items.indexOf(target),0,item);tmRank(items,item)}m.section=item.section;m.selected=item.id}else if(drag.section&&el.dataset.tmSection){const item=c.sections.find(s=>s.id===drag.section),target=c.sections.find(s=>s.id===el.dataset.tmSection);const items=c.sections.filter(s=>s.id!==item.id).sort((a,b)=>a.order-b.order);items.splice(items.indexOf(target),0,item);tmRank(items,item)}tmRender()};
 });
 const draft=m.aiDrafts?.[m.selected];if(draft&&$('#tm-ai-request')){$('#tm-ai-request').value=draft.text;$('#tm-ai-action').value=draft.action;$('#tm-ai-lang').value=draft.lang;}
 tmMark();if(m.error)tmError(m.error);
}
function tmError(e){if(templateManager)templateManager.error=e;const el=$('#tm-error');if(el)el.textContent=tmMessage(e);else toast(tmMessage(e))}
function tmMark(){const generate=$('[data-tm=ai]');if(generate)generate.textContent=uiText(templateManager.busy?'Elaborazione…':'Genera proposta');const current=tmCurrent();if(current&&$('#tm-metadata [name=reviewed]'))$('#tm-metadata [name=reviewed]').checked=['current','reviewed'].includes(current.translation_status);if(current&&$('#tm-translation-status'))$('#tm-translation-status').textContent=tmTranslation(current.translation_status);const el=$('#tm-dirty');if(el)el.textContent=uiText(tmDirty()?'Modifiche non salvate':'Salvato');$$('#dialog-content button,#dialog-content input,#dialog-content textarea,#dialog-content jui-select,#dialog-content jui-switch').forEach(b=>b.disabled=templateManager.busy)}
function tmDetail(t){
 const m=templateManager,tab=m.tab,lang=nodoLocale().startsWith('it')?'it':'en';
 let body='';
 if(tab==='contents'||tab==='locales')body=`<div class="tm-locales">${['it','en'].map(lang=>`<div><h3>${lang.toUpperCase()}</h3>${tmFields.map(k=>`<label>${uiText(tmLabels[k])}<textarea data-no-i18n data-lang="${lang}" data-tm-field="${k}" rows="${k==='title'?1:3}" maxlength="12000">${esc(t.locales[lang]?.[k]||'')}</textarea></label>`).join('')}</div>`).join('')}</div>`;
 if(tab==='workflow')body=`<p data-no-i18n>${esc(tmFormat('{nodes} nodi · {edges} collegamenti',{nodes:t.workflow.nodes.length,edges:t.workflow.edges.length}))}</p><button data-tm="replace-workflow">Sostituisci workflow</button><button data-tm="import-workflow">Importa workflow JSON</button><button data-tm="test">Crea workflow di prova</button><details><summary>Workflow JSON</summary><pre data-no-i18n>${esc(JSON.stringify(t.workflow,null,2))}</pre></details><p data-no-i18n>${esc(JSON.stringify(t.provenance))}</p>`;
 if(tab==='assistant'){
 const runtime=app.settings.default_runtime,cfg=app.settings.agents[runtime]||{};
 body=`<p><span>Modello preferred</span>: <span data-no-i18n>${esc(runtime)} · ${esc(cfg.model||uiText('Predefinito'))} · ${esc(cfg.effort||uiText('Predefinito'))}</span></p><label>Azione<jui-select id="tm-ai-action"><jui-option value="analyze">Analizza workflow e proponi i campi</jui-option><jui-option value="title">Genera titolo</jui-option><jui-option value="description">Genera descrizione</jui-option><jui-option value="setup">Genera configurazione e prerequisiti</jui-option><jui-option value="translate">Traduci</jui-option><jui-option value="check">Controlla coerenza</jui-option><jui-option value="custom">Richiesta libera</jui-option></jui-select></label><label>Lingua<jui-select id="tm-ai-lang"><jui-option value="both">IT + EN</jui-option><jui-option value="it">IT</jui-option><jui-option value="en">EN</jui-option></jui-select></label><label>Indicazioni<textarea id="tm-ai-request" rows="3"></textarea></label><button data-tm="ai" ${m.busy?'disabled':''}>${m.busy?'Elaborazione…':'Genera proposta'}</button><div id="tm-proposal">${tmProposal(t)}</div>`;
 }
 const preview=t.locales[lang]||{};
 return `<div class="tm-toolbar"><button data-tm="duplicate">Duplica</button><button data-tm="up" aria-label="Sposta template su">↑</button><button data-tm="down" aria-label="Sposta template giù">↓</button><button data-tm="archive">${t.status==='archived'?'Ripristina bozza':'Archivia'}</button><button data-tm="delete" class="danger">Elimina</button><button data-tm="publish" class="primary">Pubblica</button></div><form id="tm-metadata"><label>Sezione<jui-select name="section">${m.catalog.sections.map(s=>`<jui-option value="${esc(s.id)}" ${t.section===s.id?'selected':''} data-no-i18n>${esc(catalogText(s).title||s.id)}</jui-option>`).join('')}</jui-select></label><label>Tag<input name="tags" value="${esc(t.tags.join(', '))}"></label><label>Servizi richiesti<input name="services" value="${esc(t.services.join(', '))}"></label><label>Lingua originale<jui-select name="source_locale"><jui-option value="en" ${t.source_locale==='en'?'selected':''}>EN</jui-option><jui-option value="it" ${t.source_locale==='it'?'selected':''}>IT</jui-option></jui-select></label><label class="tm-reviewed"><jui-switch name="reviewed" aria-label="Traduzioni verificate" ${['current','reviewed'].includes(t.translation_status)?'checked':''}></jui-switch>Traduzioni verificate</label><p>${uiText('Stato traduzioni')}: <span id="tm-translation-status" data-no-i18n>${esc(tmTranslation(t.translation_status))}</span></p></form><div class="tabs">${[['contents','Contenuti'],['workflow','Workflow'],['locales','Localizzazioni'],['assistant','Assistente']].map(([id,label])=>`<button data-tm-tab="${id}" class="${tab===id?'active':''}">${label}</button>`).join('')}</div>${body}<details open><summary>Anteprima scheda</summary><div class="tm-toolbar"><button data-tm="preview-it">IT</button><button data-tm="preview-en">EN</button></div><article id="tm-preview" class="template-card" data-no-i18n><strong>${esc(preview.title)}</strong><p>${esc(preview.description)}</p><small>${esc(preview.setup)}</small></article></details>`;
}
function tmProposal(t){const r=templateManager.proposal;if(!r)return '';const rows=[];for(const lang of ['it','en'])for(const field of tmFields)if(typeof r.proposal.locales?.[lang]?.[field]==='string')rows.push({path:`${lang}.${field}`,before:t.locales[lang]?.[field]||'',after:r.proposal.locales[lang][field]});for(const field of ['services','tags'])if(Array.isArray(r.proposal[field])&&r.proposal[field].every(v=>typeof v==='string'))rows.push({path:field,before:t[field].join(', '),after:r.proposal[field].join(', ')});templateManager.proposalRows=rows;return `<h3>Proposta da rivedere</h3>${rows.map((r,i)=>`<label class="tm-diff"><jui-switch data-tm-apply="${i}" aria-label="${esc(uiText('Applica')+' '+tmFieldLabel(r.path))}" checked></jui-switch><b data-no-i18n>${esc(tmFieldLabel(r.path))}</b><del data-no-i18n>${esc(r.before)}</del><ins data-no-i18n>${esc(r.after)}</ins></label>`).join('')}<p><span>Servizi rilevati</span>: <span data-no-i18n>${esc(r.detected_services.join(', '))}</span></p><p data-no-i18n>${esc(r.warnings.map(message=>tmMessage({message})).join('\n'))}</p><button data-tm="apply">Applica selezionati</button><button data-tm="discard">Scarta proposta</button>`}
async function tmClose(){if(templateManager?.busy)return;if(tmDirty()&&!await tmOverlay('Modifiche non salvate','<p>Vuoi scartare le modifiche?</p>',()=>true,'Scarta'))return;templateManager=null;$('#dialog').classList.remove('template-manager-dialog');$('#dialog').oncancel=null;closeModal()}
async function tmSave(extra={}){const m=templateManager;const result=await tmApi('/admin/templates',{catalog:m.catalog,...extra},'PUT');m.catalog=result;m.saved=JSON.stringify(result);tmRender();toast('Catalogo salvato')}
function tmDownload(value,name){const a=document.createElement('a'),url=URL.createObjectURL(new Blob([JSON.stringify(value,null,2)],{type:'application/json'}));a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000)}
function tmFile(){return new Promise(resolve=>{const input=document.createElement('input');input.type='file';input.accept='.json,application/json';input.onchange=async()=>{try{const f=input.files[0];if(!f)return resolve(null);if(f.size>16*1024*1024)throw Error('File troppo grande');resolve(JSON.parse(await f.text()))}catch(e){tmError(e instanceof SyntaxError?new Error('File JSON non valido.'):e);resolve(null)}};input.oncancel=()=>resolve(null);input.click()})}
async function tmChooseWorkflow(allowImport=false){
 const list=await tmApi('/workflows');const selection=await tmOverlay('Scegli workflow',`<label>Workflow<jui-select id="tm-pick">${allowImport?'<jui-option value="__import__">Importa workflow JSON</jui-option>':''}${list.map(w=>`<jui-option value="${esc(w.id)}" data-no-i18n>${esc(w.name)}</jui-option>`).join('')}</jui-select></label>`,()=>$('#tm-pick').value==='__import__'?'__import__':list.find(w=>w.id===$('#tm-pick').value));return selection==='__import__'?tmFile():selection;
}
function tmOverlay(title,body,read,accept='Conferma',ready=null){return new Promise(resolve=>{const dialog=document.createElement('dialog');dialog.className='tm-overlay';dialog.innerHTML=`<div class="modal-head"><h2>${esc(uiText(title))}</h2></div><div class="modal-body">${body}<p data-no-i18n class="error" role="alert"></p></div><div class="modal-actions"><button data-cancel>${uiText('Annulla')}</button><button class="primary" data-accept>${uiText(accept)}</button></div>`;document.body.append(dialog);const done=v=>{dialog.close();dialog.remove();resolve(v)};dialog.querySelector('[data-cancel]').onclick=()=>done(null);dialog.oncancel=e=>{e.preventDefault();done(null)};dialog.querySelector('[data-accept]').onclick=()=>{try{done(read())}catch(e){tmOverlayError(dialog,e)}};dialog.showModal();if(ready)ready(dialog)})}
async function tmAction(action){
 const m=templateManager;if(!m||m.busy)return;m.error=null;const error=$('#tm-error');if(error)error.textContent='';const c=m.catalog,t=tmCurrent(),s=c.sections.find(s=>s.id===m.section);
 if(action==='close')return tmClose();if(action==='save')return tmSave();if(action==='publish')return tmSave({publish:t.id});
 if(action==='export')return tmDownload(c,'workflow-templates.json');
 if(action==='import'){const imported=await tmFile();if(!imported)return;if(imported.schema_version!==1||!Array.isArray(imported.sections)||!Array.isArray(imported.templates))throw Error('Catalogo JSON non valido');await tmApi('/admin/templates/validate',imported);imported.revision=c.revision;m.catalog=imported;m.section=imported.sections[0]?.id;m.selected=null;tmRender();return}
 if(action==='history'){const revisions=await tmApi('/admin/templates/history');const revision=await tmOverlay('Cronologia',`<label>Revisione<jui-select id="tm-history">${revisions.map(n=>`<jui-option value="${n}">${n}</jui-option>`).join('')}</jui-select></label><p>Il ripristino carica una bozza da rivedere prima di salvare o pubblicare.</p>`,()=>$('#tm-history').value);if(revision){const historical=await tmApi('/admin/templates/history/'+revision);historical.revision=c.revision;m.catalog=historical;m.section=historical.sections[0]?.id;m.selected=null;tmRender()}return}
 if(action==='new-section'||action==='edit-section')return tmSection(action==='edit-section'?s:null);
 if(action==='section-up'||action==='section-down'){tmMove(c.sections,s,action==='section-up'?-1:1);tmRender();return}
 if(action==='from-workflow'){if(!s)throw Error('Crea prima una sezione');const w=await tmChooseWorkflow(true);if(!w)return;if(!Array.isArray(w.nodes)||!Array.isArray(w.edges)||!w.name)throw Error('Il file non contiene un workflow valido.');const now=Math.floor(Date.now()/1000),id=crypto.randomUUID();const template={id,section:s.id,order:c.templates.length,status:'draft',locales:{it:tmText(),en:tmText()},services:[],tags:[],source_locale:nodoLocale().startsWith('it')?'it':'en',translation_status:'missing',created:now,updated:now,version:1,provenance:{workflow_id:w.id,copied_at:now,author:app.user.id},workflow:structuredClone(w),published:null};template.locales[template.source_locale].title=w.name;template.locales[template.source_locale].description=w.description;c.templates.push(template);m.selected=id;m.tab='contents';tmRender();return}
 if(!t)return;
 if(action==='preview-it'||action==='preview-en'){const text=t.locales[action.slice(-2)]||{};$('#tm-preview').innerHTML=`<strong>${esc(text.title)}</strong><p>${esc(text.description)}</p><small>${esc(text.setup)}</small>`;return}
 if(action==='duplicate'){const copy=structuredClone(t);copy.id=crypto.randomUUID();copy.published=null;copy.status='draft';copy.version=1;copy.order=c.templates.length;copy.provenance={template_id:t.id,author:app.user.id,copied_at:Math.floor(Date.now()/1000)};c.templates.push(copy);m.selected=copy.id}
 if(action==='up'||action==='down')tmMove(c.templates.filter(v=>v.section===t.section),t,action==='up'?-1:1);
 if(action==='archive')t.status=t.status==='archived'?'draft':'archived';
 if(action==='delete'){const ok=await tmOverlay('Elimina template',`<p data-no-i18n>${esc(catalogText(t).title)}</p>`,()=>true,'Elimina');if(ok){c.templates=c.templates.filter(v=>v.id!==t.id);m.selected=null}}
 if(action==='replace-workflow'||action==='import-workflow'){const w=action==='replace-workflow'?await tmChooseWorkflow():await tmFile();if(!w)return;if(!Array.isArray(w.nodes)||!Array.isArray(w.edges)||!w.name)throw Error('Il file non contiene un workflow valido.');const ok=await tmOverlay('Sostituisci workflow',`<p>${uiText('Rivedi le differenze prima di confermare.')}</p><div class="tm-locales"><pre data-no-i18n>${esc(JSON.stringify(t.workflow,null,2))}</pre><pre data-no-i18n>${esc(JSON.stringify(w,null,2))}</pre></div>`,()=>true);if(ok){t.workflow=structuredClone(w);t.status='draft';t.translation_status='stale';t.provenance={workflow_id:w.id,copied_at:Math.floor(Date.now()/1000),author:app.user.id}}}
 if(action==='test'){const w=structuredClone(t.workflow);w.id=crypto.randomUUID();w.created=0;w.name=(catalogText(t).title||w.name)+' — test';await tmApi('/workflows',w);app.workflows=await tmApi('/workflows');workflowOptions();toast('Workflow di prova creato')}
 if(action==='ai')return tmAI();
 if(action==='apply'){$$('[data-tm-apply][checked]').forEach(el=>{const r=m.proposalRows[Number(el.dataset.tmApply)];if(r.path.includes('.')){const [lang,field]=r.path.split('.');t.locales[lang][field]=r.after}else t[r.path]=m.proposal.proposal[r.path]});m.proposal=null;t.status='draft';t.translation_status='stale'}
 if(action==='discard')m.proposal=null;
 tmRender();
}
function tmRank(items,item){const index=items.indexOf(item),before=items[index-1],after=items[index+1];item.order=before&&after?(before.order+after.order)/2:before?before.order+1:after?after.order-1:0}
function tmMove(items,item,direction){items.sort((a,b)=>a.order-b.order);const i=items.indexOf(item),j=i+direction;if(j<0||j>=items.length)return;[items[i],items[j]]=[items[j],items[i]];tmRank(items,item)}

async function tmSection(section){
 const m=templateManager,s=section?structuredClone(section):{id:crypto.randomUUID(),order:m.catalog.sections.length,status:'draft',locales:{it:tmText(),en:tmText()},published:null};
 const result=await tmOverlay(section?'Modifica sezione':'Nuova sezione',`<div class="tm-locales">${['it','en'].map(lang=>`<div><h3>${lang.toUpperCase()}</h3><label>Titolo<input id="tm-section-${lang}-title" value="${esc(s.locales[lang].title)}"></label><label>Descrizione<textarea id="tm-section-${lang}-description">${esc(s.locales[lang].description)}</textarea></label></div>`).join('')}</div><label>Stato<jui-select id="tm-section-status"><jui-option value="draft">Bozza</jui-option><jui-option value="published" ${s.status==='published'?'selected':''}>Pubblicata</jui-option><jui-option value="archived" ${s.status==='archived'?'selected':''}>Archiviata</jui-option><jui-option value="delete">Elimina sezione vuota</jui-option></jui-select></label><p><span>Modello preferred</span>: <span data-no-i18n>${esc(app.settings.default_runtime)} · ${esc(app.settings.agents[app.settings.default_runtime]?.model||uiText('Predefinito'))}</span></p><label>Richiesta AI<textarea id="tm-section-request" data-tm-default-request="${esc(uiText('Traduci o migliora titolo e descrizione in italiano e inglese.'))}">${esc(uiText('Traduci o migliora titolo e descrizione in italiano e inglese.'))}</textarea></label><button id="tm-section-ai">Genera proposta</button><div id="tm-section-proposal"></div>`,()=>{for(const lang of ['it','en'])for(const field of ['title','description'])s.locales[lang][field]=$(`#tm-section-${lang}-${field}`).value;s.status=$('#tm-section-status').value;return s},'Conferma',dialog=>tmSectionAI(dialog,s));
 if(!result)return;
 if(s.status==='delete'){if(m.catalog.templates.some(t=>t.section===s.id))throw Error('Sposta i template prima di eliminare la sezione');m.catalog.sections=m.catalog.sections.filter(v=>v.id!==s.id);m.section=m.catalog.sections[0]?.id}
 else {const i=m.catalog.sections.findIndex(v=>v.id===s.id);if(i>=0)m.catalog.sections[i]=s;else m.catalog.sections.push(s);m.section=s.id;if(s.status==='published')return tmSave({publish_section:s.id})}
 tmRender();
}
async function tmAI(){
 const m=templateManager,t=tmCurrent(),action=$('#tm-ai-action').value,lang=$('#tm-ai-lang').value,custom=$('#tm-ai-request').value;
 const instructions={analyze:'Analyze workflow and propose all catalog metadata based only on the supplied graph.',title:'Generate only a concise title.',description:'Generate only a concise description.',setup:'Generate setup and prerequisites.',translate:`Translate existing fields from ${t.source_locale} into ${lang==='both'?(t.source_locale==='it'?'en':'it'):lang}. Preserve meaning and technical identifiers.`,check:'Check consistency of current metadata with workflow; propose corrections only where supported.',custom:''};
 if(action==='custom'&&!custom.trim())throw Error('Scrivi una richiesta');m.busy=true;tmMark();
 try{m.proposal=await tmApi('/admin/templates/assist',{template:t,instruction:`${instructions[action]} Target language: ${lang}. ${custom}`});}finally{m.busy=false;tmRender()}
}
window.addEventListener('beforeunload',tmGuard);
window.addEventListener('nodo-language-changed',()=>{
 if($('.workflow-catalog'))showWorkflowCatalog().catch(e=>toast(e.message));
 if(templateManager&&$('#template-manager')){
  const selection=$$('[data-tm-apply]').map(el=>el.checked);tmRender();
  $$('[data-tm-apply]').forEach((el,index)=>{if(index<selection.length)el.checked=selection[index]});
 }
 tmLocalizeOverlays();
});

function tmSectionAI(dialog,section){
 const button=dialog.querySelector('#tm-section-ai');if(!button)return;
 button.onclick=async()=>{button.disabled=true;button.textContent=uiText('Elaborazione…');const error=dialog.querySelector('.error');error.textContent='';dialog.tmError=null;
 try{const copy=structuredClone(section);for(const lang of ['it','en'])for(const k of ['title','description'])copy.locales[lang][k]=dialog.querySelector(`#tm-section-${lang}-${k}`).value;
 const response=await tmApi('/admin/templates/assist',{section:copy,instruction:dialog.querySelector('#tm-section-request').value+' Return only locales.it/en.title and description for this catalog section.'});
 const area=dialog.querySelector('#tm-section-proposal');const rows=[];for(const lang of ['it','en'])for(const k of ['title','description']){const value=response.proposal.locales?.[lang]?.[k];if(typeof value==='string')rows.push({lang,k,value})}
 area.innerHTML=rows.map((r,i)=>`<label class="tm-diff"><jui-switch data-section-apply="${i}" data-tm-proposal-path="${esc(r.lang+'.'+r.k)}" aria-label="${esc(uiText('Applica')+' '+tmFieldLabel(r.lang+'.'+r.k))}" checked></jui-switch><b data-no-i18n data-tm-proposal-label="${esc(r.lang+'.'+r.k)}">${esc(tmFieldLabel(r.lang+'.'+r.k))}</b><del data-no-i18n>${esc(copy.locales[r.lang][r.k])}</del><ins data-no-i18n>${esc(r.value)}</ins></label>`).join('')+'<button id="tm-section-apply">'+uiText('Applica selezionati')+'</button>';
 area.querySelector('button').onclick=()=>{area.querySelectorAll('[data-section-apply][checked]').forEach(el=>{const r=rows[Number(el.dataset.sectionApply)];dialog.querySelector(`#tm-section-${r.lang}-${r.k}`).value=r.value});area.innerHTML=''};
 }catch(e){tmOverlayError(dialog,e)}finally{button.disabled=false;button.textContent=uiText('Genera proposta')}};
}

function resetTemplateManager(){templateManager=null;const dialog=$('#dialog');dialog.classList.remove('template-manager-dialog');dialog.oncancel=null;$$('.tm-overlay').forEach(d=>{d.close();d.remove()})}
