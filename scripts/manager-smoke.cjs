// Use a disposable server with scripts/fixtures/manager on PATH (test only).
// This exercises real HTTP/UI/native-adapter behavior, without provider inference.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const BASE=process.env.NODO_TEST_URL||'http://127.0.0.1:18082';
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 const page=await browser.newPage({viewport:{width:1540,height:1050}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 const call=async(path,body,expected=200)=>{const response=await page.request.fetch(BASE+'/api'+path,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:body});const raw=await response.text();assert.equal(response.status(),expected,raw);return JSON.parse(raw)};
 const send=async text=>{await page.locator('#manager-input').fill(text);const response=page.waitForResponse(r=>r.url()===BASE+'/api/manager/chat'&&r.request().method()==='POST');await page.locator('#manager-form [type=submit]').click();const r=await response;assert.equal(r.status(),200,await r.text());await page.waitForFunction(()=>!document.querySelector('#manager-form [type=submit]').disabled);return r.json()};
 try{
  const auth=await call('/auth/status');await call(auth.setup_required?'/auth/register':'/auth/login',{email:'manager-qa@nodo.local',password:'temporary-manager-qa-password'});
  const w={id:'manager-smoke-'+Date.now(),name:'Manager smoke',description:'No provider calls',entry:'read',nodes:[{id:'source',kind:'Source',title:'Source',source_type:'example',x:80,y:100},{id:'read',kind:'Connection',title:'Read actual file',provider:'local',operation:'read_ticket',location:'README.md',x:320,y:100}],edges:[{from:'source',to:'read',kind:'context'}]};await call('/workflows',w);
  await page.goto(BASE);await page.locator('#app').waitFor({state:'visible'});await page.locator('#workflow-select').selectOption(w.id);
  await page.locator('#icon-guide').click();await page.locator('.icon-guide-row').first().waitFor({state:'visible'});
  const ids=[...fs.readFileSync('web/icons.svg','utf8').matchAll(/<symbol id="([^"]+)/g)].map(m=>m[1]);
  assert.deepEqual(await page.locator('.icon-guide-row[data-guide-id] svg use').evaluateAll(ns=>[...new Set(ns.map(n=>n.getAttribute('href').split('#')[1]))].sort()),ids.sort());
  for(const term of ['Atlassian','A2A','Comment']){await page.locator('#icon-guide-search').fill(term);assert(await page.locator('.icon-guide-row:visible').count()>0)}
  await page.locator('[data-help-tab=quick]').click();assert.equal(await page.locator('.quick-start-step:visible').count(),12);await page.locator('[data-help-tab=icons]').click();
  await page.locator('#icon-guide-search').fill('Review');assert(await page.locator('.icon-guide-row:visible').count()>0);assert(await page.locator('.icon-guide-row:visible').count()<ids.length);
  await page.locator('#dialog [data-action=close-dialog]').click();
  await page.locator('.node-main[data-id=read]').click();await page.locator('#inspector').waitFor({state:'visible'});
  await page.locator('#manager-toggle').click();assert.equal(await page.locator('#inspector').isVisible(),false);
  await page.locator('#manager-compact').click();assert.equal(await page.locator('#manager-face').isVisible(),false);assert.equal(await page.locator('#manager-compact use').getAttribute('href'),'/icons.svg#maximize-2');await page.locator('#manager-compact').click();assert(await page.locator('#manager-face').isVisible());assert.equal(await page.locator('#manager-help-toggle,#manager-help-description').count(),0);
  await page.locator('.node-main[data-id=read]').click();assert(await page.locator('#inspector').isVisible());
  const left=await page.locator('#inspector').boundingBox(),right=await page.locator('#manager-panel').boundingBox();assert(left.x+left.width<=right.x+1);assert(Math.abs(left.y-right.y)<2);
  await page.locator('#manager-input').fill('bozza digitata');await page.locator('.node-main[data-id=source]').click();assert.equal(await page.locator('#manager-input').inputValue(),'bozza digitata');
  await send('aggiungi un controllo finale');assert(await page.locator('.manager-markdown h2').count()>0);assert(await page.locator('.manager-markdown strong').count()>0);assert.equal(await page.locator('.manager-markdown script').count(),0);assert.equal(await page.evaluate(()=>app.wf.nodes.length),3);assert.equal(await page.evaluate(()=>app.wf.edges.length),2);assert(await page.locator('#manager-undo').isVisible());
  const persisted=(await call('/workflows')).find(v=>v.id===w.id);assert.equal(persisted.nodes.length,2,'Manager must edit the draft, not silently save');
  await page.locator('#manager-undo').click();assert.equal(await page.evaluate(()=>app.wf.nodes.length),2);
  await send('aggiungi ancora un controllo finale');assert.equal(await page.evaluate(()=>app.wf.nodes.length),3);
  await page.screenshot({path:process.env.NODO_TEST_SCREENSHOT||'/tmp/nodo-manager-desktop.png'});
  await page.locator('#save').click();await page.waitForFunction(()=>!app.dirty);
  await page.reload();await page.locator('#app').waitFor({state:'visible'});await page.locator('#workflow-select').selectOption(w.id);await page.locator('#manager-toggle').click();
  await page.waitForFunction(()=>document.querySelectorAll('.manager-message').length===4);
  const invalid=await send('proposta invalida');assert.equal(invalid.workflow,null);assert.match(invalid.messages.at(-1).text,/Non ho applicato/);assert.equal(await page.evaluate(()=>app.wf.nodes.length),3);
  const otherEmail='manager-other-'+Date.now()+'@nodo.local';await call('/users',{email:otherEmail,password:'temporary-manager-other-password'});
  const other=await browser.newContext();const login=await other.request.post(BASE+'/api/auth/login',{headers:{'X-Nodo-Request':'1'},data:{email:otherEmail,password:'temporary-manager-other-password'}});assert.equal(login.status(),200);
  const history=await other.request.get(BASE+'/api/manager/chat?workflow_id='+w.id);assert.equal((await history.json()).messages.length,0);await other.close();
  const run=await call('/runs',{workflow_id:w.id});let state;
  for(let i=0;i<100;i++){state=await call('/runs/'+run.id);if(['completed','failed'].includes(state.run.status))break;await page.waitForTimeout(100)}assert.equal(state.run.status,'completed',state.run.error);
  await page.evaluate(id=>loadRun(id),run.id);await page.waitForFunction(()=>app.run?.status==='completed');
  const statusReply=await send('spiega lo stato della run');assert.match(statusReply.messages.at(-1).text,/completed/);
  const before=JSON.stringify(state.run.workflow);await send('aggiungi un altro controllo');assert(await page.locator('#manager-apply').isVisible());assert.equal(await page.evaluate(()=>app.run.id),run.id);
  assert.equal(JSON.stringify((await call('/runs/'+run.id)).run.workflow),before);
  await page.locator('#manager-apply').click();await page.waitForFunction(()=>!app.run&&app.dirty);assert.notEqual(await page.evaluate(()=>app.wf.id),w.id);
  const priorDraftId=await page.evaluate(()=>app.wf.id);
  const created=await send('crea un nuovo workflow');assert.equal(created.kind,'create');await page.waitForFunction(id=>app.wf.id!==id,priorDraftId);
  assert(await page.locator('#manager-restore').isVisible());await page.locator('#manager-restore').click();await page.waitForFunction(id=>app.wf.id===id,priorDraftId);
  const resetGraph=await page.evaluate(()=>JSON.stringify(app.wf));await page.locator('#manager-reset').click();await page.waitForFunction(()=>document.querySelectorAll('.manager-message').length===0);assert.equal(await page.evaluate(()=>JSON.stringify(app.wf)),resetGraph);
  const all=await send('crea un workflow con tutti i nodi');assert.equal(all.workflow.nodes.length,11);await page.waitForFunction(()=>app.wf.nodes.length===11);
  assert.deepEqual((await page.evaluate(()=>app.wf.nodes.map(n=>n.kind))).sort(),['Source','Agent','Task','Review','Gate','Memory','Connection','Workflow','MCP','RAG','Comment'].sort());
  assert(await page.locator('.manager-warning').count()>0);await page.locator('#manager-input').fill('bozza');
  await page.locator('#manager-toggle').click();await page.locator('#manager-toggle').click();await page.locator('.node-main').first().click();
  await page.setViewportSize({width:390,height:900});await page.waitForTimeout(100);assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
  await page.locator('#manager-input').scrollIntoViewIfNeeded();assert(await page.evaluate(()=>document.querySelector('#right-panels').scrollLeft>0));
  await page.screenshot({path:'/tmp/nodo-manager-mobile.png'});
  await page.locator('#account').click();await page.locator('[data-action=logout]').click();
  await page.locator('#auth-form [name=email]').fill(otherEmail);await page.locator('#auth-form [name=password]').fill('temporary-manager-other-password');await page.locator('#auth-submit').click();
  await page.locator('#app').waitFor({state:'visible'});await page.waitForTimeout(100);
  assert.equal(await page.locator('.manager-message').count(),0);assert.equal(await page.locator('#manager-restore').count(),0);
  assert.deepEqual(errors,[]);
  console.log(JSON.stringify({pass:true,provider_inference:false,checks:['complete SVG icon guide and search','manager opening closes inspector','horizontal panel stacking','typing survives node selection','native manager resume and persistent history','validated node and edge edits with undo','drafts remain unsaved until Save','invalid graph refused','cross-user history isolation and logout cache clearing','actual run evidence and frozen run graph','new draft from run','automatic new workflow with conversation continuity and draft recovery','safe Markdown bubbles','chat reset without graph changes','all 11 node types in editor','mobile horizontal panel layout']}));
 }finally{await browser.close()}
})().catch(error=>{console.error(error);process.exit(1)});
