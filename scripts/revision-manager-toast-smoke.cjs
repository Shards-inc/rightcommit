const fs=require('node:fs'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 try{
  const page=await browser.newPage();
  await page.setContent('<dialog id="dialog">Open dialog</dialog><div id="toast" popover="manual" role="status" hidden></div>');
  await page.addStyleTag({content:fs.readFileSync('web/app.css','utf8')});
  const appSource=fs.readFileSync('web/app.js','utf8'),managerSource=fs.readFileSync('web/manager.js','utf8');
  await page.evaluate(({appSource,managerSource})=>{
   window.$=s=>document.querySelector(s);
   eval(appSource.slice(appSource.indexOf('let toastTimer;'),appSource.indexOf('\n',appSource.indexOf('let toastTimer;')))+';window.notify=toast;');
   $('#dialog').showModal();notify('Visible above dialog');
   window.app={editBase:{id:'old-run'},wf:{nodes:[{id:'source',kind:'Source',source_type:'folder',source_mode:'direct',location:'original'}]}};
   window.dirty=window.render=window.fit=window.renderChat=()=>{};window.uiText=s=>s;
   eval(managerSource.slice(managerSource.indexOf('function applyWorkflow('),managerSource.indexOf(' async function applyPending('))+';window.applyCorrection=applyWorkflow;');
  },{appSource,managerSource});
  assert(await page.locator('#toast').evaluate(el=>el.matches(':popover-open')));
  await page.screenshot({path:'/tmp/revision-toast-modal.png'});
  assert(await page.locator('#toast').isVisible());
  await page.evaluate(()=>{notify('Second error');applyCorrection({nodes:[{id:'source',kind:'Source',source_type:'example',source_mode:'copy',location:'wrong'},{id:'task',kind:'Task'}]},{});});
  const source=await page.evaluate(()=>app.wf.nodes[0]);assert.equal(source.location,'original');assert.equal(source.source_mode,'direct');assert.equal(source.source_type,'folder');
  assert(await page.evaluate(()=>{try{applyCorrection({nodes:[]},{});return false}catch{return true}}));
  console.log('PASS: toast above modal, repeated toast, manager preserves revision Sources and rejects source replacement');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exit(1)});
