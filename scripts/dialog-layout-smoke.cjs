const fs=require('node:fs'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1000,height:900}});
  await page.setContent('<div id="nodes"><div class="node" data-id="task" style="left:400px;top:200px"></div></div><dialog id="dialog"><div id="dialog-content"></div></dialog>');
  await page.addStyleTag({content:fs.readFileSync('web/app.css','utf8')});
  await page.evaluate(source=>{
   window.$=(s,r=document)=>r.querySelector(s);window.$$=(s,r=document)=>[...r.querySelectorAll(s)];
   window.closeAccountMenu=window.closeNodeMenu=window.hideTooltip=window.submitSingleTextDialog=()=>{};
   window.esc=s=>String(s).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
   window.icon=()=>'';window.select=(label,name,value)=>`<select name="${name}"><option value="${value}">${value}</option></select>`;window.input=()=>'';window.toast=message=>{throw Error(message)};
   window.api=async()=>({indices:[],languages:[]});window.openStorage=(path,root)=>window.returned={path,root};
   window.modal=eval('('+source.slice(source.indexOf('function modal('),source.indexOf('function appConfirm('))+')');
   window.app={run:{id:'run',status:'running'}};window.state=()=> 'running';window.activityData=e=>e.data;
   window.events=new Map();window.activityLog={runs:new Map([['run',{events:window.events}]])};
   eval(source.slice(source.indexOf('const nodeToolLabels='),source.indexOf('function renderGraph('))+'\nwindow.syncLabels=syncNodeToolLabels;');
   window.showTool=(name,seq)=>{window.events.set(seq,{seq,kind:'activity',node:'task',data:{phase:'start',tool:name}});window.syncLabels()};
  },fs.readFileSync('web/app.js','utf8'));
  await page.evaluate(()=>showTool('Bash',1));
  const short=await page.locator('.node-tool-label').evaluate(el=>({width:el.offsetWidth,color:getComputedStyle(el).backgroundColor}));
  assert.equal(short.width,88);assert.equal(short.color,'rgb(224, 242, 231)');
  const long='mcp__filesystem__search_project_documents_and_references';
  await page.evaluate(name=>showTool(name,2),long);await page.waitForTimeout(300);
  assert.equal(await page.locator('.node-tool-label').evaluate(el=>el.offsetWidth),140);
  assert.equal(await page.locator('.node-tool-label').getAttribute('data-tip'),long);
  assert(await page.locator('.node-tool-label>span').evaluate(el=>el.scrollWidth>el.clientWidth));
  await page.emulateMedia({colorScheme:'dark'});
  assert.equal(await page.locator('.node-tool-label').evaluate(el=>getComputedStyle(el).backgroundColor),'rgb(35, 61, 48)');
  await page.emulateMedia({colorScheme:'light'});
  await page.evaluate(()=>modal('Example','', '<button>Cancel</button><button class="primary">Save</button>'));
  assert.equal(await page.locator('.modal-actions').evaluate(el=>getComputedStyle(el).justifyContent),'flex-end');
  await page.addScriptTag({content:fs.readFileSync('web/rag.js','utf8')});
  // Use keyword mode to verify navigation without starting indexing or downloading models.
  await page.evaluate(()=>openRag('examples','nested/folder'));
  assert.equal(await page.locator('#rag-storage-back').innerText(),'Indietro');
  let back=await page.locator('#rag-storage-back').boundingBox(),start=await page.locator('#rag-start').boundingBox();assert(back.x<start.x);
  await page.locator('#rag-storage-back').click();
  assert.deepEqual(await page.evaluate(()=>window.returned),{path:'nested/folder',root:'examples'});
  for(const width of [390,1000]){
   await page.setViewportSize({width,height:900});
   await page.evaluate(()=>openRag('sources',''));
   assert(await page.locator('#rag-storage-back').isVisible());assert(await page.locator('#rag-start').isVisible());
   assert(await page.locator('#dialog').evaluate(el=>el.scrollWidth<=el.clientWidth));
  }
  await page.evaluate(()=>{
   document.documentElement.lang='en';
   window.job={id:'index',root:'examples',path:'folder',config:{retrieval:'keyword'},progress:{state:'completed',phase:'completed',total:1,done:1,files_done:1,files_total:1,chunks:1,started:1,updated:2,message:'Done'}};
   ragProgress(window.job,'examples','folder');
  });
  assert.equal(await page.locator('#rag-storage-back').innerText(),'Back');
  assert(await page.locator('#rag-progress-back').isVisible());
  await page.locator('#rag-storage-back').click();assert.deepEqual(await page.evaluate(()=>window.returned),{path:'folder',root:'examples'});
  console.log('PASS: adaptive tool labels, full names, light/dark, right-aligned footers, RAG back navigation, progress footer, mobile and English');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exit(1)});
