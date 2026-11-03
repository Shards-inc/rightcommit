// Real Pi CLI + disposable OpenAI-compatible server; no model downloads or paid calls.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),http=require('node:http'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const cli=process.env.PI_CLI;assert(cli&&fs.existsSync(cli),'Set PI_CLI to the installed Pi CLI entry point');
 const data=fs.mkdtempSync(path.join(os.tmpdir(),'pi-local-')),base='http://127.0.0.1:18255',requests=[];
 const mock=http.createServer(async(req,res)=>{
  let body='';for await(const chunk of req)body+=chunk;
  const input=JSON.parse(body);requests.push({path:req.url,auth:req.headers.authorization,model:input.model});
  assert(req.url.endsWith('/v1/chat/completions'));assert.equal(input.model,'local-test-model');assert(!input.tools?.length);
  res.writeHead(200,{'Content-Type':'text/event-stream'});
  const event=(delta,finish_reason)=>({id:'chatcmpl-local-test',object:'chat.completion.chunk',created:1,model:input.model,choices:[{index:0,delta,finish_reason}]});
  res.end('data: '+JSON.stringify(event({role:'assistant',content:'OK'},null))+'\n\ndata: '+JSON.stringify(event({},'stop'))+'\n\ndata: [DONE]\n\n');
 });await new Promise(resolve=>mock.listen(0,'127.0.0.1',resolve));const endpoint='http://127.0.0.1:'+mock.address().port;
 const binary=path.join(data,'system/tools/bin/pi');fs.mkdirSync(path.dirname(binary),{recursive:true});
 const quote=s=>"'"+s.replaceAll("'","'\\''")+"'";
 fs.writeFileSync(binary,'#!/bin/sh\nexec '+quote(process.execPath)+' '+quote(cli)+' "$@"\n',{mode:0o700});
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:data,NODO_LISTEN:'127.0.0.1:18255'},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  for(const file of ['app.js','app.css','i18n.js','controls.js'])await page.route('**/'+file,r=>r.fulfill({path:path.resolve('web/'+file)}));
  await page.request.post(base+'/api/auth/register',{headers:{'X-Nodo-Request':'1'},data:{email:'local@test.local',password:'temporary-local-password'}});
  const read=async()=>await(await page.request.get(base+'/api/settings')).json();let settings=await read();
  const seeded=await page.request.patch(base+'/api/settings',{headers:{'X-Nodo-Request':'1'},data:{revision:settings._revision,changes:{agents:{pi:{provider:'openai',model:'cloud-model',api_key:'cloud-only-key'}}}}});assert(seeded.ok(),await seeded.text());
  const original=await read();await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>settingsDialog('agents'));
  const provider=page.locator('[name=provider_pi]'),url=page.locator('[name=base_url_pi]'),button=page.locator('[data-action=pi-check]'),status=page.locator('[data-pi-status]');
  assert(await url.isHidden());await provider.getByRole('combobox').click();await provider.getByRole('option',{name:'Local / OpenAI compatible',exact:true}).click();assert(await url.isVisible());
  await url.fill(endpoint+'/v1');await page.locator('[name=model_pi]').fill('local-test-model');
  const check=async()=>{await button.click();await page.waitForFunction(()=>!document.querySelector('[data-action=pi-check]').disabled);assert((await status.textContent()).includes('Pi connection successful'),await status.textContent())};
  await check();assert.equal(requests.at(-1).auth,'Bearer local');assert.equal(requests.at(-1).path,'/v1/chat/completions');assert.deepEqual(await read(),original);
  await page.locator('[name=key_pi]').fill('local-private-key');await check();assert.equal(requests.at(-1).auth,'Bearer local-private-key');assert.deepEqual(await read(),original);
  await page.locator('[data-action=save-settings]').click();await page.waitForFunction(()=>app.settings.agents.pi.provider==='local');settings=await read();assert(settings.agents.pi.configured);assert.equal(settings.agents.pi.base_url,endpoint+'/v1');assert.equal(settings.agents.pi.api_key,'');
  await check();assert.equal(requests.at(-1).auth,'Bearer local-private-key');
  await url.fill(endpoint+'/other/v1');await check();assert.equal(requests.at(-1).auth,'Bearer local');
  await page.locator('[data-action=save-settings]').click();await page.waitForFunction(()=>app.settings.agents.pi.base_url.endsWith('/other/v1'));settings=await read();assert(settings.agents.pi.configured);assert.equal(settings.agents.pi.api_key_preview,null);
  const invalid=await page.request.post(base+'/api/agents/pi/status',{headers:{'X-Nodo-Request':'1'},data:{provider:'local',model:'local-test-model',base_url:'file:///tmp/model'}});assert.equal(invalid.status(),400);
  await page.evaluate(()=>{const language=document.querySelector('[data-language]');language.value='it';language.dispatchEvent(new Event('change',{bubbles:true}))});assert.equal(await provider.locator('.caption').textContent(),'Locale / OpenAI compatibile');
  await page.setViewportSize({width:390,height:844});assert(await page.locator('#settings-form').evaluate(el=>el.scrollWidth<=el.clientWidth));await page.screenshot({path:'/tmp/pi-local-settings.png'});assert.deepEqual(errors,[]);
  console.log('PASS: real Pi 1.4 with local SSE server, draft URL/model/key, keyless endpoint, no cloud key reuse, endpoint-scoped credentials, persistence, IT/EN and invalid URL validation');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));await new Promise(r=>mock.close(r));fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
