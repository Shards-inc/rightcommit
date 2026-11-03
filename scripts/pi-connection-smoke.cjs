const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const data=fs.mkdtempSync(path.join(os.tmpdir(),'pi-check-')),base='http://127.0.0.1:18254',binary=path.join(data,'system/tools/bin/pi');
 fs.mkdirSync(path.dirname(binary),{recursive:true});
 fs.writeFileSync(binary,`#!/bin/sh
case "$OPENAI_API_KEY:$XAI_API_KEY:$DEEPSEEK_API_KEY" in
  draft-pi-key::|:draft-pi-key:|::draft-pi-key) ;; *) exit 9 ;;
esac
sleep 0.3
printf '%s\\n' '{"type":"message_end","message":{"role":"assistant","stopReason":"stop","content":[{"type":"text","text":"OK"}]}}'
`,{mode:0o700});
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:data,NODO_LISTEN:'127.0.0.1:18254'},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  for(const file of ['app.js','app.css','i18n.js','controls.js'])await page.route('**/'+file,r=>r.fulfill({path:path.resolve('web/'+file)}));
  await page.request.post(base+'/api/auth/register',{headers:{'X-Nodo-Request':'1'},data:{email:'pi-check@test.local',password:'temporary-pi-check-password'}});
  const read=async()=>await (await page.request.get(base+'/api/settings')).json();const original=await read();
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>settingsDialog('agents'));
  const button=page.locator('[data-action=pi-check]'),status=page.locator('[data-pi-status]');
  assert.equal(await button.textContent(),'Test connection');assert.equal(await button.evaluate(el=>el.closest('[data-service]').dataset.service),'pi');
  await button.click();await status.filter({hasText:'Choose a Pi model'}).waitFor();assert(await button.isEnabled());
  await page.locator('[name=provider_pi]').getByRole('combobox').click();await page.locator('[name=provider_pi]').getByRole('option',{name:'OpenAI',exact:true}).click();
  await page.locator('[name=model_pi]').fill('test-model');await page.locator('[name=key_pi]').fill('draft-pi-key');
  await button.click();await page.waitForFunction(()=>document.querySelector('[data-action=pi-check]').disabled);await status.filter({hasText:'Pi connection successful'}).waitFor();assert(await button.isEnabled());
  assert.deepEqual(await read(),original);assert.equal(await page.locator('[name=key_pi]').inputValue(),'draft-pi-key');
  for(const [provider,label] of [['xai','xAI (Grok)'],['deepseek','DeepSeek']]){
   await page.locator('[name=provider_pi]').getByRole('combobox').click();await page.locator('[name=provider_pi]').getByRole('option',{name:label,exact:true}).click();
   await button.click();await page.waitForFunction(()=>!document.querySelector('[data-action=pi-check]').disabled&&document.querySelector('[data-pi-status]').textContent.includes('Pi connection successful'));
   assert.equal(await page.locator('[name=provider_pi]').evaluate(el=>el.value),provider);assert.deepEqual(await read(),original);
  }
  fs.writeFileSync(binary,"#!/bin/sh\necho '401 draft-pi-key private-header' >&2\nexit 1\n");
  await button.click();await status.filter({hasText:'Pi authentication failed'}).waitFor();assert(!(await status.textContent()).includes('draft-pi-key'));assert.deepEqual(await read(),original);
  await page.evaluate(()=>{const language=document.querySelector('[data-language]');language.value='it';language.dispatchEvent(new Event('change',{bubbles:true}))});
  assert.equal(await button.textContent(),'Verifica connessione');await button.click();await status.filter({hasText:'Autenticazione Pi fallita'}).waitFor();
  await page.locator('[data-action=save-settings]').click();await page.waitForFunction(()=>app.settings.agents.pi.provider==='deepseek');
  assert((await read()).agents.pi.configured);await page.evaluate(()=>settingsDialog('agents'));assert.equal(await page.locator('[name=provider_pi]').evaluate(el=>el.value),'deepseek');
  await page.screenshot({path:'/tmp/pi-connection-desktop.png'});assert.deepEqual(errors,[]);
  console.log('PASS: Pi-only draft check, actual native fixture, empty model, success/error IT/EN, loading state, xAI/DeepSeek routing and persistence, no implicit save, no secret leakage');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
