// Disposable server/browser; no agents or external services.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const data=fs.mkdtempSync(path.join(os.tmpdir(),'nodo-topics-')),base='http://127.0.0.1:18149';
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:data,NODO_LISTEN:'127.0.0.1:18149',NODO_SOURCES:path.resolve('sources')},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<150;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100));}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  const call=async(url,body)=>{const r=await page.request.fetch(base+'/api'+url,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:body});assert.equal(r.status(),200,await r.text());return r.json()};
  assert.equal((await page.request.get(base+'/api/workflow-topics')).status(),401);
  await call('/auth/register',{email:'topics@test.local',password:'temporary-topics-password'});
  const initial=await call('/workflows');for(let i=0;i<7;i++)await call('/workflows',{id:'extra-'+i,name:'Extra '+i,description:'',entry:'',nodes:[],edges:[]});
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>topicRequest);
  const open=async()=>{await page.locator('#workflow-select').click();await page.locator('#workflow-topic-menu').waitFor({state:'visible'})};
  await open();assert.equal(await page.locator('.workflow-topic').count(),1);assert.equal(await page.locator('.workflow-topic [data-workflow-choice]').count(),5);assert.match(await page.locator('.workflow-topic summary').innerText(),/Unassigned/);
  await page.locator('[data-topic-all]').click();await page.locator('#workflow-picker-rows tr').nth(8).waitFor();
  assert.equal(await page.locator('.workflow-list input[type=checkbox],.workflow-list [role=checkbox],.workflow-list select').count(),0);assert(await page.locator('#workflow-topic-settings').isHidden());assert(await page.locator('.workflow-bulk').isHidden());await page.locator('#manage-topics-toggle').click();
  for(let i=1;i<=6;i++){await page.locator('#new-topic-name').fill('Topic '+i);await page.locator('#create-topic-form button').click();await page.waitForFunction(n=>workflowOrganization.topics.length===n,i+1)}
  const state=await call('/workflow-topics'),last=state.topics.at(-1).id;
  const pick=async(id,value)=>{await page.locator(id).evaluate((el,value)=>{el.value=value;el.dispatchEvent(new Event('change',{bubbles:true}))},value)};
  await page.locator('[data-workflow-check="extra-0"] button').click();await pick('#workflow-topic-destination',last);await page.locator('#move-topic-workflows').click();await page.waitForFunction(id=>workflowOrganization.assignments['extra-0']===id,last);
  // Opening a workflow in a topic outside the saved first five promotes it without changing saved order.
  await page.locator(`[data-topic-filter="${last}"]`).click();await page.locator('[data-workflow-choice="extra-0"]').click();await page.evaluate(()=>topicRequest);await open();
  assert.equal(await page.locator('.workflow-topic').count(),5);assert.match(await page.locator('.workflow-topic summary').first().innerText(),/Topic 6/);
  assert.equal(await page.locator('.workflow-topic').first().locator('[data-workflow-choice]').first().getAttribute('data-workflow-choice'),'extra-0');
  assert.deepEqual((await call('/workflow-topics')).topics.map(t=>t.id),state.topics.map(t=>t.id));
  await page.keyboard.press('Escape');assert(await page.locator('#workflow-topic-menu').isHidden());
  await page.reload();await page.locator('#app').waitFor({state:'visible'});assert.equal(await page.evaluate(()=>app.wf.id),'extra-0');await page.evaluate(()=>topicRequest);
  await open();await page.locator('#manage-workflow-list').click();await page.locator('#manage-topics-toggle').click();
  const handle=page.locator(`[data-topic-drag="${last}"]`),previous=state.topics[4].id;
  await handle.scrollIntoViewIfNeeded();const origin=await handle.boundingBox(),destination=await page.locator(`[data-topic-id="${previous}"]`).boundingBox();
  await page.mouse.move(origin.x+origin.width/2,origin.y+origin.height/2);await page.mouse.down();await page.mouse.move(destination.x+40,destination.y+3,{steps:10});await page.mouse.up();await page.waitForFunction(id=>workflowOrganization.topics[4].id===id,last);
  const savedOrder=(await call('/workflow-topics')).topics.map(t=>t.id);assert.equal(savedOrder[4],last);
  // Escape cancels a drag without saving a different order.
  await handle.scrollIntoViewIfNeeded();const dragOrigin=await handle.boundingBox();await page.mouse.move(dragOrigin.x+10,dragOrigin.y+10);await page.mouse.down();await page.mouse.move(dragOrigin.x+35,dragOrigin.y-35,{steps:5});await page.keyboard.press('Escape');await page.mouse.up();assert.deepEqual((await call('/workflow-topics')).topics.map(t=>t.id),savedOrder);
  // Reopen the management dialog and verify persisted order and keyboard fallback.
  await page.locator('.modal-actions [data-action=close-dialog]').click();await open();await page.locator('#manage-workflow-list').click();await page.locator('#manage-topics-toggle').click();assert.equal(await page.locator('.topic-management-row').nth(4).getAttribute('data-topic-id'),last);
  await handle.focus();await page.keyboard.press('ArrowDown');await page.waitForFunction(id=>workflowOrganization.topics[5].id===id,last);await page.keyboard.press('ArrowUp');await page.waitForFunction(id=>workflowOrganization.topics[4].id===id,last);
  await page.locator(`[data-topic-rename="${last}"]`).click();await page.locator('.rename-topic-form input').fill('Renamed topic');await page.locator('.rename-topic-form button[type=submit]').click();await page.waitForFunction(()=>workflowOrganization.topics.some(t=>t.name==='Renamed topic'));
  await page.locator(`[data-topic-remove="${last}"]`).click();await page.locator('[data-confirm-accept]').click();await page.waitForFunction(id=>!workflowOrganization.topics.some(t=>t.id===id),last);assert.equal(await page.evaluate(()=>workflowTopic('extra-0')),'unassigned');
  // Delete from list using the existing destructive-action confirmation.
  await page.locator('[data-workflow-check="extra-1"] button').click();const remove=page.locator('[data-workflow-remove="extra-1"]');assert.equal(await remove.locator('svg').count(),1);assert.equal(await remove.innerText(),'');await remove.click();await page.locator('dialog.app-confirm').waitFor({state:'visible'});assert.match(await page.locator('#confirmation-message').innerText(),/Extra 1/);await page.locator('[data-confirm-cancel]').last().click();assert((await call('/workflows')).some(w=>w.id==='extra-1'));assert.equal(await page.locator('[data-workflow-check="extra-1"]').evaluate(el=>el.checked),true);await remove.click();await page.locator('[data-confirm-accept]').click();await page.waitForFunction(()=>!app.workflows.some(w=>w.id==='extra-1'));assert(await page.locator('#workflow-search').isVisible());
  await page.locator('#workflow-search').fill('Extra 0');assert.equal(await page.locator('#workflow-picker-rows tr').count(),1);
  await page.locator('#manage-topics-toggle').click();await page.locator('#workflow-search').fill('');await page.waitForFunction(()=>[...document.querySelectorAll('[data-workflow-remove] svg')].every(e=>e.getBBox().width>0));await page.screenshot({path:'/tmp/nodo-workflow-topics-desktop.png'});
  await page.setViewportSize({width:390,height:844});assert(await page.locator('#dialog').evaluate(e=>e.scrollWidth<=e.clientWidth+1));await page.screenshot({path:'/tmp/nodo-workflow-topics-mobile.png'});
  await page.locator('.modal-actions [data-action=close-dialog]').click();await open();assert(await page.locator('#workflow-topic-menu').evaluate(e=>{const r=e.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth}));
  await page.keyboard.press('Escape');await page.setViewportSize({width:1440,height:1000});await page.locator('#new-workflow').click();await page.locator('[data-action=empty-workflow]').click();await page.locator('[name=workflow_name]').fill('New unassigned');await page.locator('[data-action=create-workflow]').click();await page.waitForFunction(()=>!document.querySelector('#dialog').open);await page.evaluate(()=>topicRequest);assert.equal(await page.evaluate(()=>workflowOrganization.recent[0]),await page.evaluate(()=>app.wf.id));assert.equal(await page.evaluate(()=>workflowTopic(app.wf.id)),'unassigned');
  assert.deepEqual(errors,[]);console.log('PASS: Unassigned migration, five workflows/topics, creation, move, automatic promotion, saved order, reload, drag reorder, Escape cancellation, keyboard reorder, rename, remove, workflow deletion, search, keyboard and mobile');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(resolve=>server.exitCode!==null||server.signalCode!==null?resolve():server.once('exit',resolve));fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
