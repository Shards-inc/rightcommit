'use strict';
(()=>{
 const cache=new Map();let open=false,faceCollapsed=false,scope='',historyRequest=0,previousDraft=null,owner=null;
 function resizeInput(input){
  const style=getComputedStyle(input),line=parseFloat(style.lineHeight);
  const padding=parseFloat(style.paddingTop)+parseFloat(style.paddingBottom);
  const border=parseFloat(style.borderTopWidth)+parseFloat(style.borderBottomWidth);
  const max=3*line+padding+border;
  input.style.height='0px';
  const height=input.scrollHeight+border;
  input.style.height=Math.min(height,max)+'px';
  input.style.overflowY=height>max?'auto':'hidden';
 }
 let inputWidth=0;
 const inputObserver=new ResizeObserver(entries=>{for(const entry of entries){if(entry.contentRect.width!==inputWidth){inputWidth=entry.contentRect.width;resizeInput(entry.target)}}});
 const key=()=>`${app.user?.id}:${currentWorkflow()?.id}:${app.run?.id||'design'}`;
 const data=()=>{if(!cache.has(scope))cache.set(scope,{messages:[],input:'',busy:false,version:0,proposal:null,undo:null,error:'',proposalKind:'edit'});return cache.get(scope)};
 const labels={
 'minimize-2':['Riduci intestazione manager','Nasconde la faccina per lasciare più spazio ai messaggi della chat.'],
 'maximize-2':['Espandi intestazione manager','Mostra di nuovo la faccina del manager.'],
 'sticky-note':['Commento','Aggiunge una nota libera, spostabile e ridimensionabile, con anteprima Markdown. Si usa per spiegare il workflow senza creare un passo eseguibile.'],
 'folder-git-2':['Sorgenti e Storage','Collega codice o materiali al workflow; in Storage permette di consultare e scaricare sorgenti e risultati.'],
 'bot':['Agente e modalità di esecuzione','Sceglie chi svolge i task o supervisiona le decisioni. Si usa per assegnare responsabilità e modello.'],
 'list-todo':['Task','Definisce un’attività e le istruzioni per l’agente. Si usa per suddividere il lavoro in passi verificabili.'],
 'clipboard-check':['Review','Controlla il lavoro con un agente, una persona o entrambi. Si usa prima di procedere a una fase importante.'],
 'git-branch':['Gate e diramazioni','Valuta criteri e seleziona il percorso successivo, anche un giro di correzioni. Si usa per governare avanzamento e qualità.'],
 'brain-circuit':['Memory','Passa il risultato di un task a un altro agente. Si usa per mantenere il contesto tra responsabilità diverse.'],
 'plug':['Connection','Esegue operazioni locali o tramite servizi collegati. Si usa per test, commit e integrazioni esterne configurate.'],
 'workflow':['Workflow e sotto-workflow','Rappresenta un processo o richiama un workflow figlio. Si usa per riutilizzare fasi e consultare le esecuzioni collegate.'],
 'a2a-client':['A2A · Scambio tra agenti','Due frecce contrapposte rappresentano lo scambio di richieste e risposte. Identifica il nodo client A2A e la pubblicazione di un workflow come agente.'],
 'mcp-client':['MCP','Aggiunge strumenti di un server MCP all’agente. Si usa quando servono capacità o dati di un servizio connesso.'],
 'database-rag':['RAG','Indicizza e cerca materiali di riferimento. Si usa per trovare contenuti pertinenti senza leggere tutto in ogni turno.'],
 'plus':['Aggiungi e aumenta zoom','Crea un workflow o aumenta lo zoom, secondo la posizione del pulsante. Si usa per iniziare un processo o leggere meglio il grafo.'],
 'pencil':['Rinomina','Cambia il nome del workflow. Si usa per riconoscerne lo scopo nell’elenco.'],
 'copy':['Duplica','Crea una copia del workflow. Si usa come punto di partenza per una variante.'],
 'trash-2':['Elimina','Rimuove un nodo o, dopo conferma, un workflow e le sue run. Si usa per pulire il progetto; i materiali in /sources sono protetti.'],
 'list-checks':['Valida','Controlla struttura, associazioni e collegamenti del grafo. Si usa prima di avviare il lavoro.'],
 'save':['Salva','Registra la bozza o le configurazioni. Si usa per conservare le modifiche, incluse quelle del manager.'],
 'settings-2':['Configurazioni','Imposta agenti, connessioni e preferenze personali. Si usa per adattare l’app al proprio ambiente.'],
 'play':['Esegui o riprendi','Avvia una run o riprende una run sospesa. Si usa quando il workflow è pronto o è stata risolta una richiesta.'],
 'pause':['Pausa','Chiede di fermare la run al termine del turno attivo. Si usa per intervenire senza interrompere un’operazione a metà.'],
 'list':['Vista esecuzione','Mostra i passi e i loro stati in elenco. Si usa per seguire il lavoro in ordine.'],
 'history':['Cronologia','Apre run precedenti o decisioni registrate. Si usa per ricostruire cosa è successo.'],
 'pencil-ruler':['Progettazione e ripartenza','Torna alla progettazione riapre la bozza. Modifica e continua conserva il lavoro compatibile in una nuova run, dopo l’anteprima dei risultati mantenuti e dei nodi da rieseguire.'],
 'scan':['Inquadra','Adatta il grafo allo spazio disponibile. Si usa per ritrovare tutti i nodi.'],
 'locate-fixed':['Centra nodo corrente','Porta al centro il passo attivo. Si usa per seguire una run su grafi grandi.'],
 'expand':['Espandi','Amplia o ripristina l’area del grafo. Si usa per concentrarsi sul processo.'],
 'minus':['Riduci zoom','Allontana la vista. Si usa per leggere la struttura complessiva.'],
 'fingerprint':['Sessioni native','Mostra le conversazioni reali degli agenti. Si usa per verificare continuità e identità delle sessioni.'],
 'terminal':['Registro attività','Apre eventi e output della run. Si usa per capire progressi, errori e risultati effettivi.'],
 'circle-help':['Guida, domande e review','Qui apre la legenda; nelle run segnala una richiesta o permette un chiarimento. Si usa per capire l’interfaccia o fornire una decisione.'],
 'project-manager':['Project manager','Apre una chat che conosce workflow e prove della run. Si usa per spiegare lo stato e creare o modificare una bozza con nodi e collegamenti.'],
 'chart-finops':['FinOps','Mostra uso, tempi e costi registrati degli agenti. Si usa per valutare le risorse impiegate.'],
 'x':['Chiudi','Chiude il pannello o la finestra corrente. Si usa per liberare spazio senza eliminare dati.'],
 'check':['Confermato','Indica una scelta o un controllo riuscito. Si usa per riconoscere un esito positivo.'],
 'eye':['Anteprima','Mostra un contenuto leggibile. Si usa per consultare un artefatto senza modificarlo.'],
 'code':['Sorgente','Mostra il testo originale o il codice. Si usa per ispezionare il contenuto esatto.'],
 'file-text':['File e documenti','Apre o carica un documento. Si usa per consultare materiali e risultati.'],
 'git-compare-arrows':['Confronto e anteprima','Confronta le versioni degli artefatti. In Anteprima ripartenza mostra risultati mantenuti, nodi da rieseguire e file copiati prima di creare la nuova run.'],
 'loader-circle':['Operazione in corso','Segnala un’attesa. Si usa per distinguere un processo attivo da un risultato già disponibile.'],
 'log-in':['Ingresso e accesso OAuth','Imposta il primo nodo eseguibile del workflow; nei server MCP avvia l’accesso OAuth.'],
 'download':['Scarica','Esporta un file o una cartella in ZIP. Si usa per portare i risultati fuori dall’app.'],
 'rotate-cw':['Ripeti o ricostruisci','Ripete una verifica o ricostruisce un indice. Si usa quando serve un nuovo risultato.'],
 'refresh-cw':['Aggiorna','Rilegge dati o aggiorna un indice. Si usa dopo una modifica dei contenuti.'],
 'arrow-left':['Indietro','Torna al livello o workflow precedente. Si usa per risalire alla vista principale.'],
 'search':['Cerca','Filtra o cerca nei contenuti. Si usa per trovare rapidamente un elemento.'],
 'brand-pi':['Pi Agent','Identifica l’agente Pi. Si usa per riconoscere il runtime multi-provider assegnato.'],
 'brand-openai':['Codex / OpenAI','Identifica l’agente Codex. Si usa per riconoscere il runtime assegnato.'],
 'brand-claude':['Claude Code','Identifica l’agente Claude. Si usa per riconoscere il runtime assegnato.'],
 'brand-github':['GitHub','Identifica una connessione GitHub. Si usa per distinguere le operazioni sul repository remoto.'],
 'brand-azuredevops':['Azure DevOps','Identifica una connessione Azure DevOps. Si usa per distinguere il servizio collegato.'],
 'brand-servicenow':['ServiceNow','Identifica una connessione ServiceNow. Si usa per riconoscere le operazioni sui ticket.']
 };
 const quickStart=[["settings-2", "Configura il tuo ambiente", "Apri Configurazioni: in Agents scegli modello, credenziali e Default; in MCP e Connections collega i servizi. Aspetto offre Light, Dark e System. Salva le configurazioni."], ["plus", "Crea un workflow", "Premi + per partire da zero o da un modello. Rinomina il workflow e usa Duplica per creare una variante."], ["workflow", "Disegna e collega i passi", "Trascina i nodi dalla palette. Seleziona ogni nodo e compila le istruzioni: le modifiche vengono salvate automaticamente. Collega le porte; associa un Agent ai Task e scegli il nodo di ingresso."], ["folder-git-2", "Scegli le sorgenti", "Nel nodo Source scegli materiali, repository o esempi. La modalità permanente modifica gli originali; la copia lavora nel workspace. In Storage carichi, consulti e scarichi file; Show hidden mostra i file nascosti."], ["git-branch", "Controlla qualità e contesto", "Usa Review per una verifica e Gate per scegliere il percorso in base ai criteri. Collega i percorsi di esito e correzione. Memory trasferisce risultati tra agenti diversi."], ["play", "Salva, valida ed esegui", "Salva la bozza, premi Valida e risolvi gli errori. Scegli la modalità di esecuzione e avvia il workflow. Le richieste di review o chiarimento si aprono dalla run; Pausa si applica al termine del turno attivo."], ["pencil-ruler", "Modifica una run e continua", "Nella run premi Modifica e continua. Attendi la pausa, aggiungi o modifica i nodi e apri Anteprima ripartenza: controlla risultati mantenuti, nodi da rieseguire e file copiati. Conferma per avviare una nuova run dal punto proposto o da uno precedente."], ["project-manager", "Lavora con il manager", "Apri la chat e descrivi cosa vuoi creare, cambiare o capire. Il manager propone modifiche alla bozza e legge le prove della run. Controlla il risultato e premi Salva; avvia tu l’esecuzione."], ["mcp-client", "Aggiungi strumenti e integrazioni", "Configura un server nella scheda MCP e verifica la connessione. Collega un nodo MCP agli Agent che ne useranno i tool. Per GitHub, Azure DevOps, ServiceNow o Atlassian configura il servizio e usa Connection."], ["database-rag", "Cerca nei materiali con RAG", "In Storage usa RAG Index per indicizzare una cartella. Configura il nodo RAG e collegalo agli Agent che devono cercare nei documenti. Aggiorna l’indice quando cambiano i materiali."], ["a2a-client", "Collega agenti con A2A", "Usa il nodo A2A per chiamare un agente remoto tramite la sua connessione. Il pulsante A2A nella barra del workflow permette di esporre il workflow come agente; configura accesso e risultati da pubblicare."], ["sticky-note", "Annota e naviga nel grafo", "Comment aggiunge note spostabili, ridimensionabili e con anteprima Markdown; non viene eseguito. Trascina lo sfondo per spostarti, usa +/− o Inquadra per lo zoom e Scorri/Wheel per abilitare la rotellina. In Design, Shift + trascinamento sullo sfondo seleziona più nodi: trascinali insieme o cancella la selezione. Ctrl/Cmd+Z annulla la cancellazione."], ["chart-finops", "Consulta risultati e consumi", "Executions e Cronologia mostrano le run; il registro attività aiuta a seguire errori e progressi. Apri gli artefatti per leggere i risultati. FinOps include i consumi disponibili dei workflow e del manager; i valori mancanti restano indicati con —."]];
 quickStart.splice(5,0,['parallel-fork','Esegui Task in parallelo',"Aggiungi Fork e Join dalla palette. Seleziona il Join nel Fork e collega Fork → Task → … → Join → passo successivo, con almeno due rami e Agent distinti. Ogni ramo supporta solo Task, senza Fork annidati; colloca Review, Gate e altri nodi prima del Fork o dopo il Join. Assegna file diversi ai rami nel workspace condiviso. Il Join prosegue quando tutti i rami sono completati."]);
 $('#icon-guide').onclick=()=>{
  const entries=Object.entries(labels).map(([id,[title,text]])=>({id,icon:id,title,text}));
  entries.push({id:'Fork',icon:'parallel-fork',title:'Fork · Avvio parallelo',text:"Avvia almeno due rami di Task in parallelo. Seleziona nel campo Nodo di ricongiungimento il Join in cui devono terminare tutti i rami. Usa Agent distinti fra i rami e assegna file diversi: il workspace è condiviso."},{id:'Join',icon:'parallel-join',title:'Join · Ricongiungimento',text:"Attende il completamento di tutti i rami del Fork e raccoglie i risultati prima di proseguire. Un errore o un chiarimento impedisce di superare il Join; i Task già completati vengono conservati alla ripresa."});
  entries.push({id:'atlassian',icon:'plug',title:'Atlassian',text:'Collega Jira, Confluence, Bitbucket e gli altri servizi tramite Teamwork Graph CLI. Configura l’accesso in Connections e usa un nodo Connection con provider Atlassian.'});
  const renderRows=items=>items.map(({id,icon:picture,title,text})=>`<article class="icon-guide-row" data-guide-row data-guide-id="${id}" data-guide-keywords="${id==='sticky-note'?'Comment Commento Note Markdown':id==='a2a-client'?'A2A Agent to Agent':id}"><span class="icon-guide-picture">${icon(picture,picture.startsWith('brand-')?'brand-icon':'')}</span><div><h3>${esc(uiText(title))}</h3><p>${esc(uiText(text))}</p></div></article>`).join('');
  const sidebarIds=new Set([...Object.values(icons),'Fork','Join','fingerprint','terminal','circle-help','project-manager']);
  const rows=`<section data-guide-group><h3 class="icon-guide-group-title">${esc(uiText('Sidebar sinistra'))}</h3><div class="icon-guide-grid">${renderRows(entries.filter(e=>sidebarIds.has(e.id)))}</div></section><hr class="icon-guide-separator"><section data-guide-group><h3 class="icon-guide-group-title">${esc(uiText('Altre icone'))}</h3><div class="icon-guide-grid">${renderRows(entries.filter(e=>!sidebarIds.has(e.id)))}</div></section>`;
  const quick=quickStart.map(([picture,title,text],i)=>`<article class="quick-start-step"><span class="icon-guide-picture">${icon(picture)}</span><div><h3>${i+1}. ${esc(uiText(title))}</h3><p>${esc(uiText(text))}</p></div></article>`).join('');
  modal('Guida',`<div class="tabs help-tabs" role="tablist" aria-label="Guida"><button type="button" id="help-tab-icons" role="tab" aria-selected="true" aria-controls="help-panel-icons" data-help-tab="icons" class="active">Icon Guide</button><button type="button" id="help-tab-quick" role="tab" aria-selected="false" aria-controls="help-panel-quick" data-help-tab="quick" tabindex="-1">Quick Start</button></div><section id="help-panel-icons" role="tabpanel" aria-labelledby="help-tab-icons"><p class="note">Per ogni icona trovi il significato e quando usarla. La stessa icona può avere funzioni diverse secondo la posizione.</p><label>Cerca nella guida<input id="icon-guide-search" type="search" placeholder="Cerca un’icona o una funzione…"></label>${rows}</section><section id="help-panel-quick" role="tabpanel" aria-labelledby="help-tab-quick" hidden><div class="quick-start-grid">${quick}</div></section>`,'',true);
  const tabs=$$('[data-help-tab]');
  const activate=tab=>tabs.forEach(b=>{const active=b===tab;b.classList.toggle('active',active);b.setAttribute('aria-selected',String(active));b.tabIndex=active?0:-1;$('#'+b.getAttribute('aria-controls')).hidden=!active});
  tabs.forEach((tab,i)=>{tab.onclick=()=>activate(tab);tab.onkeydown=e=>{let next;if(e.key==='ArrowRight'||e.key==='ArrowLeft')next=1-i;else if(e.key==='Home')next=0;else if(e.key==='End')next=1;else return;e.preventDefault();activate(tabs[next]);tabs[next].focus()}});
  $('#icon-guide-search').oninput=e=>{const term=e.target.value.toLocaleLowerCase(nodoLocale());$$('[data-guide-row]').forEach(row=>row.hidden=!(row.textContent+' '+row.dataset.guideKeywords).toLocaleLowerCase(nodoLocale()).includes(term));const groups=$$('[data-guide-group]');groups.forEach(group=>group.hidden=!group.querySelector('[data-guide-row]:not([hidden])'));$('.icon-guide-separator').hidden=groups.some(group=>group.hidden)};$('#icon-guide-search').focus();
 };
 function managerFace(state){
  const expression=state.busy?'is-thinking':state.error?'is-concerned':'is-awake';
  return `<span id="manager-face" class="manager-face ${expression}" aria-hidden="true" ${faceCollapsed?'hidden':''}><span class="manager-face-orb"><svg viewBox="0 0 80 80" class="manager-face-drawing"><g class="manager-face-features"><g class="manager-face-eyes"><rect class="manager-face-eye" x="23" y="29" width="9" height="15" rx="4.5"/><rect class="manager-face-eye" x="48" y="29" width="9" height="15" rx="4.5"/></g><path class="manager-face-smile" d="M30 52 Q40 59 50 52"/></g></svg><span class="manager-face-blush left"></span><span class="manager-face-blush right"></span></span><span class="manager-face-spark"></span><span class="manager-face-thoughts"><i></i><i></i><i></i></span></span>`;
 }
 function renderChat(){
  if(!open)return;const state=data();
  $('#manager-panel').innerHTML=`<div class="manager-heading"><div class="manager-header-actions"><button type="button" id="manager-reset" class="manager-reset" ${state.busy?'disabled':''} title="Azzera chat">${icon('rotate-cw')}Azzera chat</button><button type="button" class="icon-button" id="manager-compact" aria-expanded="${!faceCollapsed}" aria-controls="manager-face" aria-label="${esc(uiText(faceCollapsed?'Mostra faccina':'Nascondi faccina'))}" title="${esc(uiText(faceCollapsed?'Mostra faccina':'Nascondi faccina'))}">${icon(faceCollapsed?'maximize-2':'minimize-2')}</button><button type="button" class="icon-button" id="manager-close" aria-label="Chiudi chat">${icon('x')}</button></div>${managerFace(state)}<div class="manager-title"><b>Project manager</b><small id="manager-context"></small></div></div><div id="manager-messages" role="log" aria-label="Messaggi del project manager" aria-live="polite">${state.messages.length?state.messages.map(m=>`<article data-no-i18n class="manager-message ${m.role==='user'?'from-user':'from-manager'}"><b>${m.role==='user'?uiText('Tu'):'Project manager'}</b><div class="${m.role==='assistant'?'manager-markdown markdown':''}">${m.role==='assistant'&&m.html?m.html:esc(m.text).replace(/\n/g,'<br>')}</div></article>`).join(''):`<div class="manager-empty">${icon('project-manager')}<p>Descrivi il workflow che vuoi creare, chiedi una modifica ai nodi o domandami a che punto è la run.</p></div>`}${state.busy?`<p class="manager-thinking" role="status">${icon('loader-circle')}Il manager sta analizzando il contesto…</p>`:''}</div><div id="manager-feedback" role="status">${state.notice?`<p class="manager-notice">${esc(state.notice)}</p>`:''}${state.warning?`<p class="manager-warning">${esc(uiText('Da completare prima di Esegui:'))} ${esc(state.warning)}</p>`:''}${state.error?`<section class="manager-error" role="alert"><button type="button" class="icon-button" id="manager-dismiss-error" aria-label="Chiudi errore">${icon('x')}</button><p>${esc(uiText(state.error))}</p><button type="button" id="manager-error-settings">${esc(uiText('Impostazioni agenti'))}</button></section>`:''}${state.proposal?`<div class="manager-proposal"><p>Proposta pronta. La run e le modifiche fatte nel frattempo restano conservate.</p><button type="button" id="manager-apply">${icon('workflow')}${app.run||state.proposalKind==='create'?uiText('Apri come nuovo workflow'):uiText('Applica alla bozza')}</button></div>`:''}${state.undo?'<button type="button" id="manager-undo">Annulla ultima modifica del manager</button>':''}${previousDraft&&!app.run?'<button type="button" id="manager-restore">Torna alla bozza precedente</button>':''}</div><form id="manager-form"><label for="manager-input">Messaggio al manager</label><div class="manager-compose-row"><textarea id="manager-input" rows="1" maxlength="32000" placeholder="Es. aggiungi una review dopo i test…" required>${esc(state.input)}</textarea><button type="submit" class="primary manager-send" ${state.busy?'disabled':''}>${icon('play')}Invia</button></div><span class="small manager-runtime">${esc(app.settings?.default_runtime||state.runtime||'')} · ${uiText('Credenziali personali')}</span></form>`;
  $('#manager-compact').onclick=()=>{
    faceCollapsed=!faceCollapsed;
    $('#manager-face').hidden=faceCollapsed;
    const button=$('#manager-compact'),label=uiText(faceCollapsed?'Mostra faccina':'Nascondi faccina');
    button.innerHTML=icon(faceCollapsed?'maximize-2':'minimize-2');
    button.setAttribute('aria-expanded',String(!faceCollapsed));button.setAttribute('aria-label',label);button.title=label;
  };
  updateContextLabel();$('#manager-close').onclick=()=>toggle(false);$('#manager-reset').onclick=resetChat;$$('.manager-markdown a').forEach(a=>{a.target='_blank';a.rel='noopener noreferrer'});
  const input=$('#manager-input');
  $('#manager-dismiss-error')?.addEventListener('click',()=>{
   state.error='';$('#manager-feedback .manager-error')?.remove();
   const face=$('#manager-face');if(face){const holder=document.createElement('div');holder.innerHTML=managerFace(state);face.replaceWith(holder.firstElementChild)}
   input.focus();
  });
  $('#manager-error-settings')?.addEventListener('click',()=>settingsDialog('agents'));
  input.oninput=e=>{state.input=e.target.value;resizeInput(e.target)};
  inputObserver.disconnect();inputWidth=0;resizeInput(input);inputObserver.observe(input);
  $('#manager-input').onkeydown=e=>{if(e.key==='Enter'&&!e.shiftKey&&!e.isComposing){e.preventDefault();if(!state.busy)$('#manager-form').requestSubmit()}};
  $('#manager-form').onsubmit=send;
  $('#manager-restore')?.addEventListener('click',()=>{const current={workflow:structuredClone(app.wf),dirty:app.dirty};app.wf=previousDraft.workflow;app.dirty=previousDraft.dirty;previousDraft=current;app.selected=null;workflowOptions(app.wf.id);rememberWorkflow(app.wf.id);render();fit();renderChat()});
  $('#manager-apply')?.addEventListener('click',()=>applyPending(state));
  $('#manager-undo')?.addEventListener('click',()=>{if(!app.run&&JSON.stringify(app.wf)===state.undo.after){app.wf=state.undo.before;state.undo=null;dirty();render();renderChat()}else{state.error=uiText('La bozza è cambiata: annullamento automatico non disponibile.');renderChat()}});
  $('#manager-messages').scrollTop=$('#manager-messages').scrollHeight;
 }
 function updateContextLabel(){if($('#manager-context'))$('#manager-context').textContent=`${currentWorkflow()?.name||''}${app.run?' · '+app.run.id.slice(0,8)+' · '+(states[app.run.status]||app.run.status):' · '+uiText('Bozza')}`}
 function toggle(value=!open){open=value;$('#manager-panel').hidden=!open;$('#manager-toggle').setAttribute('aria-expanded',String(open));$('#manager-toggle').classList.toggle('active',open);if(open){app.selected=null;renderInspector();sync(true);$('#manager-input')?.focus()}}
 window.addEventListener('settings-saved',()=>renderChat());
 $('#manager-toggle').onclick=()=>toggle();
 async function sync(force=false){
  if(owner!==app.user?.id){owner=app.user?.id;previousDraft=null;scope='';cache.clear()}
  if(!open)return;updateContextLabel();const next=key();if(next===scope&&!force)return;
  const changed=next!==scope;scope=next;const state=data();if(changed){state.notice='';state.warning=null}if(state.busy){renderChat();return}const version=state.version,request=++historyRequest;renderChat();
  try{const q=new URLSearchParams({workflow_id:currentWorkflow().id});if(app.run)q.set('run_id',app.run.id);const history=await api('/manager/chat?'+q);if(request===historyRequest&&key()===next&&state.version===version){state.messages=history.messages;state.runtime=history.runtime;state.conversationId=history.conversation_id;if(history.proposal&&JSON.stringify({...currentWorkflow(),created:0})!==JSON.stringify({...history.proposal.workflow,created:0})){state.proposal=history.proposal.workflow;state.proposalKind=history.proposal.kind;state.warning=history.proposal.warning}renderChat()}}catch(error){if(scope===next){state.error=error.message;renderChat()}}
 }
 window.nodoManagerContextChanged=()=>sync();
 function applyWorkflow(workflow,state){const before=structuredClone(app.wf);app.wf=structuredClone(workflow);state.undo={before,after:JSON.stringify(app.wf)};state.proposal=null;dirty();render();fit();renderChat()}
 async function applyPending(state){
  if(!state.proposal)return;
  if(app.run||state.proposalKind==='create'){
   if(app.wf)previousDraft={workflow:structuredClone(app.wf),dirty:app.dirty};
   const draft=structuredClone(state.proposal);draft.created=0;
   state.proposal=null;disconnect();app.run=null;app.wf=draft;app.children=[];app.reviewRun=null;app.artifacts=[];app.decisions=[];app.selected=null;app.view='graph';dirty();workflowOptions(draft.id);const transferred={...state,messages:state.messages.slice(),input:'',busy:false,proposal:null,undo:null};cache.set(key(),transferred);scope=key();render();fit();renderChat();
  }else{
   if(state.base&&JSON.stringify(app.wf)!==state.base&&!await appConfirm({title:'Bozza modificata',message:'La bozza è cambiata dopo la richiesta. Vuoi applicare la proposta del manager? Puoi annullarla subito dopo.',accept:'Applica proposta'}))return;
   applyWorkflow(state.proposal,state);
  }
 }
 async function resetChat(){
  const state=data();if(state.busy)return;const current=scope;state.busy=true;renderChat();
  try{await api('/manager/chat/reset',{workflow_id:currentWorkflow().id,run_id:app.run?.id||null});if(scope===current){cache.set(scope,{messages:[],input:'',busy:false,version:state.version+1,proposal:null,undo:null,error:'',proposalKind:'edit',notice:uiText('Chat azzerata. Il prossimo messaggio apre una nuova sessione.')});renderChat()}}
  catch(error){state.busy=false;state.error=error.message;if(scope===current)renderChat()}
 }
 async function send(event){
  event.preventDefault();const state=data(),message=state.input.trim();if(state.busy||!message)return;
  const originalDraft=state.input,originalScope=scope,workflow=structuredClone(currentWorkflow()),base=JSON.stringify(workflow),runId=app.run?.id||null;
  state.busy=true;state.version++;state.error='';state.notice='';state.input='';if(state.messages.at(-1)?.role!=='user'||state.messages.at(-1)?.text!==message)state.messages.push({role:'user',text:message});renderChat();$('#manager-input')?.focus();
  try{
   const response=await api('/manager/chat',{workflow,run_id:runId,message,conversation_id:state.conversationId||null});state.messages=response.messages;state.runtime=response.runtime;state.conversationId=response.conversation_id;state.warning=response.validation_warning;
   if(response.workflow){
    state.base=base;state.proposalKind=response.kind||'edit';
    state.notice=uiText('Editor aggiornato:')+' '+response.workflow.nodes.length+' '+uiText('nodi')+', '+response.workflow.edges.length+' '+uiText('collegamenti');
    if(originalScope===key()&&!app.run&&JSON.stringify(app.wf)===base){if(response.kind==='create'){state.proposal=response.workflow;await applyPending(state)}else{applyWorkflow(response.workflow,state)}}else{state.notice='';state.proposal=response.workflow}
   }
  }catch(error){state.error=error.message;if(!state.input)state.input=originalDraft}
  finally{
   state.busy=false;
   if(scope===originalScope&&open){
    const input=$('#manager-input'),focused=document.activeElement===input,selection=input?[input.selectionStart,input.selectionEnd]:null;
    renderChat();
    if(!document.querySelector('dialog[open]')&&(focused||state.error)){
     const next=$('#manager-input');next.focus();
     if(selection&&focused&&next.value===input.value)next.setSelectionRange(...selection);else next.setSelectionRange(next.value.length,next.value.length);
    }
   }
  }
 }
})();
