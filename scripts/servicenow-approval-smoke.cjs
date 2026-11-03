// UI contract test with saved-format approval snapshots; no ServiceNow calls.
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),assert=require('node:assert/strict'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const data=fs.mkdtempSync(path.join(os.tmpdir(),'sn-approval-')),base='http://127.0.0.1:18258';let browser;
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{cwd:data,env:{...process.env,NODO_DATA:data,NODO_LISTEN:'127.0.0.1:18258'},stdio:'ignore'});
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.stack));
  for(const f of ['app.js','app.css','i18n.js'])await page.route('**/'+f,r=>r.fulfill({path:path.resolve('web/'+f)}));
  const register=await page.request.post(base+'/api/auth/register',{headers:{'X-Nodo-Request':'1'},data:{email:'approval@test.local',password:'temporary-approval-password'}});assert(register.ok());
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>{if(document.querySelector('[data-agent-setup-notice]'))closeModal()});
  const effect={instance:'https://dev12345.service-now.com',method:'POST',path:'/api/now/table/incident',body:{short_description:'VPN unavailable',description:'Ticket <script>window.pwned=true</script> {text}',impact:'3',urgency:'3',work_notes:'KB suggestion, unverified:\n[KB-001#chunk] Restart the client.\nKeep MFA enabled.',correlation_id:'rightcommit:example:123'},correlation:'rightcommit:example:123'};
  const show=async(value,options={})=>page.evaluate(async({value,options})=>{
   if(document.querySelector('#dialog').open)closeModal();
   app.reviewRun=null;app.run={id:'approval-ui-run',status:'waiting_review',current:'apply-plan',created:1793706088,updated:1793706157,workflow:{id:'approval-workflow',name:'ITSM approval demo',nodes:[{id:'apply-plan',kind:'Connection',provider:options.provider||'servicenow',location:'output/plan.json'}]},pending:{kind:options.kind||'effect',node:'apply-plan',questions:[options.question||`Autorizzare apply_ticket_plan su servicenow? ${JSON.stringify(value)} Controlla target e artefatti nelle impostazioni e nel run.`]}};
   await pending();
  },{value,options});
  const language=async lang=>page.evaluate(lang=>{const el=document.querySelector('[data-language]');el.value=lang;el.dispatchEvent(new Event('change',{bubbles:true}))},lang);
  for(const lang of ['it','en'])for(const width of [1440,390]){
   await page.setViewportSize({width,height:1000});await language(lang);await show(effect);
   const summary=page.locator('.approval-summary');await summary.waitFor();assert.match(await summary.locator('h3').textContent(),lang==='it'?/Crea un incidente/:/Create a ServiceNow incident/);
   assert.equal(await page.locator('.approval-json').evaluate(el=>el.open),false);assert.equal(await page.locator('.approval-json pre').isVisible(),false);
   assert((await summary.innerText()).includes(effect.instance));assert((await summary.innerText()).includes(effect.body.work_notes));assert((await summary.innerText()).includes('output/plan.json'));assert(!(await page.locator('#dialog-content').innerText()).includes('Autorizzare apply_ticket_plan'));
   assert.equal(await page.evaluate(()=>window.pwned),undefined);assert.equal(await summary.locator('script').count(),0);
   await page.locator('.approval-json summary').click();assert.deepEqual(JSON.parse(await page.locator('.approval-json pre').textContent()),effect);
   assert(await page.locator('#dialog-content').evaluate(el=>el.scrollWidth<=el.clientWidth+1));
   for(const decision of ['approve','reject'])assert(await page.locator(`[data-action=decide][data-value=${decision}]`).isVisible());
   await page.locator('.approval-json summary').click();
   if(width===1440)await page.screenshot({path:path.join(os.tmpdir(),'servicenow-approval-'+lang+'.png')});
  }
  await language('en');
  await show({...effect,method:'PATCH',path:'/api/now/table/incident/'+('a'.repeat(32)),body:{state:'6',close_code:'Solution provided',close_notes:'User confirmed VPN works'}});assert.match(await page.locator('.approval-summary h3').textContent(),/Record the incident resolution/);assert((await page.locator('.approval-target').innerText()).includes('a'.repeat(32)));
  await show({...effect,path:'/api/sn_sc/servicecatalog/items/'+('b'.repeat(32))+'/order_now',body:{sysparm_quantity:'1',variables:{software:'Python',justification:'Project <img src=x onerror=alert(1)>',approved:false,options:['one','two']}}});assert.match(await page.locator('.approval-summary h3').textContent(),/Submit a ServiceNow catalog request/);assert((await page.locator('.approval-fields').first().innerText()).includes('Python'));assert.equal(await page.locator('.approval-fields img').count(),0);
  for(const options of [{kind:'review',question:'Confirm this human review?'},{kind:'question',question:'Which ticket?'},{kind:'effect',provider:'github',question:'Authorize commit?'},{kind:'effect',question:'Autorizzare apply_ticket_plan su servicenow? {bad JSON} Controlla target e artefatti nelle impostazioni e nel run.'}]){await show(effect,options);assert.equal(await page.locator('.approval-summary').count(),0);assert((await page.locator('#dialog-content').innerText()).includes(options.question))}
  const batch={...effect,method:'BATCH',path:'',body:{namespace:'northstar-demo',items:[{request_id:'vpn',category:'Incident',action:'create_incident',status:'ready',input_digest:'test-hash',effect},{request_id:'software',category:'Request',action:'order_catalog',status:'blocked',message:'Configure catalog item'},{request_id:'faq',category:'Other',action:'answer_only',status:'ready',effect:{body:{answer:'KB answer'}}}]}};
  for(const lang of ['it','en']){await language(lang);await show(batch);assert.equal(await page.locator('.approval-batch-item').count(),3);assert((await page.locator('.approval-batch-item').nth(1).innerText()).includes('Configure catalog item'));assert.equal(await page.locator('.approval-json').evaluate(el=>el.open),false);assert(!(await page.locator('.approval-summary').innerText()).includes('test-hash'));await page.locator('.approval-json summary').click();assert.deepEqual(JSON.parse(await page.locator('.approval-json pre').textContent()),batch);}
  await show(effect);let submitted;await page.route('**/api/runs/approval-ui-run/decide',r=>{submitted=r.request().postDataJSON();return r.fulfill({status:400,json:{error:'Test-only decision capture'}})});
  await page.locator('#decision-answer').fill('Reviewed test fields');await page.locator('[data-action=decide][data-value=approve]').click();await page.waitForFunction(()=>document.querySelector('#dialog').open);for(let i=0;i<30&&!submitted;i++)await page.waitForTimeout(50);assert.deepEqual(submitted,{action:'approve',answer:'Reviewed test fields'});assert.deepEqual(errors,[]);
  console.log('PASS: existing approval format, exact JSON snapshot, incident/catalog/resolution summaries, all fields and multiline notes, IT/EN, mobile, XSS escaping, safe fallback and unchanged decision request');
 }finally{await browser?.close();if(server.exitCode===null){server.kill('SIGINT');await new Promise(r=>server.once('exit',r))}fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exitCode=1});
