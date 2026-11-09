// Use a disposable server with scripts/fixtures/manager on PATH (test only).
// This exercises real HTTP/UI/native-adapter behavior, without provider inference.
const assert=require('node:assert/strict');
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
  await page.goto(BASE);await page.locator('#app').waitFor({state:'visible'});
  await page.evaluate(()=>{if(document.querySelector('#dialog').open)closeModal()});
  await page.locator('#workflow-select').click();await page.locator('#workflow-topic-menu [data-workflow-choice]').filter({hasText:w.name}).click();
  const initialHistory=page.waitForResponse(r=>r.url().includes('/api/manager/chat?'));
  await page.locator('#manager-toggle').click();
  await initialHistory;
  await page.route('**/api/manager/chat',async route=>{
   const response=await route.fetch();
   // An intervening draft edit makes the Manager offer manual application.
   await page.evaluate(()=>{app.wf.description='Edited while waiting for the Manager';dirty()});
   await route.fulfill({response});
  },{times:1});
  const reply=await send('aggiungi un controllo finale');
  assert(reply.proposal_id,JSON.stringify(reply.messages));
  await page.locator('#manager-apply').click();
  const applied=page.waitForResponse(r=>r.url().endsWith('/api/manager/chat/proposal/resolve'));
  await page.locator('.app-confirm [data-confirm-accept]').click();
  assert((await applied).ok());
  assert.equal(await page.locator('.manager-proposal').count(),0);
  assert.equal(await page.evaluate(()=>app.wf.nodes.length),3);
  assert(await page.locator('#manager-undo').isVisible());
  const persisted=(await call('/workflows')).find(v=>v.id===w.id);assert.equal(persisted.nodes.length,2,'Manager must edit the draft, not silently save');
  await page.locator('#manager-undo').click();assert.equal(await page.evaluate(()=>app.wf.nodes.length),2);
  assert.equal(await page.locator('.manager-proposal,#manager-undo').count(),0);
  assert.equal((await call('/manager/chat?workflow_id='+w.id)).proposal,null);
  await page.locator('#manager-toggle').click();
  const reopened=page.waitForResponse(r=>r.url().includes('/api/manager/chat?'));
  await page.locator('#manager-toggle').click();
  await reopened;
  assert.equal(await page.locator('.manager-proposal,#manager-undo').count(),0);
  await page.route('**/api/manager/chat',async route=>{
   const response=await route.fetch();
   await page.evaluate(()=>{app.wf.description='Keep this draft edit';dirty()});
   await route.fulfill({response});
  },{times:1});
  const next=await send('aggiungi un altro controllo');
  assert(next.proposal_id&&next.proposal_id!==reply.proposal_id);
  const draft=await page.evaluate(()=>JSON.stringify({...app.wf,created:0}));
  const dismissed=page.waitForResponse(r=>r.url().endsWith('/api/manager/chat/proposal/resolve'));
  await page.locator('#manager-dismiss-proposal').click();
  assert((await dismissed).ok());
  assert.equal(await page.locator('.manager-proposal').count(),0);
  assert.equal(await page.evaluate(()=>JSON.stringify({...app.wf,created:0})),draft);
  assert.equal((await call('/manager/chat?workflow_id='+w.id)).proposal,null);
  assert.deepEqual(errors,[]);
  console.log('Manager proposals stay closed after Apply, Undo and Dismiss.');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
