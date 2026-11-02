// Disposable local instance; uses read-only example workflows, no model calls.
const fs=require('fs'),os=require('os'),path=require('path'),assert=require('node:assert/strict'),{spawn}=require('child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'nodo-view-')),base='http://127.0.0.1:18096';
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18096',NODO_SOURCES:path.resolve('sources')}});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1050}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  const call=async(url,body)=>{const r=await page.request.fetch(base+'/api'+url,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:body});assert.equal(r.status(),200,await r.text());return r.json()};
  await call('/auth/register',{email:'view@test.local',password:'temporary-view-password'});
  const workflow=id=>({id,name:id,description:'',entry:'read',nodes:[{id:'source',kind:'Source',title:'Source',source_type:'example',x:80,y:100},{id:'read',kind:'Connection',title:'Read',provider:'local',operation:'read_ticket',location:'README.md',x:300,y:100}],edges:[{from:'source',to:'read',kind:'context'}]});
  const a=workflow('workflow-a'),b=workflow('workflow-b');await call('/workflows',a);await call('/workflows',b);
  const run=async w=>{const r=await call('/runs',{workflow_id:w.id});for(let i=0;i<100;i++){const d=await call('/runs/'+r.id);assert.notEqual(d.run.status,'failed',d.run.error);if(d.run.status==='completed')return r;await page.waitForTimeout(100)}throw Error('run timeout')};
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.locator('#workflow-select').selectOption(a.id);
  await page.waitForFunction(()=>document.querySelector('#latest-run').getAttribute('aria-busy')==='false');assert(await page.locator('#latest-run').isDisabled());assert.equal(await page.locator('#view-context').getAttribute('data-mode'),'design');assert.equal(await page.locator('#workspace-mode').innerText(),'Working on Design');
  const first=await run(a);await page.locator('#workflow-select').selectOption(b.id);await page.locator('#workflow-select').selectOption(a.id);await page.waitForFunction(()=>!document.querySelector('#latest-run').disabled);await page.locator('#latest-run').click();await page.waitForFunction(id=>app.run?.id===id,first.id);
  assert.equal(await page.locator('#view-context').getAttribute('data-mode'),'run');assert((await page.locator('#workspace-mode').innerText()).includes(first.id.slice(0,8)));assert((await page.locator('#run-status').innerText()).includes('sola lettura'));assert.equal(await page.locator('#history-tab').getAttribute('aria-selected'),'false');
  const second=await run(a);await page.waitForFunction(()=>!document.querySelector('#latest-run').disabled);await page.locator('#latest-run').click();await page.waitForFunction(id=>app.run?.id===id,second.id);
  await page.screenshot({path:'/tmp/nodo-view-context-desktop.png'});
  await page.locator('#close-run').click();assert.equal(await page.locator('#view-context').getAttribute('data-mode'),'design');await page.waitForFunction(()=>!document.querySelector('#latest-run').disabled);
  await page.locator('.node-main[data-id=read]').click();await page.locator('#node-form [name=title]').fill('Unsaved edit');assert(await page.evaluate(()=>app.dirty));
  await page.locator('#latest-run').click();await page.waitForFunction(id=>app.run?.id===id&&!app.dirty,second.id);assert.equal((await call('/workflows')).find(w=>w.id===a.id).nodes.find(n=>n.id==='read').title,'Unsaved edit');
  await page.locator('#close-run').click();await page.waitForFunction(()=>!document.querySelector('#latest-run').disabled);
  let intercept;const intercepted=new Promise(resolve=>intercept=resolve);await page.route('**/api/runs?workflow_id='+a.id,route=>intercept(route));await page.locator('#latest-run').click();const held=await intercepted;await page.locator('#workflow-select').selectOption(b.id);await held.continue();await page.unroute('**/api/runs?workflow_id='+a.id);await page.waitForFunction(()=>document.querySelector('#latest-run').getAttribute('aria-busy')==='false');assert(await page.evaluate(()=>!app.run&&app.wf.id==='workflow-b'));assert(await page.locator('#latest-run').isDisabled());
  await page.setViewportSize({width:390,height:900});assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));await page.screenshot({path:'/tmp/nodo-view-context-mobile.png'});assert.deepEqual(errors,[]);
  console.log('PASS: explicit Design/Run context, latest workflow run without History, newer run refresh, no-run disabled, unsaved edits protected, stale response ignored, mobile without horizontal overflow');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(dir,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
