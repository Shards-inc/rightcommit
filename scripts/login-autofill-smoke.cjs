// Only disposable rightcommit accounts and dummy connector credentials are used.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const data=fs.mkdtempSync(path.join(os.tmpdir(),'login-autofill-')),base='http://127.0.0.1:18258';
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{cwd:data,env:{...process.env,NODO_DATA:data,NODO_LISTEN:'127.0.0.1:18258',NODO_ALLOW_REGISTRATION:'1'},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[],authPosts=[];page.on('pageerror',error=>errors.push(error.message));page.on('request',request=>{if(request.method()==='POST'&&/\/api\/auth\/(login|register)$/.test(request.url()))authPosts.push(request.postDataJSON())});
  for(const file of ['app.js','app.css','i18n.js','controls.js'])await page.route('**/'+file,route=>route.fulfill({path:path.resolve('web/'+file)}));await page.route(base+'/',route=>route.fulfill({path:path.resolve('web/index.html'),contentType:'text/html'}));
  await page.goto(base);await page.locator('#auth').waitFor({state:'visible'});assert((await page.locator('#auth-submit').textContent()).includes('Create administrator'));
  const email=page.locator('#auth-form [name=email]'),password=page.locator('#auth-form [name=password]');
  assert.equal(await page.locator('#auth-autofill').count(),0);
  assert.equal(await password.getAttribute('autocomplete'),'section-rightcommit-account new-password');
  assert.equal(await email.getAttribute('autocomplete'),'section-rightcommit-account username');
  assert.equal(await password.getAttribute('data-1p-ignore'),null);
  const account={email:'rightcommit-login@test.local',password:'temporary-rightcommit-password'};
  await email.fill(account.email);await password.fill(account.password);await page.locator('#auth-submit').click();await page.locator('#app').waitFor({state:'visible'});assert.deepEqual(authPosts[0],account);
  const logout=async()=>{await page.evaluate(()=>{if(document.querySelector('#dialog').open)closeModal()});await page.locator('#account').click();await page.locator('#account-menu [data-action=logout]').click();await page.locator('#auth').waitFor({state:'visible'})};
  await logout();assert.equal(await email.inputValue(),'');assert.equal(await password.inputValue(),'');
  assert.equal(await password.getAttribute('autocomplete'),'section-rightcommit-account current-password');
  // Saved credentials can be submitted directly, even without input events.
  await page.evaluate(account=>{const form=document.querySelector('#auth-form');form.elements.email.value=account.email;form.elements.password.value=account.password;form.requestSubmit()},account);
  await page.locator('#app').waitFor({state:'visible'});assert.equal(authPosts.length,2);assert.deepEqual(authPosts[1],account);
  await logout();await page.locator('#auth-switch').click();assert.equal(await password.getAttribute('autocomplete'),'section-rightcommit-account new-password');
  await page.locator('#auth-switch').click();assert.equal(await password.getAttribute('autocomplete'),'section-rightcommit-account current-password');
  await page.setViewportSize({width:390,height:844});assert(await page.locator('.auth-card').evaluate(el=>el.getBoundingClientRect().right<=innerWidth&&el.scrollWidth<=el.clientWidth));await page.screenshot({path:'/tmp/rightcommit-login-autofill-mobile.png'});
  await page.locator('#auth-switch').click();assert.equal(await page.locator('#auth-switch').textContent(),'Back to login');await page.screenshot({path:'/tmp/rightcommit-register-mobile.png'});
  await page.locator('#auth-switch').click();assert.equal(await page.locator('#auth-switch').textContent(),'Create an account');
  assert.deepEqual(errors,[]);
  console.log('PASS: standard autofill, manual registration, saved credential login without opt-in, logout clears secrets, registration mode and mobile');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(data,{recursive:true,force:true})}
})().catch(error=>{console.error(error);process.exit(1)});
