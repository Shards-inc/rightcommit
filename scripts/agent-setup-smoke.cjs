const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const data=fs.mkdtempSync(path.join(os.tmpdir(),'agent-setup-')),base='http://127.0.0.1:18253';
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:data,NODO_LISTEN:'127.0.0.1:18253'},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
  const routeAssets=async target=>{for(const file of ['app.js','app.css','i18n.js','icons.svg','controls.js'])await target.route('**/'+file,r=>r.fulfill({path:path.resolve('web/'+file)}))};await routeAssets(page);
  await page.request.post(base+'/api/auth/register',{headers:{'X-Nodo-Request':'1'},data:{email:'setup@test.local',password:'temporary-setup-password'}});
  await page.goto(base);await page.locator('[data-agent-setup-notice]').waitFor({state:'visible'});
  assert.equal(await page.locator('#dialog h2').textContent(),'No agents configured');
  const preference=page.locator('[data-agent-setup-dismiss]');assert.equal(await preference.evaluate(el=>el.checked),false);
  assert.equal(await preference.getByRole('switch').getAttribute('aria-label'),"Don't show again");
  await preference.getByRole('switch').click();assert.equal(await page.evaluate(()=>localStorage.getItem('rightcommit.hideAgentSetupNotice')),'true');
  await preference.getByRole('switch').click();assert.equal(await page.evaluate(()=>localStorage.getItem('rightcommit.hideAgentSetupNotice')),'false');
  assert.equal(await page.locator('[data-action=configure-agents]').evaluate(el=>getComputedStyle(el).outlineStyle),'none');
  assert.equal(await page.locator('#dialog .modal-head .icon-button').evaluate(el=>getComputedStyle(el).borderTopWidth),'1px');
  assert.equal(await page.locator('.agent-setup-icon use').getAttribute('href'),'/icons.svg#bot-off');
  await page.waitForFunction(()=>document.querySelector('.agent-setup-icon svg').getBBox().width>0);
  await page.locator('[data-action=configure-agents]').click();await page.locator('#settings-panel-agents').waitFor({state:'visible'});
  await page.locator('#dialog .modal-head .icon-button').focus();assert.equal(await page.locator('#dialog .modal-head .icon-button').evaluate(el=>getComputedStyle(el).outlineStyle),'none');
  await page.locator('[name=provider_pi]').evaluate(el=>el.shadowRoot.querySelector('button').focus());assert.equal(await page.locator('[name=provider_pi]').evaluate(el=>getComputedStyle(el.shadowRoot.querySelector('button')).outlineStyle),'none');
  await page.keyboard.press('Escape');await page.evaluate(()=>{const language=document.querySelector('[data-language]');language.value='it';language.dispatchEvent(new Event('change',{bubbles:true}))});
  await page.reload();await page.locator('[data-agent-setup-notice]').waitFor({state:'visible'});assert.equal(await page.locator('#dialog h2').textContent(),'Nessun agente configurato');
  await page.setViewportSize({width:390,height:844});assert(await page.locator('#dialog').evaluate(el=>{const r=el.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth&&el.scrollWidth<=el.clientWidth}));
  await page.screenshot({path:'/tmp/agent-setup-mobile.png'});await page.locator('#dialog .modal-actions [data-action=close-dialog]').click();assert(await page.locator('#dialog').isHidden());

  // Persist immediately when toggled, and suppress after a reload in the same browser.
  await page.evaluate(()=>showAgentSetupNotice());assert.equal(await preference.evaluate(el=>el.checked),false);
  assert.equal(await preference.getByRole('switch').getAttribute('aria-label'),'Non visualizzare più');
  await preference.getByRole('switch').click();await page.locator('#dialog .modal-actions [data-action=close-dialog]').click();
  await page.reload();await page.locator('#app').waitFor({state:'visible'});assert(await page.locator('#dialog').isHidden());assert.equal(await page.evaluate(()=>agentSetupNoticeHidden()),true);

  // Personal Settings can restore the notice immediately without a server save.
  await page.evaluate(()=>settingsDialog('general'));const enabled=page.locator('[data-agent-setup-enabled]');assert.equal(await enabled.evaluate(el=>el.checked),false);
  assert.equal(await enabled.getByRole('switch').getAttribute('aria-label'),'Mostra l’avviso degli agenti non configurati');
  await enabled.getByRole('switch').click();assert.equal(await page.evaluate(()=>localStorage.getItem('rightcommit.hideAgentSetupNotice')),'false');
  await page.keyboard.press('Escape');await page.reload();await page.locator('[data-agent-setup-notice]').waitFor({state:'visible'});
  await page.locator('[data-agent-setup-dismiss]').getByRole('switch').click();await page.locator('[data-action=configure-agents]').click();await page.locator('[data-settings-tab=general]').click();assert.equal(await enabled.evaluate(el=>el.checked),false);
  await enabled.getByRole('switch').click();assert.equal(await enabled.evaluate(el=>el.checked),true);await enabled.getByRole('switch').click();assert.equal(await enabled.evaluate(el=>el.checked),false);await page.keyboard.press('Escape');
  // A different browser profile has the default false preference, even for the same account.
  const fresh=await browser.newContext();await fresh.addCookies(await page.context().cookies());const other=await fresh.newPage();await routeAssets(other);await other.goto(base);await other.locator('[data-agent-setup-notice]').waitFor({state:'visible'});assert.equal(await other.locator('[data-agent-setup-dismiss]').evaluate(el=>el.checked),false);await fresh.close();
  // False re-enables the notice; an unavailable storage leaves the switch off.
  await page.evaluate(()=>localStorage.setItem('rightcommit.hideAgentSetupNotice','false'));await page.reload();await page.locator('[data-agent-setup-notice]').waitFor({state:'visible'});
  await page.evaluate(()=>{window.originalStorageSetItem=Storage.prototype.setItem;Storage.prototype.setItem=function(key,value){if(key==='rightcommit.hideAgentSetupNotice')throw new Error('Storage unavailable');return window.originalStorageSetItem.call(this,key,value)}});
  await preference.getByRole('switch').click();assert.equal(await preference.evaluate(el=>el.checked),false);assert.equal(await page.evaluate(()=>localStorage.getItem('rightcommit.hideAgentSetupNotice')),'false');await page.evaluate(()=>{Storage.prototype.setItem=window.originalStorageSetItem});
  await page.locator('#dialog .modal-actions [data-action=close-dialog]').click();
  const settings=await (await page.request.get(base+'/api/settings')).json();
  const saved=await page.request.patch(base+'/api/settings',{headers:{'X-Nodo-Request':'1'},data:{revision:settings._revision,changes:{agents:{codex:{model:'configured-model',api_key:'dummy-setup-key'}}}}});assert(saved.ok(),await saved.text());
  await page.reload();await page.locator('#app').waitFor({state:'visible'});await page.waitForFunction(()=>app.settings.agents.codex.configured);assert(await page.locator('#dialog').isHidden());assert.deepEqual(errors,[]);
  console.log('PASS: startup notice only without configured agents, IT/EN, agent icon, direct Agents settings, dismiss, mobile layout, default false browser preference, immediate persistence, reload suppression, browser-profile isolation, Personal Settings restore toggle and storage failure');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
