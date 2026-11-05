const fs=require('node:fs'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1100,height:900}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.setContent(fs.readFileSync('web/icons.svg','utf8').replace('<svg ', '<svg style="display:none" ')+ '<section id="artifacts-panel" style="height:700px;width:370px"></section><dialog id="dialog"><div id="dialog-content"></div></dialog>');
  await page.addStyleTag({content:fs.readFileSync('web/app.css','utf8')});
  await page.evaluate(source=>{
   window.$=(s,r=document)=>r.querySelector(s);window.$$=(s,r=document)=>[...r.querySelectorAll(s)];window.uiText=s=>({'Filtra':'Filter','Filtra artefatti':'Filter artifacts'}[s]||s);window.nodoLocale=()=> 'en-US';
   window.esc=s=>String(s).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;').replaceAll('>','&gt;');window.icon=name=>`<svg class="icon" aria-hidden="true"><use href="#${name}"></use></svg>`;
   window.modal=(title,body,actions)=>{const dialog=$('#dialog');$('#dialog-content').innerHTML=`<div class="modal-head"><h2>${uiText(title)}</h2></div><div class="modal-body">${body}</div><div class="modal-actions">${actions}</div>`;if(!dialog.open)dialog.showModal();$('[data-action=close-dialog]').onclick=()=>dialog.close()};
   window.app={user:{id:1},run:{id:'run-1'},artifacts:[]};
   for(let i=0;i<1100;i++)app.artifacts.push({id:'file-'+i,path:'output/nested/file-'+i+'.txt',version:1,created:i+1,file:{root:'/data',location:'/data/output/nested/file-'+i+'.txt'}});
   for(const [id,path,root] of [['late','z-after1000/deep/result.txt','/data'],['sibling','output2/file.txt','/data'],['root','root.txt','/data'],['secondary','output/file.txt','/sources'],['hidden','.hidden/secret.txt','/data'],['report','.rightcommit/reports/task.md','/data'],['escaped','a<&"/result.txt','/data']])app.artifacts.push({id,path,version:1,created:1,file:{root,location:root+'/'+path,change:id==='report'?'report':'created'}});
   app.artifacts.push({...app.artifacts[0],id:'old-version',version:0});
   eval(source.slice(source.indexOf('const artifactSearch='),source.indexOf('function renderInspector('))+'\nwindow.draw=()=>{$("#artifacts-panel").innerHTML=inspectorArtifacts(app.artifacts);bindArtifactSearch();refreshArtifactFolderDialog()};');draw();
  },fs.readFileSync('web/app.js','utf8'));
  const visible=()=>page.locator('[data-inspector-artifacts]>.artifact-item:visible').count();
  assert.equal(await visible(),1105);
  assert((await page.locator('[data-artifact-filter]').boundingBox()).x<(await page.locator('[data-artifact-show-hidden]').boundingBox()).x);
  await page.locator('[data-artifact-query]').fill('no matches');await page.locator('[data-artifact-filter]').click();
  assert.equal(await page.locator('[data-artifact-folder="/data/z-after1000/deep"]').count(),1);
  assert.equal(await page.locator('[data-artifact-folder="/data/.hidden"]').count(),1);
  assert((await page.locator("[data-artifact-folder]").evaluateAll(rows=>rows.map(row=>row.dataset.artifactFolder))).includes('/data/a<&"'));
  assert.equal(await page.locator('[data-artifact-folder]:visible').count(),2);
  assert.equal(await page.locator('[data-artifact-expand="/data"]').getAttribute('aria-expanded'),'false');
  await page.locator('[data-artifact-expand="/data"]').click();
  assert(await page.locator('[data-artifact-folder="/data/output"]').isVisible());
  await page.locator('[data-artifact-expand="/data"]').press('Enter');
  assert.equal(await page.locator('[data-artifact-folder="/data/output"]').isVisible(),false);
  await page.locator('[data-artifact-expand="/data"]').press('Enter');
  await page.locator('[data-artifact-folder="/data/output"]').click();
  assert.equal(await page.locator('[data-artifact-folder="/data/output/nested"]').getAttribute('aria-checked'),'false');assert(await page.locator('[data-artifact-folder="/data/output/nested"]').isDisabled());
  await page.locator('[data-action=close-dialog]').click();await page.locator('[data-artifact-query]').fill('');assert.equal(await visible(),5);
  assert.equal(await page.locator('[data-id=sibling]').isVisible(),true);assert.equal(await page.locator('[data-id=secondary]').isVisible(),true);
  await page.locator('[data-artifact-show-hidden]').click();assert.equal(await visible(),6);
  await page.locator('[data-artifact-filter]').click();assert.equal(await page.locator('[data-artifact-folder="/data/output"]').getAttribute('aria-checked'),'false');
  assert.equal(await page.locator('[data-artifact-expand="/data"]').getAttribute('aria-expanded'),'true');
  await page.locator('[data-artifact-folder="/data/output"]').click();await page.locator('[data-artifact-expand="/data/output"]').click();await page.locator('[data-artifact-folder="/data/output/nested"]').click();
  await page.locator('[data-artifact-folder="/data"]').click();assert(await page.locator('[data-artifact-folder="/data/output"]').isDisabled());
  await page.locator('[data-action=close-dialog]').click();assert.equal(await visible(),1);assert(await page.locator('[data-id=secondary]').isVisible());
  await page.evaluate(()=>draw());assert.equal(await visible(),1);
  await page.locator('[data-artifact-filter]').click();assert.equal(await page.locator('[data-artifact-expand="/data/output"]').getAttribute('aria-expanded'),'true');await page.locator('[data-artifact-folder="/data"]').click();await page.locator('[data-action=close-dialog]').click();assert.equal(await visible(),1106);
  await page.evaluate(()=>{app.run.id='other-run';draw()});assert.equal(await visible(),1106);assert.equal(await page.locator('[data-artifact-filter]').getAttribute('aria-pressed'),'false');
  await page.setViewportSize({width:390,height:844});await page.locator('[data-artifact-filter]').click();assert(await page.locator('#dialog').evaluate(el=>el.scrollWidth<=el.clientWidth));assert.equal(await page.locator('[data-artifact-folder]:visible').count(),2);await page.screenshot({path:'/tmp/rightcommit-artifact-folder-filter.png'});
  await page.evaluate(()=>{const select=document.createElement('select');select.dataset.language='';select.innerHTML='<option value="en">English</option><option value="it">Italiano</option>';document.body.append(select)});
  await page.addScriptTag({content:fs.readFileSync('web/i18n.js','utf8')});
  for(const lang of ['en','it','en']){
   await page.locator('[data-language]').selectOption(lang);
   assert.equal(await page.locator('[data-artifact-filter]').innerText(),lang==='en'?'Filter':'Filtra');
   assert.equal(await page.locator('#dialog h2').innerText(),lang==='en'?'Filter artifacts':'Filtra artefatti');
   assert.equal(await page.locator('[data-action=close-dialog]').innerText(),lang==='en'?'Close':'Chiudi');
   assert.equal(await page.locator('#artifact-hidden-label').innerText(),lang==='en'?'Show hidden':'Mostra nascosti');
   assert((await page.locator('#dialog .note').innerText()).startsWith(lang==='en'?'Choose which folders':'Scegli le cartelle'));
   assert.equal(await page.locator('[data-artifact-folder="/data"]').getAttribute('aria-label'),(lang==='en'?'Show folder':'Mostra cartella')+' /data');
   assert.equal(await page.locator('[data-artifact-expand="/data"]').getAttribute('aria-label'),(lang==='en'?'Expand folder':'Espandi cartella')+' /data');
   await page.locator('[data-artifact-expand="/data"]').click();
   assert.equal(await page.locator('[data-artifact-expand="/data"]').getAttribute('aria-label'),(lang==='en'?'Collapse folder':'Comprimi cartella')+' /data');
   await page.locator('[data-artifact-expand="/data"]').click();
   assert.equal(await page.locator('[data-artifact-folder="/data/output"]').getAttribute('data-artifact-folder'),'/data/output');
   await page.screenshot({path:'/tmp/rightcommit-artifact-folder-filter-'+lang+'.png'});
  }
  await page.locator('[data-action=close-dialog]').click();
  await page.evaluate(()=>{
   for(const [id,path] of [['debug','target/debug/deps/build.bin'],['release','target/release/app'],['debugger','target/debugger/result.txt'],['nested-debug','project/target/debug/app']])app.artifacts.push({id,path,version:1,created:1,file:{root:'/data',location:'/data/'+path}});
   draw();
  });
  assert.equal(await page.locator('[data-id=debug]').isVisible(),false);assert.equal(await page.locator('[data-id=nested-debug]').isVisible(),false);
  assert(await page.locator('[data-id=release]').isVisible());assert(await page.locator('[data-id=debugger]').isVisible());
  await page.locator('[data-artifact-filter]').click();await page.locator('[data-artifact-expand="/data"]').click();await page.locator('[data-artifact-expand="/data/target"]').click();
  assert.equal(await page.locator('[data-artifact-folder="/data/target/debug"]').getAttribute('aria-checked'),'false');
  await page.locator('[data-artifact-folder="/data/target/debug"]').click();await page.locator('[data-action=close-dialog]').click();assert(await page.locator('[data-id=debug]').isVisible());
  await page.evaluate(()=>draw());assert(await page.locator('[data-id=debug]').isVisible());
  await page.evaluate(()=>{app.run.id='next-workflow-run';draw()});assert.equal(await page.locator('[data-id=debug]').isVisible(),false);
  assert(await page.locator('[data-id=release]').isVisible());
  assert.deepEqual(errors,[]);console.log('PASS: all 1100+ files, full folder tree independent of search/hidden, nested switches, subtree filtering, path boundaries, source isolation, rerenders, run reset, escaping, collapsed defaults, keyboard expansion, automatic ancestors of modified folders, mobile, live English/Italian localization and target/debug defaults with manual override and run reset');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exit(1)});
