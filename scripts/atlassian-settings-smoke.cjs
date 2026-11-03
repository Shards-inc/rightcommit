// Disposable app + fake TWG: verify settings, draft tests and browser login without remote calls.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const root=path.resolve('.'),data=fs.mkdtempSync(path.join(os.tmpdir(),'atlassian-settings-')),base='http://127.0.0.1:18279';
 const bin=path.join(data,'system/tools/bin');fs.mkdirSync(bin,{recursive:true});fs.writeFileSync(path.join(bin,'twg'),`#!/bin/sh
if [ "$1" = login ]; then
 echo 'Visit: https://auth.atlassian.com/oauth/activate?user_code=TEST-1234'
 echo 'Open in browser?'
 read answer
 sleep 3
 exit 0
fi
if [ "$1" = user ]; then echo '{"accountId":"demo"}'; exit 0; fi
if [ "$1" = bitbucket ]; then
 [ "$TWG_BBC_TOKEN" = fixture-bb-token ] && [ "$TWG_BBC_USER" = demo@example.test ] && [ "$TWG_SITE" = demo.atlassian.net ] || exit 3
 echo '{"slug":"demo"}'; exit 0
fi
exit 2
`,{mode:0o755});
 const server=spawn(path.join(root,'target/debug/rightcommit'),[],{cwd:data,env:{...process.env,NODO_DATA:data,NODO_ASSETS:root,NODO_LISTEN:'127.0.0.1:18279'}});let browser,log='';server.stderr.on('data',b=>log+=b);
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}if(i===99)throw Error(log);await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const context=await browser.newContext();await context.addInitScript(()=>{localStorage.setItem('rightcommit.hideAgentSetupNotice','true');localStorage.setItem('nodo.language','en')});
  async function api(route,body){const r=await context.request.fetch(base+'/api'+route,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},...(body?{data:body}:{})});const v=await r.json();assert(r.ok(),JSON.stringify(v));return v}
  await api('/auth/register',{email:'atlassian-settings@test.local',password:'temporary-settings-test-password'});
  const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(base);await page.locator('#app').waitFor();await page.locator('#settings').click();await page.locator('#settings-tab-connections').click();
  const section=()=>page.locator('[data-service=atlassian]');assert.equal(await section().getByRole('heading',{name:'Atlassian',exact:true}).count(),1);
  await page.locator('[name=base_atlassian]').fill('https://demo.atlassian.net');await page.locator('[name=username_atlassian]').fill('demo@example.test');await page.locator('[name=token_atlassian]').fill('fixture-bb-token');await page.locator('[name=repo_atlassian]').fill('workspace/demo');
  await section().getByRole('button',{name:'Test connection',exact:true}).click();await page.locator('[data-atlassian-status]').filter({hasText:'Atlassian access: Connected · Bitbucket: Connected'}).waitFor();
  assert.equal((await api('/settings')).connections.atlassian,undefined,'Draft test must not save');
  await page.locator('[data-action=save-settings]').click();await page.locator('[data-action=save-settings]').waitFor({state:'visible'});await page.locator('[data-service=atlassian]').getByText('Bitbucket token saved',{exact:true}).waitFor();
  assert.equal(await page.locator('[name=token_atlassian]').inputValue(),'');const saved=await api('/settings');assert.equal(saved.connections.atlassian.token,'');assert.equal(saved.connections.atlassian.configured,true);assert(!JSON.stringify(saved).includes('fixture-bb-token'));
  await section().getByRole('button',{name:'Test connection',exact:true}).click();await page.locator('[data-atlassian-status]').filter({hasText:'Bitbucket: Connected'}).waitFor();
  await page.locator('[data-action=twg-login]').click();const link=page.locator('[data-atlassian-login-status] a');await link.waitFor();assert.equal(await link.getAttribute('href'),'https://auth.atlassian.com/oauth/activate?user_code=TEST-1234');await page.locator('[data-atlassian-login-status]').filter({hasText:'Atlassian account connected.'}).waitFor();
  await page.evaluate(()=>{localStorage.setItem('nodo.language','it');window.dispatchEvent(new StorageEvent('storage',{key:'nodo.language',newValue:'it'}))});await section().getByRole('button',{name:'Collega account Atlassian',exact:true}).waitFor();await section().getByRole('button',{name:'Verifica connessione',exact:true}).waitFor();
  await page.locator('[data-action=clear-key][data-field=token_atlassian]').click();await page.locator('[data-action=save-settings]').click();await page.locator('[data-service=atlassian]').getByText('Token Bitbucket non configurato',{exact:true}).waitFor();assert.equal((await api('/settings')).connections.atlassian.configured,false);
  assert.equal((await api('/runs')).length,0);assert.deepEqual(errors,[]);console.log('PASS: IT/EN Atlassian settings, draft/saved connection tests, hidden encrypted token, browser login and token removal; no external calls.');
 }finally{await browser?.close();if(server.exitCode===null){const stopped=new Promise(resolve=>server.once('exit',resolve));server.kill('SIGINT');await stopped;}fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
