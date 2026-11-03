// Create Atlassian workflows in IT/EN without remote calls.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const root=path.resolve('.'),data=fs.mkdtempSync(path.join(os.tmpdir(),'atlassian-templates-')),base='http://127.0.0.1:18268';
 const server=spawn(path.join(root,'target/debug/rightcommit'),[],{cwd:data,env:{...process.env,NODO_DATA:data,NODO_ASSETS:root,NODO_LISTEN:'127.0.0.1:18268'}});let browser,log='';server.stderr.on('data',b=>log+=b);
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}if(i===99)throw Error(log);await new Promise(r=>setTimeout(r,100));}
  browser=await chromium.launch({channel:'chrome',headless:true});const context=await browser.newContext();await context.addInitScript(()=>{localStorage.setItem('rightcommit.hideAgentSetupNotice','true');localStorage.setItem('nodo.language','en')});
  async function api(route,body){const response=await context.request.fetch(base+'/api'+route,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},...(body?{data:body}:{})});const result=await response.json();assert(response.ok(),JSON.stringify(result));return result;}
  await api('/auth/register',{email:'servicenow-templates@test.local',password:'temporary-template-test-password'});
  const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(base);await page.locator('#app').waitFor();
  const catalog=await api('/template-catalog');
  const ids=['atlassian-jira-requirements','atlassian-jira-bitbucket-review','atlassian-confluence-brief'];
  for(const lang of ['en','it']){
   await page.evaluate(lang=>{localStorage.setItem('nodo.language',lang);window.dispatchEvent(new StorageEvent('storage',{key:'nodo.language',newValue:lang}))},lang);
   for(const id of ids){
    const template=catalog.templates.find(t=>t.id===id);assert(template);assert.equal(template.section,'connected');
    await page.locator('#new-workflow').click();const card=page.locator(`[data-template-id="${id}"][data-action="create-template"]`);await card.waitFor();assert.equal(await card.locator('strong').textContent(),template.locales[lang].title);
    await card.click();await page.locator('#template-workflow-form [name=workflow_name]').fill(lang+' '+id);
    const saved=page.waitForResponse(r=>r.url()===base+'/api/workflows'&&r.request().method()==='POST');await page.locator('[data-action=save-template]').click();assert((await saved).ok());await page.locator('#template-workflow-form').waitFor({state:'hidden'});
    const w=(await api('/workflows')).find(w=>w.name===lang+' '+id);assert(w);assert.equal(w.execution.allow_external_writes,false);
    const connections=w.nodes.filter(n=>n.kind==='Connection');assert.equal(connections.length,id==='atlassian-jira-bitbucket-review'?3:1);for(const n of connections){assert.equal(n.provider,'atlassian');assert.equal(n.operation,'twg');const args=JSON.parse(n.command);if(n.id==='diffstat'){assert(args.includes('diffstat'));assert(args.includes('--limit'));}else{assert(args.includes('get'));if(n.id!=='confluence')assert(args.includes('--strict'));else{assert(args.includes('--include-metadata'));assert(args.includes('PAGE_ID'));}}}const source=w.nodes.find(n=>n.id==='source');assert.equal(source.source_type,'examples');assert.equal(source.location,'atlassian');
    assert.equal(w.nodes.find(n=>n.id==='report').prompt,template.locales[lang].nodes.report.prompt);if(id==='atlassian-jira-bitbucket-review'){for(const id of ['capture-jira','capture-bitbucket','capture-diffstat'])assert(w.nodes.some(n=>n.id===id&&n.prompt.includes('capture_evidence.py capture')));assert(w.nodes.find(n=>n.id==='report').prompt.includes('capture_evidence.py verify'));}
   }
  }
  assert.equal((await api('/runs')).length,0);assert.deepEqual(errors,[]);console.log('PASS: three Atlassian templates created in IT/EN; native Jira/Bitbucket/Confluence reads, localized reports and portable Source; no runs or remote calls.');
 }finally{await browser?.close();if(server.exitCode===null){const stopped=new Promise(resolve=>server.once('exit',resolve));server.kill('SIGINT');await stopped;}fs.rmSync(data,{recursive:true,force:true});}
})().catch(error=>{console.error(error);process.exit(1)});
