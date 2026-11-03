// Simulate a password manager filling unrelated fields; use only disposable dummy credentials.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const data=fs.mkdtempSync(path.join(os.tmpdir(),'settings-autofill-')),base='http://127.0.0.1:18251';
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:data,NODO_LISTEN:'127.0.0.1:18251',NODO_ASSETS:process.cwd()},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1400,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  const api=async(p,method='GET',body)=>{const r=await page.request.fetch(base+'/api'+p,{method,headers:{'X-Nodo-Request':'1'},data:body});assert(r.ok(),await r.text());return r.json()};
  await api('/auth/register','POST',{email:'autofill@test.local',password:'temporary-autofill-password'});
  let settings=await api('/settings');settings.agents.codex.model='original-model';settings.agents.codex.api_key='dummy-agent-api-key';settings.connections.servicenow={base_url:'https://dev12345.service-now.com',authentication:'basic',username:'rightcommit.api',password:'pdi-only-secret',token:'',catalog_variables:'{}'};
  await api('/settings','PUT',settings);const original=await api('/settings');
  for(const file of ['app.js','i18n.js','controls.js','mcp.js','a2a.js'])await page.route('**/'+file,r=>r.fulfill({path:path.resolve('web/'+file)}));
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>settingsDialog('agents'));
  const forms=await page.evaluate(()=>({codex:document.querySelector('[name=model_codex]').form.id,pi:document.querySelector('[name=model_pi]').form.id,sn:document.querySelector('[name=sn_password]').form.id,nested:document.querySelectorAll('#settings-form form form').length}));assert.equal(new Set([forms.codex,forms.pi,forms.sn]).size,3);assert.equal(forms.nested,0);
  const patches=[];page.on('request',r=>{if(r.url()===base+'/api/settings'&&r.method()==='PATCH')patches.push(r.postDataJSON())});
  await page.locator('[name=model_pi]').fill('pi-test-model');
  await page.locator('[name=provider_pi]').getByRole('combobox').click();await page.locator('[name=provider_pi]').getByRole('option',{name:'OpenAI',exact:true}).click();
  await page.locator('[name=key_pi]').fill('dummy-pi-api-key');
  // A manager changes the first model/password pair without the user touching that pair.
  await page.evaluate(()=>{for(const [name,value] of [['model_codex','rightcommit.api'],['key_codex','pdi-only-secret']]){const field=document.querySelector(`[name="${name}"]`);field.value=value;field.dispatchEvent(new Event('input',{bubbles:true}));field.dispatchEvent(new Event('change',{bubbles:true}))}});
  await page.evaluate(()=>{window.saved=false;window.addEventListener('settings-saved',()=>{window.saved=true},{once:true})});await page.locator('[data-action=save-settings]').click();await page.waitForFunction(()=>window.saved);
  settings=await api('/settings');assert.equal(settings.agents.pi.model,'pi-test-model');assert.equal(settings.agents.pi.provider,'openai');assert(settings.agents.pi.configured);assert(settings.agents.pi.api_key_preview);assert.equal(settings.agents.codex.model,'original-model');assert.equal(settings.agents.codex.api_key_preview,original.agents.codex.api_key_preview);assert.equal(settings.connections.servicenow.username,'rightcommit.api');assert(settings.connections.servicenow.configured);
  assert.deepEqual(Object.keys(patches[0].changes),['agents']);assert.deepEqual(Object.keys(patches[0].changes.agents),['pi']);
  // Direct requests also reject this credential mix, even if client protections are bypassed.
  settings.agents.codex.api_key='pdi-only-secret';const response=await page.request.put(base+'/api/settings',{headers:{'X-Nodo-Request':'1'},data:settings});assert.equal(response.status(),400);assert((await response.json()).error.includes('autocompilazione'));
  assert.equal((await api('/settings')).agents.codex.api_key_preview,original.agents.codex.api_key_preview);
  // A second window saves after this form was opened: reject the stale draft and retain input.
  await page.evaluate(()=>settingsDialog('agents'));await page.locator('[name=model_pi]').fill('pending-local-edit');
  let latest=await api('/settings');await api('/settings','PATCH',{revision:latest._revision,changes:{task_timeout_secs:7201}});
  await page.locator('[data-action=save-settings]').click();await page.waitForFunction(()=>document.querySelector('#settings-error')?.textContent.includes('Settings have changed'));
  assert.equal(await page.locator('[name=model_pi]').inputValue(),'pending-local-edit');latest=await api('/settings');assert.equal(latest.task_timeout_secs,7201);assert.equal(latest.agents.pi.model,'pi-test-model');
  // MCP saves only its own record, with protected credential fields.
  await page.evaluate(()=>mcpEdit());await page.locator('#mcp-form [name=name]').fill('Isolation MCP');await page.locator('#mcp-form [name=url]').fill('https://mcp.example.com');
  await page.locator('#mcp-form [name=auth_mode]').getByRole('combobox').click();await page.locator('#mcp-form [name=auth_mode]').getByRole('option',{name:'Bearer token',exact:true}).click();await page.locator('#mcp-form [name=auth_token]').fill('dummy-mcp-token');
  await page.locator('[data-action=mcp-save]').click();await page.waitForFunction(()=>Object.values(app.settings.mcp_servers).some(c=>c.name==='Isolation MCP'));
  assert.deepEqual(Object.keys(patches.at(-1).changes),['mcp_servers']);assert((await api('/settings')).connections.servicenow.configured);
  // A2A has its own form and version, and never posts global settings.
  await page.evaluate(()=>a2aConnectionDialog());await page.locator('#a2a-connection-form [name=name]').fill('Isolation A2A');await page.locator('#a2a-connection-form [name=url]').fill('https://a2a.example.com/card');await page.locator('#a2a-connection-form [name=token]').fill('dummy-a2a-token');
  await page.locator('[data-action=a2a-save-connection]').click();await page.waitForFunction(()=>a2aConnections.some(c=>c.name==='Isolation A2A'));
  const connection=(await api('/a2a/connections')).find(c=>c.name==='Isolation A2A');assert(connection.has_token);assert(connection._revision);
  const updated=await api('/a2a/connections','POST',{...connection,_revision:connection._revision,name:'Updated A2A',token:''});assert.equal(updated.id,connection.id);
  const conflict=await page.request.post(base+'/api/a2a/connections',{headers:{'X-Nodo-Request':'1'},data:{...connection,_revision:connection._revision,name:'Stale A2A',token:''}});assert.equal(conflict.status(),409);
  assert.equal((await api('/settings')).agents.codex.model,'original-model');
  const backups=fs.readdirSync(path.join(data,'system/settings-history/1'));assert(backups.length>=2);for(const file of backups){const bytes=fs.readFileSync(path.join(data,'system/settings-history/1',file));assert(!bytes.includes(Buffer.from('pdi-only-secret')));assert(!bytes.includes(Buffer.from('dummy-agent-api-key')))}
  assert.deepEqual(errors,[]);console.log('PASS: unrelated autofill ignored, intentional Pi edit saved, ServiceNow preserved, server rejects credential mix, encrypted settings backups, isolated forms, scoped PATCH, stale-save preservation, MCP/A2A revision checks');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
