// Runs against a disposable local server; no external services or model calls.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const os=require('node:os');
const path=require('node:path');
const {spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const data=fs.mkdtempSync(path.join(os.tmpdir(),'nodo-catalog-'));
 const base='http://127.0.0.1:18089';
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:data,NODO_LISTEN:'127.0.0.1:18089',NODO_SOURCES:path.resolve('sources')}});
 let output='';server.stderr.on('data',b=>output+=b);server.stdout.on('data',b=>output+=b);
 let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100));if(i===99)throw Error(output)}
  browser=await chromium.launch({channel:'chrome',headless:true});
  const page=await browser.newPage({viewport:{width:1440,height:1080}}),errors=[];
  page.setDefaultTimeout(15000);
  page.on('pageerror',e=>errors.push(e.message));
  const call=async(url,body)=>{const r=await page.request.fetch(base+'/api'+url,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:body});assert.equal(r.status(),200,await r.text());return r.json()};
  const unauthorized=await page.request.get(base+'/api/workflow-templates');assert.equal(unauthorized.status(),401);
  await call('/auth/register',{email:'catalog-qa@nodo.local',password:'temporary-catalog-qa-password'});
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});
  const originals=await call('/workflows');assert.equal(originals.length,2);
  const templates=await call('/workflow-templates');assert.equal(templates.length,11);
  assert.deepEqual(templates.slice(0,2).map(t=>t.id),['as-is-sdd-adversarial','as-is-sdd']);
  await page.locator('#new-workflow').click();await page.locator('.workflow-catalog').waitFor();
  assert.equal(await page.locator('.catalog-section').count(),3);assert.equal(await page.locator('.template-card').count(),11);
  assert.equal(await page.locator('.template-grid').first().evaluate(e=>getComputedStyle(e).gridTemplateColumns.split(' ').length),3);
  for(const lang of ['en','it','en','it']){
   await page.evaluate(lang=>{localStorage.setItem('nodo.language',lang);window.dispatchEvent(new StorageEvent('storage',{key:'nodo.language',newValue:lang}))},lang);
   for(const t of templates){
    const card=page.locator(`[data-template-id="${t.id}"]`);
    const localized=await page.evaluate(t=>[uiLocalizedText(t.workflow.name),uiLocalizedText(t.workflow.description),uiLocalizedText(t.setup)],t);
    await page.waitForFunction(({id,expected})=>document.querySelector(`[data-template-id="${id}"] .template-description`).textContent===expected,{id:t.id,expected:localized[1]},{timeout:5000});
    assert.equal(await card.locator('strong').textContent(),localized[0]);
    assert.equal(await card.locator('.template-setup').textContent(),localized[2]);
    if(lang==='en')assert.deepEqual(localized,[t.workflow.name,t.workflow.description,t.setup]);
    else {assert.notEqual(localized[1],t.workflow.description);assert.notEqual(localized[2],t.setup)}
   }
  }
  assert.equal(await page.locator('.template-card').first().getAttribute('data-template-id'),'as-is-sdd-adversarial');
  await page.screenshot({path:'/tmp/nodo-workflow-catalog-desktop.png'});
  const initialId=await page.evaluate(()=>app.wf.id);
  await page.locator('.template-card').first().click();
  await page.locator('#template-workflow-form').waitFor();
  assert.equal((await call('/workflows')).length,2,'opening confirmation does not create a workflow');
  assert.match(await page.locator('[name=workflow_name]').inputValue(),/^QUADRO_APPLICATIVO_EVIDENZE_E_CATALOGO_FUNZIONALE_/);
  assert.equal(await page.locator('[data-action=template-back]').textContent(),'Indietro');
  await page.locator('[name=workflow_name]').fill('');await page.locator('[data-action=save-template]').click();
  assert(await page.locator('#template-workflow-form').isVisible());assert.equal((await call('/workflows')).length,2);
  await page.locator('[data-action=template-back]').click();await page.locator('.workflow-catalog').waitFor();
  await page.locator('.template-card').first().click();await page.locator('#template-workflow-form').waitFor();
  await page.locator('.modal-actions [data-action=close-dialog]').click();
  assert.equal(await page.evaluate(()=>app.wf.id),initialId);assert.equal((await call('/workflows')).length,2);
  await page.locator('#new-workflow').click();await page.locator('.template-card').first().click();
  await page.keyboard.press('Escape');assert.equal((await call('/workflows')).length,2);
  await page.locator('#new-workflow').click();await page.locator('.workflow-catalog').waitFor();
  await page.locator('[data-action=empty-workflow]').click();assert.equal(await page.locator('[name=workflow_name]').inputValue(),'Nuovo workflow');
  await page.locator('[name=workflow_name]').fill('Workflow vuoto QA');await page.locator('[data-action=create-workflow]').click();await page.waitForFunction(()=>!document.querySelector('#dialog').open);
  assert.equal(await page.evaluate(()=>app.wf.nodes.length),0);assert.equal(await page.evaluate(()=>app.wf.name),'Workflow vuoto QA');
  for(const [index,t] of templates.entries()){
   const lang=index%2?'en':'it';
   await page.evaluate(lang=>{localStorage.setItem('nodo.language',lang);window.dispatchEvent(new StorageEvent('storage',{key:'nodo.language',newValue:lang}))},lang);
   const expectedName=await page.evaluate(t=>templateWorkflowName(uiLocalizedText(t.workflow.name)).replace(/_[0-9]{12}$/,''),t);
   await page.locator('#new-workflow').click();await page.locator('.workflow-catalog').waitFor();
   await page.locator(`[data-template-id="${t.id}"]`).click();await page.locator('#template-workflow-form').waitFor();
   assert.equal(await page.locator('[data-action=template-back]').textContent(),lang==='en'?'Back':'Indietro');
   assert.equal(await page.locator('.modal-actions [data-action=close-dialog]').textContent(),lang==='en'?'Close':'Chiudi');
   assert.equal(await page.locator('[data-action=save-template]').textContent(),lang==='en'?'Save':'Salva');
   const suggested=await page.locator('[name=workflow_name]').inputValue();assert.equal(suggested.replace(/_[0-9]{12}$/,''),expectedName);
   if(index===0){await page.locator('[name=workflow_name]').fill('Application overview QA');await page.locator('[name=workflow_name]').press('Enter')}
   else await page.locator('[data-action=save-template]').click();
   await page.waitForFunction(()=>!document.querySelector('#dialog').open);
   const w=await page.evaluate(()=>app.wf);assert.notEqual(w.id,t.workflow.id);if(index===0)assert.equal(w.name,'Application overview QA');else assert.match(w.name,/_[0-9]{12}$/);assert.equal(w.nodes.length,t.workflow.nodes.length);assert.deepEqual(w.nodes,t.workflow.nodes);assert.equal(w.description,t.locales[lang].description);if(index!==0)assert.equal(w.name.replace(/_[0-9]{12}$/ ,''),expectedName);
   await call('/workflows/validate',w);
  }
  const before=await page.evaluate(()=>app.wf.id);
  await page.evaluate(()=>{app.wf.description='unsaved';dirty()});
  await page.locator('#new-workflow').click();await page.locator('.template-card').first().click();
  await page.locator('[data-action=save-template]').click();
  await page.waitForFunction(()=>!document.querySelector('#dialog').open);
  assert.notEqual(await page.evaluate(()=>app.wf.id),before);
  assert.equal((await call('/workflows')).find(w=>w.id===before).description,'unsaved');
  await page.locator('#new-workflow').click();await page.locator('.workflow-catalog').waitFor();
  await page.locator('.template-card').first().click();await page.locator('[data-action=save-template]').dblclick();await page.waitForFunction(()=>!document.querySelector('#dialog').open);
  assert.equal((await call('/workflows')).length,16,'double click creates one copy');
  assert.equal(new Set((await call('/workflows')).map(w=>w.name)).size,16);
  await page.reload();await page.locator('#app').waitFor({state:'visible'});assert.equal(await page.evaluate(()=>app.workflows.length),16);
  await page.locator('#new-workflow').click();await page.locator('.workflow-catalog').waitFor();
  await page.setViewportSize({width:390,height:844});
  assert.equal(await page.locator('.template-grid').first().evaluate(e=>getComputedStyle(e).gridTemplateColumns.split(' ').length),1);
  assert(await page.locator('.workflow-catalog').evaluate(e=>e.scrollWidth<=e.clientWidth+1));
  await page.screenshot({path:'/tmp/nodo-workflow-catalog-mobile.png'});
  await page.locator('.template-card').first().click();await page.locator('#template-workflow-form').waitFor();
  assert(await page.locator('#dialog').evaluate(e=>e.scrollWidth<=e.clientWidth+1));
  await page.screenshot({path:'/tmp/nodo-template-confirm-mobile.png'});
  await page.keyboard.press('Escape');assert.equal(await page.locator('#dialog').evaluate(e=>e.open),false);
  assert.deepEqual(errors,[]);
  console.log(JSON.stringify({pass:true,templates:11,checks:['authenticated catalog','three sections','maximum three columns','original blank dialog','all templates create independent persisted valid graphs','localized confirmation footer','Back, Close and Escape create nothing','required names','editable suggested names','Enter submits','automatic names','unsaved changes persist','double click protection','reload persistence','mobile single column without overflow','Escape closes dialog']}));
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(resolve=>server.exitCode!==null||server.signalCode!==null?resolve():server.once('exit',resolve));fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
