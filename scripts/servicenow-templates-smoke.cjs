// Create all four ServiceNow workflows through the real template picker, without running them.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const root=path.resolve('.'),data=fs.mkdtempSync(path.join(os.tmpdir(),'servicenow-templates-')),base='http://127.0.0.1:18268';
 const server=spawn(path.join(root,'target/debug/rightcommit'),[],{cwd:data,env:{...process.env,NODO_DATA:data,NODO_ASSETS:root,NODO_LISTEN:'127.0.0.1:18268'}});let browser,log='';server.stderr.on('data',b=>log+=b);
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}if(i===99)throw Error(log);await new Promise(r=>setTimeout(r,100));}
  browser=await chromium.launch({channel:'chrome',headless:true});const context=await browser.newContext();await context.addInitScript(()=>{localStorage.setItem('rightcommit.hideAgentSetupNotice','true');localStorage.setItem('nodo.language','en')});
  async function api(route,body){const response=await context.request.fetch(base+'/api'+route,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},...(body?{data:body}:{})});const result=await response.json();assert(response.ok(),JSON.stringify(result));return result;}
  await api('/auth/register',{email:'servicenow-templates@test.local',password:'temporary-template-test-password'});
  const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(base);await page.locator('#app').waitFor();
  const catalog=await api('/template-catalog');assert.deepEqual(catalog.templates.filter(t=>t.section==='servicenow-itsm').map(t=>t.id),['servicenow-oauth-check','itsm-generic','itsm-northstar','servicenow-requests-from-files','servicenow-request-to-speckit','servicenow-triage']);assert(catalog.sections.some(s=>s.id==='servicenow-itsm'));const ids=['servicenow-oauth-check','itsm-generic','itsm-northstar','servicenow-requests-from-files','servicenow-request-to-speckit'];
  for(const id of ids){
   const template=catalog.templates.find(t=>t.id===id);assert(template,'Missing published template '+id);assert.equal(template.section,'servicenow-itsm');
   await page.locator('#new-workflow').click();const card=page.locator(`[data-template-id="${id}"][data-action="create-template"]`);await card.waitFor();assert.equal(await card.locator('strong').textContent(),template.locales.en.title);
   await card.click();await page.locator('#template-workflow-form [name=workflow_name]').fill('Test '+id);
   const saved=page.waitForResponse(r=>r.url()===base+'/api/workflows'&&r.request().method()==='POST');await page.locator('[data-action=save-template]').click();assert((await saved).ok());await page.locator('#template-workflow-form').waitFor({state:'hidden'});
   const workflows=await api('/workflows');const workflow=workflows.find(w=>w.name==='Test '+id);assert(workflow);const source=workflow.nodes.find(n=>n.kind==='Source');assert.equal(source.source_type,'examples');assert.equal(source.source_mode,'copy');assert.equal(source.location,id.startsWith('itsm-')?'itsm':'servicenow');
   if(id==='servicenow-oauth-check'){assert(workflow.nodes.find(n=>n.id==='report').prompt.includes('in English'));assert.equal(workflow.nodes.find(n=>n.id==='check-servicenow').title,'Check OAuth and incident read access');assert(workflow.nodes.filter(n=>n.kind==='Connection').every(n=>n.operation==='check_connection'));}
   if(id==='servicenow-request-to-speckit'){assert.equal(workflow.nodes.find(n=>n.id==='specify').command,'specify');assert(workflow.nodes.filter(n=>n.kind==='Connection').every(n=>n.operation==='read_request'));}
  }
  await page.evaluate(()=>{localStorage.setItem('nodo.language','it');window.dispatchEvent(new StorageEvent('storage',{key:'nodo.language',newValue:'it'}))});await page.locator('#new-workflow').click();
  for(const id of ids){const card=page.locator(`[data-template-id="${id}"][data-action="create-template"]`);assert.equal(await card.locator('strong').textContent(),catalog.templates.find(t=>t.id===id).locales.it.title);}
  await page.locator('[data-template-id="servicenow-oauth-check"][data-action="create-template"]').click();
  await page.locator('#template-workflow-form [name=workflow_name]').fill('Test OAuth italiano');
  const savedItalian=page.waitForResponse(r=>r.url()===base+'/api/workflows'&&r.request().method()==='POST');await page.locator('[data-action=save-template]').click();assert((await savedItalian).ok());await page.locator('#template-workflow-form').waitFor({state:'hidden'});
  const italian=(await api('/workflows')).find(w=>w.name==='Test OAuth italiano');assert(italian);
  assert.equal(italian.nodes.find(n=>n.id==='check-servicenow').title,'Verifica OAuth e lettura incident');assert(italian.nodes.find(n=>n.id==='report').prompt.includes('in italiano'));assert(italian.nodes.find(n=>n.id==='notes').text.includes('Configurare'));
  assert.equal((await api('/runs')).length,0);assert.deepEqual(errors,[]);console.log('PASS: ServiceNow template order and creation in IT/EN, including OAuth node titles, prompts and setup notes; portable Sources; no workflow runs or service calls.');
 }finally{await browser?.close();if(server.exitCode===null){const stopped=new Promise(resolve=>server.once('exit',resolve));server.kill('SIGINT');await stopped;}fs.rmSync(data,{recursive:true,force:true});}
})().catch(error=>{console.error(error);process.exit(1)});
