'use strict';
// Scheduling uses the app dialog, account-menu pattern, JUI pickers and switches.
const scheduleText=s=>uiText(s);
const scheduleDate=(at,tz)=>at?new Date(at*1000).toLocaleString(nodoLocale(),tz?{timeZone:tz}:undefined):'—';
const scheduleState=s=>!s.next?'Conclusa':s.config.active?'Attiva':'In pausa';
const scheduleOutcome=state=>scheduleText(({error:'Errore',skipped:'Avvio non eseguito',preparing:'Preparazione',started:'Avviata'})[state]||states[state]||state||'—');
async function refreshSchedules(){const owner=app.user?.id;if(!owner)return;const rows=await api('/schedules');if(app.user?.id===owner)app.schedules=rows;}
function scheduleRule(c){const label={once:'Esecuzione singola',minutes:'Ogni N minuti',hours:'Ogni N ore',daily:'Giornaliera',weekly:'Settimanale'}[c.kind];return scheduleText(label).replace('N',String(c.every))+(c.kind==='weekly'?' · '+c.weekdays.map(d=>scheduleText(['Lunedì','Martedì','Mercoledì','Giovedì','Venerdì','Sabato','Domenica'][d])).join(', '):'')+' · '+c.start.replace('T',' ')+' · '+c.timezone+(c.end?' → '+c.end.replace('T',' '):'');}
function scheduleRows(workflow=null){
 const rows=(app.schedules||[]).filter(r=>!workflow||r.schedule.config.workflow_id===workflow);
 return rows.map(({schedule:s,workflow_name,current,last})=>`<button type="button" class="history-row schedule-row" data-schedule-open="${esc(s.id)}">${icon('history')}<span><b data-no-i18n>${esc(s.config.name)}</b><small data-no-i18n>${workflow_name&&workflow_name.trim()!==s.config.name.trim()?esc(workflow_name)+' · ':''}${esc(scheduleRule(s.config))}</small><small>${scheduleText('Prossimo avvio')}: <span data-no-i18n>${s.config.active?scheduleDate(s.next,s.config.timezone):'—'}</span> · ${scheduleText('Ultimo esito')}: ${esc(scheduleOutcome(last?.state))}${last?` · <span data-no-i18n>${scheduleDate(last.due,s.config.timezone)}</span>`:''}</small>${current?`<small>${scheduleText((current.needs_attention||['paused','waiting_review','waiting_user'].includes(current.status))?'In attesa del tuo intervento':'Esecuzione corrente')}: ${esc(scheduleText(states[current.status]||current.status))}</small>`:''}</span><span class="status-label">${scheduleText(scheduleState(s))}</span></button>`).join('');
}
function scheduleLive(s,current,history){
 const currentHtml=current?`<p>${scheduleText((current.needs_attention||['paused','waiting_review','waiting_user'].includes(current.status))?'In attesa del tuo intervento':'Esecuzione corrente')}</p><button type="button" data-schedule-run="${esc(current.id)}">${icon('play')}${scheduleText('Apri esecuzione corrente')}</button>${(current.needs_attention||['paused','waiting_review','waiting_user'].includes(current.status))?`<p class="note">${scheduleText('I nuovi avvii saranno saltati finché questa esecuzione non sarà terminata.')}</p>`:''}`:'';
 const rows=history.map(h=>`<div class="history-row"><span data-no-i18n>${scheduleDate(h.due,s.config.timezone)}</span><span>${esc(scheduleOutcome(h.state))}${h.message?`<small>${esc(scheduleText(h.message))}</small>`:''}</span>${h.run_id?`<button type="button" data-schedule-run="${esc(h.run_id)}">${scheduleText('Apri esecuzione')}</button>`:''}</div>`).join('');
 return currentHtml+`<h3>${scheduleText('Storico esecuzioni')}</h3>`+(rows||`<p class="note">${scheduleText('Nessuna esecuzione programmata ancora avviata.')}</p>`);
}
async function scheduleDetails(id){
 await refreshSchedules();const row=(app.schedules||[]).find(r=>r.schedule.id===id);if(!row)return;
 const {schedule:s,current}=row;const history=await api('/schedules/'+id+'/history');
 modal('Pianificazione',`<div data-schedule-detail="${esc(id)}"><h3 data-no-i18n>${esc(s.config.name)}</h3><p data-no-i18n>${esc(scheduleRule(s.config))}</p><label class="schedule-active"><jui-switch id="schedule-active" aria-label="Pianificazione attiva" ${s.config.active&&s.next?'checked':''} ${!s.next?'disabled':''}></jui-switch><span>${scheduleText(scheduleState(s))}</span></label><p>${scheduleText('Prossimo avvio')}: <span id="schedule-next" data-no-i18n>${s.config.active?scheduleDate(s.next,s.config.timezone):'—'}</span></p><p class="note">La pausa o l’eliminazione della pianificazione non interrompe l’esecuzione corrente.</p><div id="schedule-live">${scheduleLive(s,current,history)}</div><p id="schedule-detail-error" role="alert" class="error"></p></div>`,`<button type="button" id="schedule-delete" class="danger">${icon('trash-2')}Elimina pianificazione</button><div class="spacer"></div><button type="button" id="schedule-edit">${icon('pencil')}Modifica</button><button type="button" data-action="close-dialog">Chiudi</button>`,true);
 $('[data-schedule-detail]').scheduleSnapshot=JSON.stringify(row);
 $('#schedule-active').onchange=async e=>{const control=e.currentTarget;control.disabled=true;try{await api('/schedules/'+id,{...s.config,active:control.checked},'PUT');await scheduleDetails(id)}catch(error){control.checked=s.config.active;$('#schedule-detail-error').textContent=error.message;control.disabled=false}};
 $('#schedule-edit').onclick=()=>scheduleDialog(s).catch(e=>toast(e.message));
 $('#schedule-delete').onclick=async()=>{if(!await appConfirm({title:'Elimina pianificazione',message:'I prossimi avvii saranno eliminati. Le esecuzioni già generate saranno conservate e quella corrente potrà proseguire.',accept:'Elimina',danger:true}))return;try{await api('/schedules/'+id,{},'DELETE');await refreshSchedules();closeModal();renderHistory();await showExecutions()}catch(e){toast(e.message)}};
}
function scheduleDateFields(prefix,value,label){
 const [date,time]=value.split('T'),[y,m,d]=date.split('-').map(Number),[h,min]=time.split(':').map(Number);const year=new Date().getFullYear();
 const range=(a,b)=>Array.from({length:b-a+1},(_,i)=>[String(a+i),String(a+i).padStart(2,'0')]);
 return `<fieldset class="schedule-date"><legend>${scheduleText(label)}</legend><div class="form-grid">${select('Anno',prefix+'_year',String(y),range(Math.min(year,y),Math.max(year+10,y)))}${select('Mese',prefix+'_month',String(m),range(1,12))}${select('Giorno',prefix+'_day',String(d),range(1,31))}${select('Ora',prefix+'_hour',String(h),range(0,23))}${select('Minuti',prefix+'_minute',String(min),range(0,59))}</div></fieldset>`;
}
function scheduleReadDate(form,prefix){const get=n=>form.querySelector(`[name="${prefix}_${n}"]`).value.padStart(2,'0');return `${get('year')}-${get('month')}-${get('day')}T${get('hour')}:${get('minute')}`;}
async function scheduleDialog(existing=null){
 const owner=app.user?.id;
 const workflow=existing?app.workflows.find(w=>w.id===existing.config.workflow_id):await executionWorkflow();
 if(!workflow)throw Error(scheduleText('Workflow non disponibile'));
 const runs=await api('/runs');if(owner!==app.user?.id)return;
 const now=new Date(Date.now()+5*60000);now.setSeconds(0,0);const local=new Date(now-now.getTimezoneOffset()*60000).toISOString().slice(0,16);
 const c=existing?.config||{workflow_id:workflow.id,name:workflow.name,kind:'daily',start:local,end:null,timezone:Intl.DateTimeFormat().resolvedOptions().timeZone||'Europe/Rome',every:1,weekdays:[0,1,2,3,4],input:'',execution:workflow.execution,base_run:null,active:true};
 const zones=[...new Set([c.timezone,...(Intl.supportedValuesOf?Intl.supportedValuesOf('timeZone'):['Europe/Rome','UTC']),'UTC'])].sort();
 modal(existing?'Modifica pianificazione':'Nuova pianificazione',`<form id="schedule-form">${input('Nome pianificazione','name',c.name)}${select('Frequenza','kind',c.kind,[['once','Esecuzione singola'],['minutes','Ogni N minuti'],['hours','Ogni N ore'],['daily','Giornaliera'],['weekly','Settimanale']])}<div id="schedule-interval">${input('Intervallo','every',c.every)}</div>${scheduleDateFields('start',c.start,'Inizio')}<fieldset id="schedule-weekdays"><legend>Giorni della settimana</legend><div class="schedule-days">${['Lunedì','Martedì','Mercoledì','Giovedì','Venerdì','Sabato','Domenica'].map((label,i)=>`<label><jui-switch data-schedule-day="${i}" aria-label="${label}" ${c.weekdays.includes(i)?'checked':''}></jui-switch>${scheduleText(label)}</label>`).join('')}</div></fieldset><div id="schedule-end-settings"><label class="schedule-active"><jui-switch id="schedule-has-end" aria-label="Data di fine" ${c.end?'checked':''}></jui-switch>Data di fine</label><div id="schedule-end">${scheduleDateFields('end',c.end||c.start,'Fine')}</div></div>${select('Fuso orario','timezone',c.timezone,zones)}<details><summary>Opzioni avanzate</summary><label>Richiesta e contesto aggiuntivo<textarea name="input" rows="3">${esc(c.input)}</textarea></label>${policyFields(c.execution||workflow.execution)}${select('Base del repository','base_run',c.base_run||'',[['','Source configurato nel workflow'],...runs.filter(r=>r.status==='completed').map(r=>[r.id,r.name+' · '+r.id.slice(0,8)])])}</details><p class="note">Le modifiche salvate al workflow si applicano alle prossime esecuzioni.</p><p class="note">Il servizio locale deve essere in funzione. Gli avvii persi non vengono recuperati.</p><section aria-live="polite"><h3>Prossimi avvii</h3><p id="schedule-rule" data-no-i18n></p><ol id="schedule-preview" data-no-i18n></ol></section><p id="schedule-error" class="error" role="alert"></p></form>`,`<button type="button" data-action="close-dialog">Annulla</button><button type="button" id="schedule-save" class="primary">Salva pianificazione</button>`,true);
 const form=$('#schedule-form'),button=$('#schedule-save');let timer,request=0;
 $('[name=name]',form).required=true;$('[name=name]',form).maxLength=160;const interval=$('[name=every]',form);interval.inputMode='numeric';interval.pattern='[0-9]+';interval.required=true;interval.maxLength=5;
 const read=()=>({...c,locale:nodoLocale().startsWith('it')?'it':'en',name:form.elements.name.value.trim(),kind:form.elements.kind.value,every:['minutes','hours'].includes(form.elements.kind.value)?Number(form.elements.every.value):1,start:scheduleReadDate(form,'start'),end:$('#schedule-has-end').checked&&form.elements.kind.value!=='once'?scheduleReadDate(form,'end'):null,timezone:form.elements.timezone.value,weekdays:$$('[data-schedule-day]',form).filter(s=>s.checked).map(s=>Number(s.dataset.scheduleDay)),input:form.elements.input.value,execution:readPolicy(form),base_run:form.elements.base_run.value||null,allow_large_source_copy:false});
 const preview=async()=>{const seq=++request;button.disabled=true;const config=read();$('#schedule-interval').hidden=!['minutes','hours'].includes(config.kind);interval.disabled=$('#schedule-interval').hidden;$('#schedule-weekdays').hidden=config.kind!=='weekly';$('#schedule-end-settings').hidden=config.kind==='once';$('#schedule-end').hidden=!$('#schedule-has-end').checked;$('#schedule-rule').textContent=scheduleRule(config);try{const times=await api('/schedules/preview',config);if(!form.isConnected||seq!==request)return;$('#schedule-preview').innerHTML=times.map(at=>`<li>${esc(scheduleDate(at,config.timezone))}</li>`).join('');$('#schedule-error').textContent=times.length?'':scheduleText('Nessuna occorrenza futura nel periodo selezionato');button.disabled=!times.length;}catch(e){if(form.isConnected&&seq===request){$('#schedule-preview').innerHTML='';$('#schedule-error').textContent=e.message;}}};
 form.addEventListener('input',()=>{clearTimeout(timer);request++;button.disabled=true;timer=setTimeout(preview,250)});form.addEventListener('change',e=>{if(e.target.matches('jui-select,jui-switch')){clearTimeout(timer);preview()}});form.onsubmit=e=>e.preventDefault();
 button.onclick=async()=>{if(!form.reportValidity())return;button.disabled=true;try{const result=await api('/schedules'+(existing?'/'+existing.id:''),read(),existing?'PUT':'POST');await refreshSchedules();renderHistory();await scheduleDetails(result.id)}catch(e){if(form.isConnected){$('#schedule-error').textContent=e.message;button.disabled=false}}};await preview();
}
function scheduleRunMenu(){
 if(app.editBase){previewRevision();return;}
 closeAccountMenu();let menu=$('#schedule-run-menu');const trigger=$('#execute');
 if(!menu){menu=document.createElement('div');menu.id='schedule-run-menu';menu.className='account-menu';menu.setAttribute('popover','manual');menu.setAttribute('role','menu');menu.setAttribute('aria-labelledby','execute');menu.innerHTML=`<button type="button" role="menuitem" data-run-now>${icon('play')}Run adesso</button><button type="button" role="menuitem" data-run-scheduled>${icon('history')}Temporizzata</button>`;document.body.append(menu);
 const close=(focus=false)=>{if(menu.matches(':popover-open'))menu.hidePopover();trigger.setAttribute('aria-expanded','false');if(focus)trigger.focus()};
 $('[data-run-now]',menu).onclick=()=>{close();runNowDialog()};$('[data-run-scheduled]',menu).onclick=()=>{close();scheduleDialog().catch(e=>toast(e.message))};
 document.addEventListener('pointerdown',e=>{if(!e.composedPath().includes(menu)&&!e.composedPath().includes(trigger))close()},true);window.addEventListener('resize',()=>close());
 menu.onkeydown=e=>{const items=$$('[role=menuitem]',menu),i=items.indexOf(document.activeElement);if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close(true)}else if(e.key==='Tab')close();else if(['ArrowDown','ArrowUp','Home','End'].includes(e.key)){e.preventDefault();items[e.key==='Home'?0:e.key==='End'?items.length-1:(i+(e.key==='ArrowDown'?1:-1)+items.length)%items.length].focus()}};
 }
 if(menu.matches(':popover-open')){menu.hidePopover();trigger.setAttribute('aria-expanded','false');return;}hideTooltip();menu.showPopover();const rect=trigger.getBoundingClientRect();menu.style.left=Math.max(8,Math.min(rect.right-menu.offsetWidth,innerWidth-menu.offsetWidth-8))+'px';menu.style.top=(rect.bottom+8)+'px';trigger.setAttribute('aria-expanded','true');$('[role=menuitem]',menu).focus();
}
$('#execute').setAttribute('aria-haspopup','menu');$('#execute').setAttribute('aria-controls','schedule-run-menu');$('#execute').setAttribute('aria-expanded','false');$('#execute').onclick=scheduleRunMenu;
document.addEventListener('click',async e=>{try{const open=e.target.closest('[data-schedule-open]'),run=e.target.closest('[data-schedule-run]');if(open)await scheduleDetails(open.dataset.scheduleOpen);if(run){await loadRun(run.dataset.scheduleRun);closeModal();}}catch(error){toast(error.message)}});
// Refresh summaries and live details without replacing controls or an edited form.
setInterval(async()=>{
 if(!app.user||$('#app').hidden)return;
 try{
  const detail=$('#dialog[open] [data-schedule-detail]');
  if(app.view!=='history'&&!$('#dialog[open] #executions-rows')&&!detail)return;
  await refreshSchedules();if(app.view==='history')renderHistory();
  const list=$('#dialog[open] #executions-schedules');if(list)list.innerHTML=scheduleRows();
  if(detail?.isConnected&&$('#dialog').open){
   const row=app.schedules.find(r=>r.schedule.id===detail.dataset.scheduleDetail);
   if(!row||detail.scheduleSnapshot===JSON.stringify(row))return;
   const history=await api('/schedules/'+row.schedule.id+'/history');
   if(!detail.isConnected||!$('#dialog').open)return;
   const s=row.schedule,control=$('#schedule-active');if(control.disabled&&s.next)return;
   control.checked=s.config.active&&!!s.next;control.disabled=!s.next;control.nextElementSibling.textContent=scheduleText(scheduleState(s));
   $('#schedule-next').textContent=s.config.active?scheduleDate(s.next,s.config.timezone):'—';
   const live=$('#schedule-live'),focused=document.activeElement?.closest('[data-schedule-run]')?.dataset.scheduleRun;
   live.innerHTML=scheduleLive(s,row.current,history);if(focused)$$('[data-schedule-run]',live).find(b=>b.dataset.scheduleRun===focused)?.focus({preventScroll:true});
   detail.scheduleSnapshot=JSON.stringify(row);
  }
 }catch{}
},5000);
