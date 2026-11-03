const fs=require('fs'),os=require('os'),path=require('path'),assert=require('assert'),{spawn}=require('child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{const dir=fs.mkdtempSync(path.join(os.tmpdir(),'settings-layout-')),base='http://127.0.0.1:18159';const server=spawn(path.resolve('target/debug/rightcommit'),[],{cwd:dir,env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18159',NODO_SOURCES:path.resolve('sources')},stdio:'ignore'});let browser;try{
for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1050}});const errors=[];page.on('pageerror',e=>errors.push(e.message));for(const f of ['controls.js','app.js','app.css','i18n.js','a2a.js','mcp.js','rag.js','manager.js','mcp-oauth.js','icons.svg'])await page.route('**/'+f,r=>r.fulfill({path:path.resolve('web/'+f)}));
await page.route(base+'/',r=>r.fulfill({path:path.resolve('web/index.html'),contentType:'text/html'}));
const password='temporary-account-password';await page.request.post(base+'/api/auth/register',{headers:{'X-Nodo-Request':'1'},data:{email:'account-admin@test.local',password}});await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>{if(document.querySelector('[data-agent-setup-notice]'))closeModal()});

await page.evaluate(()=>settingsDialog('manager'));
const toggle=page.locator('jui-switch[name=manager_write_enabled]');
assert.equal(await toggle.evaluate(el=>el.checked),true);
for(const enabled of [false,true]){
 await toggle.click();
 assert.equal(await toggle.evaluate(el=>el.checked),enabled);
 const saved=page.waitForResponse(r=>r.url()===base+'/api/settings'&&r.request().method()==='PATCH');
 await page.locator('[data-action=save-settings]').click();
 const response=await saved;assert.equal(response.status(),200,await response.text());
 assert.equal((await response.json()).manager_write_enabled,enabled);
 await page.waitForFunction(()=>!document.querySelector('[data-settings-root]')?.settingsSaving);
 await page.evaluate(()=>settingsDialog('manager'));
 assert.equal(await toggle.evaluate(el=>el.checked),enabled);
}
assert.deepEqual(errors,[]);console.log('PASS: manager switch default on, disable, save/reopen, enable and save/reopen');
}finally{await browser?.close();server.kill();fs.rmSync(dir,{recursive:true,force:true})}})().catch(e=>{console.error(e);process.exit(1)});
