// Isolated UI regression: configure, persist, execute and restore a bypassed Task.
const fs=require('fs'),os=require('os'),path=require('path'),assert=require('node:assert/strict'),{spawn}=require('child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'rightcommit-skip-')),base='http://127.0.0.1:18115';
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18115'}});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  const call=async(url,body)=>{const r=await page.request.fetch(base+'/api'+url,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:body});assert.equal(r.status(),200,await r.text());return r.json()};
  await call('/auth/register',{email:'skip@test.local',password:'temporary-skip-password'});
  const workflow={id:'skip-ui',name:'Skip UI',description:'',entry:'task',nodes:[{id:'source',kind:'Source',title:'Source',source_type:'example',x:80,y:100},{id:'agent',kind:'Agent',title:'Agent',runtime:'claude',x:240,y:100},{id:'task',kind:'Task',title:'Optional task',agent:'agent',command:'missing-command-must-not-run',prompt:'',x:400,y:100},{id:'review',kind:'Review',title:'Review',mode:'human',x:600,y:100}],edges:[{from:'source',to:'agent',kind:'context'},{from:'agent',to:'task',kind:'context'},{from:'task',to:'review',kind:'control'}]};
  await call('/workflows',workflow);await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>closeModal());await page.evaluate(id=>selectWorkflow(id),workflow.id);
  await page.locator('.node-main[data-id=task]').click();const toggle=page.locator('#node-form jui-switch[name=skip]').getByRole('switch');
  assert.equal(await toggle.getAttribute('aria-checked'),'false');await toggle.click();
  await page.waitForFunction(()=>app.wf.nodes.find(n=>n.id==='task').skip===true);
  assert(await page.locator('.node[data-id=task]').evaluate(n=>n.classList.contains('task-bypassed')));
  assert.equal(await page.locator('.node[data-id=task] .node-main').evaluate(n=>getComputedStyle(n).borderTopStyle),'dashed');
  assert.equal(await page.locator('.node[data-id=task] .node-badge use').getAttribute('href'),'/icons.svg#fast-forward');
  await page.evaluate(()=>saveWorkflow());const saved=(await call('/workflows')).find(w=>w.id===workflow.id);assert.equal(saved.nodes.find(n=>n.id==='task').skip,true);
  await page.reload();await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>closeModal());await page.evaluate(id=>selectWorkflow(id),workflow.id);assert.equal(await page.locator('.node[data-id=task] .task-skip-label').textContent(),'Skipped');
  const run=await call('/runs',{workflow_id:workflow.id});let result;
  for(let i=0;i<100;i++){result=await call('/runs/'+run.id);if(result.run.status==='waiting_review')break;assert.notEqual(result.run.status,'failed',result.run.error);await page.waitForTimeout(100)}
  assert.equal(result.run.status,'waiting_review');assert.equal(result.run.states.task,'bypassed');assert.equal(result.run.outputs.task,undefined);assert.equal(result.run.attempts.task,undefined);assert.deepEqual(result.run.sessions,{});
  await page.evaluate(id=>loadRun(id),run.id);await page.locator('.node-main[data-id=task]').click();assert(await page.locator('#node-form jui-switch[name=skip]').getByRole('switch').isDisabled());
  await page.evaluate(()=>{app.view='list';render()});assert((await page.locator('.step-row[data-id=task]').innerText()).includes('Skipped'));
  await page.evaluate(id=>{app.view='graph';selectWorkflow(id)},workflow.id);await page.locator('.node-main[data-id=task]').click();await page.locator('#node-form jui-switch[name=skip]').getByRole('switch').click();await page.evaluate(()=>saveWorkflow());
  assert.equal((await call('/workflows')).find(w=>w.id===workflow.id).nodes.find(n=>n.id==='task').skip,false);assert.equal(await page.locator('.node[data-id=task] .task-skip-label').count(),0);assert.equal((await call('/runs/'+run.id)).run.workflow.nodes.find(n=>n.id==='task').skip,true);
  assert.deepEqual(errors,[]);console.log('PASS: default off, real switch, icon/badge/dashed border, save/reload, no agent calls/outputs, normal continuation, frozen historical configuration and switch off restores task');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(dir,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
