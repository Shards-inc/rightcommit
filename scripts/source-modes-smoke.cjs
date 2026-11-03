// Run against a disposable instance with NODO_SOURCES fixtures material/ and oversized/.
const assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const BASE=process.env.NODO_TEST_URL||'http://127.0.0.1:18081';
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 const page=await browser.newPage({viewport:{width:1440,height:1000}});
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 const request=async(path,body,expected=200,method)=>{const response=await page.request.fetch(BASE+'/api'+path,{method:method||(body?'POST':'GET'),headers:{'X-Nodo-Request':'1'},data:body});const data=await response.json();assert.equal(response.status(),expected,JSON.stringify(data));return data};
 try {
  const status=await request('/auth/status');await request(status.setup_required?'/auth/register':'/auth/login',{email:process.env.NODO_TEST_EMAIL||'source-modes-qa@nodo.local',password:'temporary-qa-password-123'});
  await page.goto(BASE);await page.locator('#app').waitFor({state:'visible'});
  const initial=await request('/settings');assert.equal(initial.source_copy_limit_mib,10);
  await page.locator('#settings').click();
  const limit=page.locator('#settings-form [name=source_copy_limit_mib]');
  assert.equal(await limit.inputValue(),'10');
  await limit.fill('13');await page.locator('[data-action=save-settings]').click();
  await page.locator('#settings-form').waitFor({state:'hidden'});
  const larger=await request('/storage/source-size?path=oversized');assert.equal(larger.threshold_bytes,13*1048576);assert.equal(larger.copy_error,null);
  await page.locator('#settings').click();
  await page.locator('#settings-form [name=source_copy_limit_mib]').fill('7');
  await page.locator('[data-action=save-settings]').click();await page.locator('#settings-form').waitFor({state:'hidden'});
  await page.reload();await page.locator('#app').waitFor({state:'visible'});
  assert.equal((await request('/settings')).source_copy_limit_mib,7);
  const invalid=await request('/settings');invalid.source_copy_limit_mib=0;await request('/settings',invalid,400,'PUT');
  assert.equal((await request('/settings')).source_copy_limit_mib,7);
  await page.locator('[data-action=add-node][data-kind=Source]').click();
  await page.locator('#node-form [name=source_type]').selectOption('folder');
  assert.equal(await page.locator('#node-form [name=source_mode]').inputValue(),'direct');
  await page.locator('#node-form [name=location]').fill('material');
  await page.locator('#node-form [name=source_mode]').selectOption('copy');
  await page.locator('.app-confirm').waitFor({state:'visible'});
  assert.match(await page.locator('.app-confirm').innerText(),/7 MiB/);
  await page.locator('.app-confirm [data-confirm-cancel]').first().click();
  await page.waitForFunction(()=>document.querySelector('#node-form [name=source_mode]').value==='direct');
  await page.locator('#node-form [name=source_mode]').selectOption('copy');
  await page.locator('.app-confirm [data-confirm-accept]').click();
  assert.equal(await page.locator('#node-form [name=source_mode]').inputValue(),'copy');
  await page.screenshot({path:process.env.NODO_TEST_SCREENSHOT||'/tmp/nodo-source-modes.png'});
  await page.locator('#node-form [name=source_type]').selectOption('git');
  assert.equal(await page.locator('#source-mode-fields').isVisible(),false);
  await page.locator('#node-form [name=source_type]').selectOption('folder');
  await page.locator('#node-form [name=source_mode]').selectOption('direct');
  await page.locator('#node-form [name=location]').fill('oversized');
  await page.locator('#node-form [name=source_mode]').selectOption('copy');
  await page.waitForFunction(()=>document.querySelector('#node-form [name=source_mode]').value==='direct');
  assert.match(await page.locator('#toast').innerText(),/large.bin.*7 MiB/);
  const size=await request('/storage/source-size?path=material');assert(size.bytes>7*1048576);assert.equal(size.copy_error,null);
  await request('/storage/source-size?path=material&root=examples',null,400);
  const workflow=mode=>({id:'qa-source-'+mode+'-'+Date.now(),name:'Source modes QA '+mode,description:'Disposable source-mode test',entry:'test',nodes:[{id:'source',kind:'Source',x:80,y:100,title:'Material',source_type:'folder',source_mode:mode,location:mode==='direct'?'oversized':'material'},{id:'test',kind:'Connection',x:350,y:100,title:'Real local tests',provider:'local',operation:'test'}],edges:[{from:'source',to:'test',kind:'context'}]});
  let copyWorkflowId;const completed=[];for(const mode of ['copy','direct']){
   const w=workflow(mode);await request('/workflows',w);completed.push(w.id);if(mode==='copy')copyWorkflowId=w.id;
   if(mode==='copy')await request('/runs',{workflow_id:w.id},400);
   const run=await request('/runs',{workflow_id:w.id,allow_large_source_copy:mode==='copy'});
   assert.equal(run.workspace.includes('/sources/oversized'),mode==='direct');
   let state;for(let attempt=0;attempt<120;attempt++){state=await request('/runs/'+run.id);if(['completed','failed'].includes(state.run.status))break;await page.waitForTimeout(250)}
   assert.equal(state.run.status,'completed',state.run.error);
   assert.match(state.run.outputs.test.summary,/test result: ok/);
   const listing=await request('/storage?root=workspaces&path='+run.id);assert(listing.entries.some(e=>e.name==='Cargo.toml'));
  }
  await page.reload();await page.locator('#app').waitFor({state:'visible'});
  await page.locator('#workflow-select').selectOption(copyWorkflowId);
  await page.locator('.node-main[data-id=source]').click();
  assert.equal(await page.locator('#node-form [name=source_mode]').inputValue(),'copy');
  await page.locator('#execute').click();
  await page.locator('[data-run-now]').click();
  await page.locator('[data-action=start-run]').click();
  await page.locator('.app-confirm').waitFor({state:'visible'});
  await page.locator('.app-confirm [data-confirm-cancel]').first().click();
  await page.waitForFunction(()=>!document.querySelector('[data-action=start-run]').disabled);
  await page.locator('[data-action=start-run]').click();
  await page.locator('.app-confirm [data-confirm-accept]').click();
  await page.locator('#close-run').waitFor({state:'visible'});
  await page.waitForFunction(()=>app.run?.status==='completed',{},{timeout:30000});
  for(const id of completed){
   const summary=await request('/workflows/'+id+'/deletion');
   await request('/workflows/'+id,{acknowledge:true,run_ids:summary.run_ids},200,'DELETE');
  }
  for(const path of ['material','oversized']){
   const original=await request('/storage?root=sources&path='+path);
   assert(original.entries.some(e=>e.name==='Cargo.toml'));
   assert(original.entries.some(e=>e.name===(path==='material'?'a.bin':'large.bin')));
  }
  const restored=await request('/settings');restored.source_copy_limit_mib=10;await request('/settings',restored,200,'PUT');
  assert.deepEqual(errors,[]);
  console.log(JSON.stringify({pass:true,checks:['configurable threshold persisted and applied','invalid threshold refused','deleting copy and direct runs preserves originals','new default direct','copy warning cancel/accept','remote mode hidden','oversized file refused explicitly','size endpoint scoped to sources','server requires large-copy acknowledgement','real CI runs for copy and direct','workspace browsing for both modes','saved copy mode preserved','start dialog cancellation and confirmed copy run']}));
 } finally {await browser.close()}
})().catch(error=>{console.error(error);process.exit(1)});
