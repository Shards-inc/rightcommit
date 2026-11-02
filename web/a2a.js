'use strict';
let a2aConnections=[];
async function a2aRefresh(){a2aConnections=await api('/a2a/connections')}
async function a2aConnectionDialog(){
 await a2aRefresh();
 modal('Connessioni A2A',`<p class="note">Agent Card A2A 1.0 con binding JSONRPC. Le credenziali sono personali e protette.</p><form id="a2a-connection-form">${select('Connessione','id','',[['','Nuova connessione'],...a2aConnections.map(c=>[c.id,c.name])])}${input('Nome','name','')}${input('URL Agent Card','url','','url')}${input('Token Bearer (vuoto = conserva quello salvato)','token','','password')}</form>`,'<button data-action="a2a-save-connection" class="primary">Salva connessione</button>');
 $('#a2a-connection-form [name=id]').onchange=e=>{const c=a2aConnections.find(c=>c.id===e.target.value);for(const k of ['name','url'])$('#a2a-connection-form [name='+k+']').value=c?.[k]||''};
}
async function a2aPublicationDialog(){
 if(app.run)throw Error('Torna alla progettazione per configurare A2A');
 await saveWorkflow(); const p=await api('/a2a/workflows/'+app.wf.id);
 modal('Esponi workflow come agente A2A',`<p class="note">Ogni richiesta avvia una run locale. Le approvazioni restano nell’app. Salva per pubblicare la versione corrente del workflow. L’app deve essere raggiungibile e restare accesa.</p><form id="a2a-publication-form">${select('Accesso A2A','enabled',String(p.enabled),[['false','Disabilitato'],['true','Abilitato con token dedicato']])}${input('Nome pubblico','name',p.name||app.wf.name)}<label>Descrizione<textarea name="description">${esc(p.description||app.wf.description||'')}</textarea></label>${input('Indirizzo pubblico dell’app','base_url',p.base_url||location.origin,'url')}<p class="note">Risultati da restituire al client:</p>${app.wf.nodes.filter(n=>!['Source','Agent','MCP','RAG','Comment'].includes(n.kind)).map(n=>`<label><jui-switch name="outputs" value="${esc(n.id)}" ${p.outputs.includes(n.id)?'checked':''}></jui-switch><span data-no-i18n>${esc(n.title)}</span></label>`).join('')}${select('Credenziale','rotate','false',[['false','Conserva token attuale'],['true','Genera nuovo token e revoca il precedente']])}</form><p class="note">Agent Card: <span class="code">${esc((p.base_url||location.origin).replace(/\/$/,'')+p.path+'/.well-known/agent-card.json')}</span></p>`,'<button data-action="a2a-publish" class="primary">Salva esposizione</button>');
}
document.addEventListener('DOMContentLoaded',()=>{
 $('#a2a-publication').onclick=()=>a2aPublicationDialog().catch(e=>toast(e.message));
});
document.addEventListener('click',async e=>{
 const b=e.target.closest('[data-action]');if(!b)return;
 try{switch(b.dataset.action){
 case 'a2a-connections':await a2aConnectionDialog();break;
 case 'a2a-save-connection':{const f=$('#a2a-connection-form');if(!f.reportValidity())return;await api('/a2a/connections',Object.fromEntries(new FormData(f)));await a2aRefresh();closeModal();renderInspector();toast('Connessione A2A salvata');break;}
 case 'a2a-probe':{const id=$('#node-form [name=a2a_id]').value;if(!id)throw Error('Seleziona una connessione');const card=await api('/a2a/connections/'+id+'/probe',{});toast('Agente disponibile: '+card.name);break;}
 case 'a2a-publish':{const f=$('#a2a-publication-form');if(!f.reportValidity())return;const d=Object.fromEntries(new FormData(f));d.enabled=d.enabled==='true';d.rotate=d.rotate==='true';d.outputs=new FormData(f).getAll('outputs');const r=await api('/a2a/workflows/'+app.wf.id,d);if(r.token){modal('Token A2A creato',`<p>Copialo ora: non verrà mostrato di nuovo.</p><textarea readonly rows="3">${esc(r.token)}</textarea>`,'<button data-action="close-dialog">Chiudi</button>')}else{closeModal();toast('Esposizione A2A aggiornata')}break;}
 }}catch(error){toast(error.message)}
});
