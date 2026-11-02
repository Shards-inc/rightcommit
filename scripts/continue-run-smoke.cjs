// Disposable local execution; reads example files and makes no AI/provider calls.
const fs=require('fs'),os=require('os'),path=require('path'),assert=require('node:assert/strict'),{spawn}=require('child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'nodo-continue-')),base='http://127.0.0.1:18095';
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18095',NODO_SOURCES:path.resolve('sources')}});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1050}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  const call=async(url,body,status=200)=>{const r=await page.request.fetch(base+'/api'+url,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:body});assert.equal(r.status(),status,await r.text());return r.json()};
  await call('/auth/register',{email:'continue@test.local',password:'temporary-continue-password'});
  const w={id:'continue-test',name:'Continue test',description:'',entry:'first',nodes:[{id:'source',kind:'Source',title:'Source',source_type:'example',x:80,y:100},...['first','second','third'].map((id,i)=>({id,kind:'Connection',title:id,provider:'local',operation:'read_ticket',location:'README.md',x:300+i*200,y:100}))],edges:[{from:'source',to:'first',kind:'context'},{from:'first',to:'second',kind:'control'},{from:'second',to:'third',kind:'control'}]};
  await call('/workflows',w);const run=await call('/runs',{workflow_id:w.id});
  const finished=async()=>{for(let i=0;i<150;i++){const d=await call('/runs/'+run.id);assert.notEqual(d.run.status,'failed',d.run.error);if(d.run.status==='completed'&&d.continue_nodes.includes('second'))return d;await page.waitForTimeout(100)}throw Error('Run did not complete')};
  const initial=await finished();assert.equal(initial.run.attempts.first,1);assert(initial.continue_nodes.includes('first'));assert(initial.continue_nodes.includes('source'));
  await call('/runs/'+run.id+'/continue',{node:'missing',acknowledge:true},400);
  await call('/runs/'+run.id+'/continue',{node:'second',acknowledge:false},400);
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(id=>loadRun(id),run.id);await page.locator('#fit').click();
  await page.locator('.node-main[data-id=source]').click({button:'right'});assert(await page.locator('[data-action=continue-run-node]').isEnabled());await page.keyboard.press('Escape');
  await page.locator('.node-main[data-id=second]').click({button:'right'});assert(await page.locator('[data-action=continue-run-node]').isEnabled());await page.locator('[data-action=continue-run-node]').click();
  assert(await page.locator('.app-confirm').isVisible());await page.locator('.app-confirm [data-confirm-accept]').click();
  await page.waitForFunction(()=>app.run?.attempts.second===2&&app.run?.status==='completed');
  const after=await finished();assert.equal(after.run.id,initial.run.id);assert.equal(after.run.workspace,initial.run.workspace);assert.equal(after.run.attempts.first,1);assert.equal(after.run.attempts.second,2);assert.equal(after.run.attempts.third,2);assert.deepEqual(errors,[]);
  assert((await call('/runs/'+run.id+'/events')).some(e=>e.kind==='continue'));
  await call('/runs/'+run.id+'/continue',{node:'source',acknowledge:true});const restarted=await finished();assert.equal(restarted.run.attempts.first,2);assert.equal(restarted.run.attempts.second,3);assert.equal(restarted.run.attempts.third,3);
  console.log('PASS: first-node restart without predecessor, intermediate-node continuation, API validation, confirmation, same run/workspace, attempt counts and audit event');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(dir,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
