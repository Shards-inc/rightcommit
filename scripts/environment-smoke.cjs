// Disposable server; provider credentials below are fake and no AI requests are made.
const fs=require('fs'),os=require('os'),path=require('path'),assert=require('node:assert/strict'),{spawn}=require('child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'rightcommit-environment-')),base='http://127.0.0.1:18102',sentinel='dummy-environment-secret';
 const env=Object.fromEntries(Object.entries(process.env).filter(([key])=>! /^(NODO_|ANTHROPIC_|CLAUDE_CODE_|CODEX_|OPENAI_|AWS_|GOOGLE_|AZURE_)/.test(key)));
 Object.assign(env,{NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18102',NODO_ASSETS:path.resolve('.'),ANTHROPIC_AUTH_TOKEN:sentinel,ANTHROPIC_BASE_URL:'https://gateway.invalid',NODO_TEST_EMPTY:'',NODO_TEST_NAME:sentinel});
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{cwd:dir,env,stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1050}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  const api=async(context,url,body,method)=>{const response=await context.request.fetch(base+'/api'+url,{method:method||(body?'POST':'GET'),headers:{'X-Nodo-Request':'1'},...(body?{data:body}:{})});assert(response.ok(),await response.text());return response.json()};
  assert.equal((await page.request.get(base+'/api/environment')).status(),401);
  await api(page,'/auth/register',{email:'admin@env.invalid',password:'temporary-environment-password'});
  const authDir=path.join(dir,'users/1/.codex');fs.mkdirSync(authDir,{recursive:true});fs.writeFileSync(path.join(authDir,'auth.json'),JSON.stringify({tokens:{access_token:'dummy-external-secret'}}));
  const data=await api(page,'/environment');assert.equal(data.authentication.claude.method,'bearer_token');assert.equal(data.authentication.codex.method,'external_configuration');assert.equal(data.authentication.codex.login_file,undefined);assert(!JSON.stringify(data).includes(sentinel));assert(!JSON.stringify(data).includes('dummy-external-secret'));assert(data.names.includes('NODO_TEST_NAME'));assert(!data.names.includes('NODO_TEST_EMPTY'));assert.deepEqual(data.names,[...new Set(data.names)].sort());
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>{if(document.querySelector('[data-agent-setup-notice]'))closeModal();return settingsDialog('environment')});
  await page.locator('[data-auth-runtime=codex]').waitFor();assert.equal(await page.locator('[data-login-file-status],[data-action=import-host]').count(),0);
  assert(!/Login ChatGPT|File di login|subscription login/i.test(await page.locator('#settings-panel-environment').textContent()));
  await page.evaluate(()=>{const el=document.querySelector('[data-language]');el.value='en';el.dispatchEvent(new Event('change',{bubbles:true}))});assert((await page.locator('[data-auth-runtime=codex]').textContent()).includes('External configuration'));
  await page.setViewportSize({width:390,height:900});assert(await page.locator('#settings-form').evaluate(el=>el.scrollWidth<=el.clientWidth));
  const saved=await api(page,'/settings');await api(page,'/settings',{revision:saved._revision,changes:{agents:{codex:{api_key:sentinel}}}},'PATCH');
  await api(page,'/users',{email:'member@env.invalid',password:'temporary-environment-password'});const other=await browser.newContext();await api(other,'/auth/login',{email:'member@env.invalid',password:'temporary-environment-password'});
  const otherData=await api(other,'/environment');assert.equal(otherData.authentication.codex.source,'administrator_api_key');assert.equal(otherData.authentication.codex.login_file,undefined);assert(!JSON.stringify(otherData).includes(sentinel));
  const settings=await api(other,'/settings');assert.equal(settings.agents.codex.uses_admin_api_key,true);assert.equal(settings.agents.codex.api_key_preview,null);assert.equal(settings.agents.codex.api_key,'');
  assert.equal((await page.request.post(base+'/api/auth/import-host',{headers:{'X-Nodo-Request':'1'},data:{runtime:'codex'}})).status(),404);
  await other.close();assert.deepEqual(errors,[]);console.log('PASS: environment names and credential summaries without login details, administrator API fallback hidden from member, IT/EN, mobile and removed import endpoint.');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(dir,{recursive:true,force:true})}
})().catch(error=>{console.error(error);process.exit(1)});
