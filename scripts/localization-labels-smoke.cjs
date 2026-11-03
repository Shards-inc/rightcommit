const assert=require('node:assert/strict'),path=require('node:path');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 try{
  const page=await browser.newPage({locale:'it-IT'});
  await page.route('http://labels.test/**',r=>r.fulfill({contentType:'text/html',body:'<html><body><select data-language><option>it</option><option>en</option></select><div id="probes"></div></body></html>'}));
  await page.goto('http://labels.test/');await page.addScriptTag({path:path.resolve('web/i18n.js')});
  const labels=[['Tariffe acquisite il:','Prices retrieved on:'],['Workflow principale','Parent workflow'],['Credenziali personali','Personal credentials'],['Bozza','Draft'],['Decisione umana','Human decision'],['Esecuzione in corso','Working on Run'],['Progettazione in corso','Working on Design'],['Mostra nascosti','Show hidden'],['Esci','Sign out'],['Deseleziona','Clear selection'],['Sposta','Move']];
  await page.evaluate(labels=>{for(const [it] of labels)for(const value of [' · '+it,it+' · ',' · '+it+' · ']){const p=document.createElement('p');p.textContent=value;document.querySelector('#probes').append(p)}},labels);
  const fs=require('node:fs');
  const workspace=fs.readFileSync('web/index.html','utf8').match(/<strong id="workspace-mode"[^>]*>.*?<\/strong>/)[0];
  await page.evaluate(markup=>{document.body.insertAdjacentHTML('beforeend',markup);document.querySelector('#workspace-mode').textContent='Esecuzione in corso · e03ac425'},workspace);
  await page.selectOption('[data-language]','it');
  const italian=await page.locator('#probes p').allTextContents();
  const english=labels.flatMap(([,en])=>[' · '+en,en+' · ',' · '+en+' · ']);
  // Mirror the actual history markup, including a user name that is also a UI label.
  const app=require('node:fs').readFileSync('web/app.js','utf8');
  await page.evaluate(source=>{window.app={runs:[{id:'child',workflow_id:'w',name:'Salva',parent:{run:'parent'},created:1793440800,status:'completed'}]};window.currentWorkflow=()=>({id:'w',name:'Salva'});window.$=s=>document.querySelector(s);window.esc=s=>String(s);window.icon=()=>'';window.states={completed:'Completato'};const host=document.createElement('div');host.id='run-history';document.body.append(host);(0,eval)(source);renderHistory()},app.slice(app.indexOf('function renderHistory()'),app.indexOf('async function showExecutions()')));
  for(let i=0;i<3;i++){
   await page.selectOption('[data-language]','en');assert.deepEqual(await page.locator('#probes p').allTextContents(),english);
   assert.equal(await page.locator('#workspace-mode').textContent(),'Working on Run · e03ac425');
   assert((await page.locator('#run-history').textContent()).includes('Parent workflow'));
   assert.equal(await page.locator('.history-row [data-no-i18n]').first().textContent(),'Salva');
   await page.selectOption('[data-language]','it');assert.deepEqual(await page.locator('#probes p').allTextContents(),italian);
   assert.equal(await page.locator('#workspace-mode').textContent(),'Esecuzione in corso · e03ac425');
   assert((await page.locator('#run-history').textContent()).includes('Workflow principale'));
  }
  // Text inserted in English mode must remain translated after the observer runs.
  await page.selectOption('[data-language]','en');await page.evaluate(()=>{const p=document.createElement('p');p.id='dynamic';p.textContent=' · '+uiText('Tariffe acquisite il:')+' ';document.body.append(p)});
  assert.equal(await page.locator('#dynamic').textContent(),' · Prices retrieved on: ');
  console.log('PASS: leading/trailing separators, dynamic insertion, repeated IT/EN changes, parent workflow label, preserved user names');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exit(1)});
