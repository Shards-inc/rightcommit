const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),assert=require('node:assert/strict'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'preview-settings-')),base='http://127.0.0.1:18163',sources=path.join(dir,'sources');fs.mkdirSync(sources);
 fs.writeFileSync(path.join(sources,'catalogo.md'),'# Catalogo leggibile\n'+'contenuto\n'.repeat(33000));
 fs.writeFileSync(path.join(sources,'large.txt'),'x'.repeat(1_500_000));
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18163',NODO_SOURCES:sources},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1050}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  for(const file of ['app.js','i18n.js'])await page.route('**/'+file,r=>r.fulfill({path:path.resolve('web/'+file)}));
  const request=async(p,data,method=data?'POST':'GET')=>{const r=await page.request.fetch(base+'/api'+p,{method,headers:{'X-Nodo-Request':'1'},data});assert(r.ok(),await r.text());return r.json()};
  await request('/auth/register',{email:'previews@test.local',password:'temporary-previews-password'});assert.equal((await request('/settings')).file_preview_limit_mb,5);
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});
  const settings=async()=>{await page.locator('#account').click();await page.locator('#account-menu [data-action=settings]').click();await page.locator('[data-settings-tab=general]').click();await page.locator('[name=file_preview_limit_mb]').waitFor({state:'visible'})};
  await settings();assert.equal(await page.locator('[name=file_preview_limit_mb]').inputValue(),'5');
  await page.locator('[name=file_preview_limit_mb]').fill('1');await page.locator('[data-action=save-settings]').click();await page.waitForFunction(()=>app.settings.file_preview_limit_mb===1);
  assert.equal((await request('/settings')).file_preview_limit_mb,1);await page.locator('#dialog .modal-head [data-action=close-dialog]').click();
  await page.evaluate(()=>openStorage('catalogo.md','sources'));await page.locator('#dialog .markdown h1').waitFor({state:'visible'});assert.equal(await page.locator('#dialog .markdown h1').innerText(),'Catalogo leggibile');
  await page.evaluate(()=>openStorage('large.txt','sources'));assert((await page.locator('#dialog .modal-body').innerText()).includes('1 MB'));assert((await page.locator('#dialog .modal-body').innerText()).includes('Configurazioni personali'));
  assert.equal(await page.locator('#dialog .source-view').count(),0);await page.locator('#dialog .modal-head [data-action=close-dialog]').click();
  await settings();assert.equal(await page.locator('[name=file_preview_limit_mb]').inputValue(),'1');await page.locator('[name=file_preview_limit_mb]').fill('2');await page.locator('[data-action=save-settings]').click();await page.waitForFunction(()=>app.settings.file_preview_limit_mb===2);
  await page.locator('#dialog .modal-head [data-action=close-dialog]').click();await page.evaluate(()=>openStorage('large.txt','sources'));await page.locator('#dialog .source-view').waitFor({state:'visible'});assert.equal((await page.locator('#dialog .source-view').innerText()).length,1_500_000);
  const prefs=await request('/settings');const invalid=await page.request.put(base+'/api/settings',{headers:{'X-Nodo-Request':'1'},data:{...prefs,file_preview_limit_mb:0}});assert.equal(invalid.status(),400);assert.equal((await request('/settings')).file_preview_limit_mb,2);
  await page.reload();await page.locator('#app').waitFor({state:'visible'});await settings();assert.equal(await page.locator('[name=file_preview_limit_mb]').inputValue(),'2');
  await page.evaluate(()=>{const language=document.querySelector('[data-language]');language.value='en';language.dispatchEvent(new Event('change',{bubbles:true}))});
  assert((await page.locator('#settings-panel-general').innerText()).includes('File previews'));
  await page.setViewportSize({width:390,height:844});assert(await page.locator('#settings-form').evaluate(el=>el.scrollWidth<=el.clientWidth));
  assert.deepEqual(errors,[]);console.log('PASS: default 5 MB, Personal Settings save/reload, Markdown over 256 KB, configurable text preview, clear size message, validation, English and mobile');
 }finally{await browser?.close();server.kill('SIGINT');fs.rmSync(dir,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
