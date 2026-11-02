// Run against a disposable server; creates only isolated test account/workflows.
const assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const BASE=process.env.NODO_TEST_URL||'http://127.0.0.1:18089';
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1540,height:1050}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
  const call=async(path,body)=>{const r=await page.request.fetch(BASE+'/api'+path,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:body});const v=await r.json();assert.equal(r.status(),200,JSON.stringify(v));return v};
  const email=process.env.NODO_TEST_EMAIL||'a2a-ui@nodo.local';const registration=await page.request.post(BASE+'/api/auth/register',{headers:{'X-Nodo-Request':'1'},data:{email,password:'temporary-a2a-password-123'}});if(registration.status()!==200)await call('/auth/login',{email,password:'temporary-a2a-password-123'});
  await page.goto(BASE);await page.locator('#app').waitFor({state:'visible'});
  await page.locator('#a2a-publication').click();await page.locator('#a2a-publication-form').waitFor();
  await page.locator('#a2a-publication-form [name=enabled]').selectOption('true');
  await page.locator('#a2a-publication-form [name=outputs]').first().check();
  await page.locator('#a2a-publication-form [name=rotate]').selectOption('true');
  await page.locator('[data-action=a2a-publish]').click();
  await page.getByText('Token A2A creato',{exact:true}).waitFor();
  const token=await page.locator('#dialog textarea').inputValue();assert(token.length>50);
  const wid=await page.evaluate(()=>app.wf.id);const pub=await call('/a2a/workflows/'+wid);assert(pub.enabled);assert(!JSON.stringify(pub).includes(token));
  const card=await page.request.get(BASE+pub.path+'/.well-known/agent-card.json',{headers:{Authorization:'Bearer '+token}});assert.equal(card.status(),200);assert.equal((await card.json()).supportedInterfaces[0].protocolBinding,'JSONRPC');
  await page.locator('[data-action=close-dialog]').first().click();
  await page.locator('#palette-items [data-kind=A2A]').click();
  await page.locator('[data-action=a2a-connections]').click();
  await page.locator('#a2a-connection-form [name=name]').fill('Local A2A');
  await page.locator('#a2a-connection-form [name=url]').fill(BASE+pub.path+'/.well-known/agent-card.json');
  await page.locator('#a2a-connection-form [name=token]').fill(token);
  await page.locator('[data-action=a2a-save-connection]').click();
  await page.locator('#node-form [name=a2a_id] option').nth(1).waitFor({state:'attached'});
  const option=await page.locator('#node-form [name=a2a_id] option').nth(1).getAttribute('value');
  await page.locator('#node-form [name=a2a_id]').selectOption(option);
  await page.locator('[data-action=a2a-probe]').click();await page.waitForFunction(()=>document.querySelector('#toast').textContent.includes('Agente disponibile'));
  await page.locator('#save').click();await page.waitForFunction(()=>!app.dirty);
  const stored=(await call('/workflows')).find(w=>w.id===wid);assert.equal(stored.nodes.find(n=>n.kind==='A2A').a2a_id,option);
  await page.setViewportSize({width:900,height:800});await page.locator('#a2a-publication').click();await page.locator('#a2a-publication-form').waitFor();
  assert.equal(errors.length,0,errors.join('\n'));console.log('A2A UI smoke passed: publication, token, card, connection, probe, node persistence, narrow viewport.');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
