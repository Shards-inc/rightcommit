// Create GitHub workflows through the real template picker without remote calls.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const root=path.resolve('.'),data=fs.mkdtempSync(path.join(os.tmpdir(),'github-templates-')),base='http://127.0.0.1:18268';
 const server=spawn(path.join(root,'target/debug/rightcommit'),[],{cwd:data,env:{...process.env,NODO_DATA:data,NODO_ASSETS:root,NODO_LISTEN:'127.0.0.1:18268'}});let browser,log='';server.stderr.on('data',b=>log+=b);
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}if(i===99)throw Error(log);await new Promise(r=>setTimeout(r,100));}
  browser=await chromium.launch({channel:'chrome',headless:true});const context=await browser.newContext();await context.addInitScript(()=>{localStorage.setItem('rightcommit.hideAgentSetupNotice','true');localStorage.setItem('nodo.language','en')});
  async function api(route,body){const response=await context.request.fetch(base+'/api'+route,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},...(body?{data:body}:{})});const result=await response.json();assert(response.ok(),JSON.stringify(result));return result;}
  await api('/auth/register',{email:'servicenow-templates@test.local',password:'temporary-template-test-password'});
  const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(base);await page.locator('#app').waitFor();
  const catalog=await api('/template-catalog');
  const ids=['github-connection-check','github-issue-to-requirements','github-ci-readiness','github-ci-setup'];
  for(const lang of ['en','it']){
   await page.evaluate(lang=>{localStorage.setItem('nodo.language',lang);window.dispatchEvent(new StorageEvent('storage',{key:'nodo.language',newValue:lang}))},lang);
   for(const id of ids){
    const template=catalog.templates.find(t=>t.id===id);assert(template);assert.equal(template.section,'connected');
    await page.locator('#new-workflow').click();const card=page.locator(`[data-template-id="${id}"][data-action="create-template"]`);await card.waitFor();assert.equal(await card.locator('strong').textContent(),template.locales[lang].title);
    await card.click();await page.locator('#template-workflow-form [name=workflow_name]').fill(lang+' '+id);
    const saved=page.waitForResponse(r=>r.url()===base+'/api/workflows'&&r.request().method()==='POST');await page.locator('[data-action=save-template]').click();assert((await saved).ok());await page.locator('#template-workflow-form').waitFor({state:'hidden'});
    const w=(await api('/workflows')).find(w=>w.name===lang+' '+id);assert(w);assert.equal(w.execution.allow_external_writes,false);
    const connection=w.nodes.find(n=>n.id==='github');assert.equal(connection.provider,'github');assert.equal(connection.operation,['github-ci-readiness','github-ci-setup'].includes(id)?'read_ci':'read_ticket');
    if(id==='github-ci-setup'){const publish=w.nodes.find(n=>n.id==='publish-ci');assert.equal(publish.api_method,'PUT');assert.equal(publish.operation,'api_request');assert.equal(JSON.parse(publish.api_body).sha,undefined);assert.equal(Buffer.from(JSON.parse(publish.api_body).content,'base64').toString(),fs.readFileSync('examples/github-ci/demo-ci.yml','utf8'));assert.equal(w.nodes.find(n=>n.id==='wait').mode,'human');assert.equal(w.nodes.find(n=>n.id==='source').source_mode,'copy');}
    assert.equal(w.nodes.find(n=>n.id==='report').prompt,template.locales[lang].nodes.report.prompt);
   }
  }
  assert.equal((await api('/runs')).length,0);assert.deepEqual(errors,[]);console.log('PASS: four GitHub templates created in IT/EN; native reads and approval-gated CI creation, localized reports, no runs or remote calls.');
 }finally{await browser?.close();if(server.exitCode===null){const stopped=new Promise(resolve=>server.once('exit',resolve));server.kill('SIGINT');await stopped;}fs.rmSync(data,{recursive:true,force:true});}
})().catch(error=>{console.error(error);process.exit(1)});
