const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require('playwright');
(async()=>{
 const root=path.resolve(__dirname,'..'),data=fs.mkdtempSync(path.join(os.tmpdir(),'activity-message-')),base='http://127.0.0.1:18280';
 const server=spawn(path.join(root,'target/debug/rightcommit'),[],{cwd:data,env:{...process.env,NODO_DATA:data,NODO_ASSETS:root,NODO_LISTEN:'127.0.0.1:18280'},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}if(i===99)throw Error('Test server unavailable');await new Promise(r=>setTimeout(r,100));}
  browser=await chromium.launch({channel:'chrome',headless:true});const context=await browser.newContext({viewport:{width:1440,height:1050}});
  await context.grantPermissions(['clipboard-read','clipboard-write']);
  await context.addInitScript(()=>localStorage.setItem('rightcommit.hideAgentSetupNotice','true'));
  // Serve current workspace assets, since the executable embeds assets at build time.
  await context.route(/\/(?:[^/]+\.(?:js|css)|index\.html)(?:\?.*)?$/,async route=>{
   const file=path.join(root,'web',path.basename(new URL(route.request().url()).pathname));
   if(fs.existsSync(file))await route.fulfill({path:file});else await route.continue();
  });
  async function api(route,body){const r=await context.request.post(base+'/api'+route,{headers:{'X-Nodo-Request':'1'},data:body});assert(r.ok(),await r.text());return r.json()}
  await api('/auth/register',{email:'activity-message@test.local',password:'temporary-activity-message-password'});
  const workflow={id:'activity-message-test',name:'Palette focus test',description:'Disposable focus verification',entry:'',nodes:[],edges:[]};await api('/workflows',workflow);
  const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(base);await page.locator('#app').waitFor();
  await page.evaluate(id=>selectWorkflow(id),workflow.id);
  await page.waitForFunction(id=>app.wf?.id===id,workflow.id);
  await page.evaluate(()=>{
   activityLog.workflow=app.wf.id;const meta={id:'message-fixture',name:'Messaggi di prova',workflow_id:app.wf.id,status:'completed',created:1700000000};app.runs=[meta];const entry=logRun(meta);entry.loaded=true;
   const long='Prima riga\n'+('Contenuto molto lungo e leggibile. '.repeat(35))+'\nUltima riga <script>window.injected=true</script>';
   for(const event of [{seq:1,kind:'output',text:long},{seq:2,kind:'activity',text:JSON.stringify({phase:'end',runtime:'claude',tool:'Bash',detail:long,body:{nested:'CORPO COMPLETO'}})},{seq:3,kind:'state',text:'completed'},{seq:4,kind:'output',text:'<nodo_result>'+JSON.stringify({summary:'Riassunto',details:long,artifacts:[{name:'report'}]})+'</nodo_result>'}])entry.events.set(event.seq,{...event,created:1700000000,node:'task',run:meta.id});
   activityLog.selected=meta.id;document.querySelector('#logs').hidden=false;document.body.classList.add('has-logs');renderActivityLog();
  });
  assert.equal(await page.locator('[data-log-read]').count(),3);
  assert.equal(await page.locator('[data-log-seq="3"] [data-log-read]').count(),0);
  assert((await page.locator('[data-log-seq="1"] .activity-event-preview pre').textContent()).endsWith('…'));
  assert((await page.locator('[data-log-seq="1"] .activity-event-preview pre').textContent()).length<=241);
  for(const language of ['it','en']){
   await page.evaluate(language=>{window.dispatchEvent(new StorageEvent('storage',{key:'nodo.language',newValue:language}));renderActivityLog()},language);
   const read=page.locator('[data-log-read="2"]');assert.equal(await read.textContent(),language==='it'?'Leggi':'Read');await read.click();
   assert(await page.locator('#dialog').isVisible());assert.equal(await page.locator('.activity-message-dialog section').count(),2);
   const message=await page.locator('.activity-message-dialog section').first().locator('pre').textContent();assert(message.includes('Prima riga\n'));assert(message.includes('Ultima riga'));
   const body=await page.locator('.activity-message-dialog section').nth(1).locator('pre').textContent();assert(body.includes('CORPO COMPLETO'));assert(body.includes('\n  "phase"'));
   for(const [key,text] of [['message',message],['body',body]]){
    const copy=page.locator(`[data-log-copy="${key}"]`);assert.equal(await copy.textContent(),language==='it'?'Copia':'Copy');await copy.click();
    await page.waitForFunction(text=>navigator.clipboard.readText().then(value=>value===text),text);
    assert.equal(await copy.textContent(),language==='it'?'Copiato':'Copied');
   }
   assert.equal(await page.evaluate(()=>window.injected),undefined);assert.equal(await page.locator('.activity-message-dialog script').count(),0);
   await page.screenshot({path:'/tmp/activity-message-'+language+'.png'});await page.keyboard.press('Escape');assert(!(await page.locator('#dialog').isVisible()));
  }
  await page.locator('[data-log-read="1"]').click();assert.equal(await page.locator('.activity-message-dialog section').count(),1);await page.keyboard.press('Escape');
  await page.setViewportSize({width:390,height:844});await page.locator('[data-log-read="2"]').click();assert(await page.locator('.activity-message-dialog').evaluate(el=>el.scrollWidth<=el.clientWidth));await page.screenshot({path:'/tmp/activity-message-mobile.png'});
  assert.deepEqual(errors,[]);console.log('PASS: truncated previews, full multiline messages, complete formatted bodies, IT/EN labels, safe text, Escape and mobile dialog');
 }finally{await browser?.close();if(server.exitCode===null){const stopped=new Promise(resolve=>server.once('exit',resolve));server.kill('SIGINT');await stopped}fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
