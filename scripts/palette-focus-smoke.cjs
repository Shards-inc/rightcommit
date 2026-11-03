const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require('playwright');
(async()=>{
 const root=path.resolve(__dirname,'..'),data=fs.mkdtempSync(path.join(os.tmpdir(),'palette-focus-')),base='http://127.0.0.1:18279';
 const server=spawn(path.join(root,'target/debug/rightcommit'),[],{cwd:data,env:{...process.env,NODO_DATA:data,NODO_ASSETS:root,NODO_LISTEN:'127.0.0.1:18279'},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}if(i===99)throw Error('Test server unavailable');await new Promise(r=>setTimeout(r,100));}
  browser=await chromium.launch({channel:'chrome',headless:true});const context=await browser.newContext({viewport:{width:1440,height:1050}});
  await context.addInitScript(()=>localStorage.setItem('rightcommit.hideAgentSetupNotice','true'));
  // Serve current workspace assets, since the executable embeds assets at build time.
  await context.route(/\/(?:[^/]+\.(?:js|css)|index\.html)(?:\?.*)?$/,async route=>{
   const file=path.join(root,'web',path.basename(new URL(route.request().url()).pathname));
   if(fs.existsSync(file))await route.fulfill({path:file});else await route.continue();
  });
  async function api(route,body){const r=await context.request.post(base+'/api'+route,{headers:{'X-Nodo-Request':'1'},data:body});assert(r.ok(),await r.text());return r.json()}
  await api('/auth/register',{email:'palette-focus@test.local',password:'temporary-palette-focus-password'});
  const workflow={id:'palette-focus-test',name:'Palette focus test',description:'Disposable focus verification',entry:'',nodes:[],edges:[]};await api('/workflows',workflow);
  const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(base);await page.locator('#app').waitFor();
  await page.evaluate(id=>selectWorkflow(id),workflow.id);
  await page.waitForFunction(id=>app.wf?.id===id,workflow.id);
  const task=page.locator('#palette-items [data-kind="Task"]'),gate=page.locator('#palette-items [data-kind="Gate"]');
  const ring=el=>({focused:document.activeElement===el,visible:el.matches(':focus-visible'),outline:getComputedStyle(el).outlineStyle,width:getComputedStyle(el).outlineWidth});
  await task.click();await page.waitForFunction(()=>app.wf.nodes.length===1);const added=await page.evaluate(()=>app.selected);
  await page.keyboard.press('Backspace');await page.waitForFunction(id=>!app.wf.nodes.some(n=>n.id===id),added);
  await gate.click();await page.waitForFunction(()=>app.wf.nodes.length===1&&app.wf.nodes[0].kind==='Gate');
  const mouse=await gate.evaluate(ring);assert.equal(mouse.focused,false);assert.equal(mouse.visible,false);
  await page.screenshot({path:'/tmp/palette-focus-mouse.png'});
  // Reach the palette using real Tab presses, without programmatic focus.
  let reached=false;for(let i=0;i<100;i++){await page.keyboard.press('Tab');if(await page.evaluate(()=>document.activeElement?.matches('#palette-items button'))){reached=true;break}}
  assert(reached,'Tab must reach a palette button');const focused=page.locator('#palette-items button:focus');const keyboard=await focused.evaluate(ring);
  assert.equal(keyboard.visible,true);assert.equal(keyboard.outline,'solid');assert.equal(keyboard.width,'3px');
  await page.screenshot({path:'/tmp/palette-focus-keyboard.png'});
  await page.keyboard.press('Enter');await page.waitForFunction(()=>app.wf.nodes.length===2);assert.equal(await focused.evaluate(el=>el.matches(':focus-visible')),true);
  assert.deepEqual(errors,[]);console.log(JSON.stringify({pass:true,mouse,keyboard,checks:['add Task → Backspace deletes Task → click Gate adds Gate without focus ring','Tab reaches palette with visible 3px outline','Enter adds node and preserves keyboard focus'],screenshots:['/tmp/palette-focus-mouse.png','/tmp/palette-focus-keyboard.png']}));
 }finally{await browser?.close();if(server.exitCode===null){const stopped=new Promise(resolve=>server.once('exit',resolve));server.kill('SIGINT');await stopped}fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
