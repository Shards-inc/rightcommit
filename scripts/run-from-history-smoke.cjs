// Real browser/API regression for starting a new run while a historical run is visible.
// Uses only local file-reading Connection nodes: no models or external services.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const root=process.cwd(),data=fs.mkdtempSync(path.join(os.tmpdir(),'run-from-history-')),sources=path.join(data,'sources'),base='http://127.0.0.1:18273';
 for(const name of ['first','latest','other']){fs.mkdirSync(path.join(sources,name),{recursive:true});fs.writeFileSync(path.join(sources,name,'evidence.txt'),name+' source evidence');}
 const server=spawn(process.env.NODO_TEST_BINARY||path.join(root,'target/debug/rightcommit'),[],{cwd:data,env:{...process.env,NODO_DATA:data,NODO_SOURCES:sources,NODO_ASSETS:root,NODO_LISTEN:'127.0.0.1:18273'},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}if(i===99)throw Error('Temporary server unavailable');await new Promise(r=>setTimeout(r,100));}
  browser=await chromium.launch({channel:'chrome',headless:true});const ctx=await browser.newContext();await ctx.addInitScript(()=>{localStorage.setItem('rightcommit.hideAgentSetupNotice','true');localStorage.setItem('nodo.language','en')});const page=await ctx.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));for(const file of ['app.js','schedules.js','i18n.js'])await page.route('**/'+file,r=>r.fulfill({path:path.join(root,'web',file),contentType:'text/javascript'}));
  const api=async(url,body)=>{const r=await ctx.request.fetch(base+'/api'+url,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},...(body?{data:body}:{})});const text=await r.text();assert(r.ok(),url+': '+text);return JSON.parse(text);};
  const finished=async id=>{for(let i=0;i<100;i++){const d=await api('/runs/'+id);assert.notEqual(d.run.status,'failed',d.run.error);if(d.run.status==='completed')return d.run;await page.waitForTimeout(100);}throw Error('Run did not complete');};
  const workflow=(id,folder)=>({id,name:'Workflow '+id,description:'Local read test',entry:'read',nodes:[{id:'source',kind:'Source',title:'Source',source_type:'folder',source_mode:'copy',location:folder,x:40,y:60},{id:'read',kind:'Connection',title:'Read local evidence',provider:'local',operation:'read_ticket',location:'evidence.txt',x:300,y:60}],edges:[{from:'source',to:'read',kind:'context',outcome:''}]});
  await api('/auth/register',{email:'history-run@test.local',password:'temporary-history-run-password'});let original=workflow('history-workflow','first');await api('/workflows',original);const historical=await finished((await api('/runs',{workflow_id:original.id})).id);
  const empty={id:'empty-background-draft',name:'Empty background draft',description:'',nodes:[],edges:[],entry:''};await api('/workflows',empty);const other=workflow('other-background-workflow','other');await api('/workflows',other);
  // New executions use the latest saved definition; the completed snapshot stays immutable.
  original.nodes[0].location='latest';await api('/workflows',original);
  await page.goto(base);await page.locator('#app').waitFor();
  const startFromHistory=async background=>{
   await page.evaluate(id=>selectWorkflow(id),background);await page.evaluate(id=>loadRun(id),historical.id);
   assert.equal(await page.evaluate(()=>app.wf.id),background);assert.equal(await page.evaluate(()=>currentWorkflow().nodes.length),2);
   await page.locator('#execute').click();await page.locator('[data-run-now]').click();await page.locator('#run-form').waitFor();
   await page.locator('#run-form [name=input]').fill('New execution from visible history');
   const response=page.waitForResponse(r=>r.url()===base+'/api/runs'&&r.request().method()==='POST');await page.locator('[data-action=start-run]').click();const r=await response,j=await r.json();assert(r.ok(),JSON.stringify(j));assert.equal(r.request().postDataJSON().workflow_id,original.id);
   const completed=await finished(j.id);assert.notEqual(completed.id,historical.id);assert.equal(completed.workflow.id,original.id);assert.equal(completed.outputs.read.summary,'latest source evidence');assert.equal(completed.input,'New execution from visible history');
   const unchanged=await api('/runs/'+historical.id);assert.equal(unchanged.run.workflow.nodes[0].location,'first');
   return completed.id;
  };
  await startFromHistory(empty.id);await startFromHistory(other.id);
  assert.equal((await api('/workflows')).find(w=>w.id===empty.id).nodes.length,0);
  // Normal editor execution still uses its own workflow.
  await page.evaluate(id=>selectWorkflow(id),other.id);await page.locator('#execute').click();await page.locator('[data-run-now]').click();const normalResponse=page.waitForResponse(r=>r.url()===base+'/api/runs'&&r.request().method()==='POST');await page.locator('[data-action=start-run]').click();const normal=await normalResponse,body=await normal.json();assert(normal.ok(),JSON.stringify(body));assert.equal((await finished(body.id)).outputs.read.summary,'other source evidence');
  // A missing saved workflow is explained before submitting an unrelated background draft.
  await page.evaluate(id=>loadRun(id),historical.id);await page.route('**/api/workflows',async route=>{if(route.request().method()!=='GET')return route.continue();const r=await route.fetch(),rows=await r.json();return route.fulfill({json:rows.filter(w=>w.id!==original.id)});});
  await page.locator('#execute').click();await page.locator('[data-run-now]').click();await page.waitForFunction(()=>document.querySelector('#toast')?.textContent.includes('no longer available'));
  assert.equal(await page.locator('#run-form').isVisible(),false);assert.deepEqual(errors,[]);
  console.log('PASS: history Run now uses the selected saved workflow with an empty/different background draft, latest Source, preserved historical run, normal editor execution and missing-workflow handling; no model/service calls.');
 }finally{await browser?.close();if(server.exitCode===null){const stopped=new Promise(r=>server.once('exit',r));server.kill('SIGINT');await stopped;}fs.rmSync(data,{recursive:true,force:true});}
})().catch(e=>{console.error(e);process.exit(1)});
