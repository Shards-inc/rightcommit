const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),assert=require('node:assert/strict'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'run-artifacts-')),base='http://127.0.0.1:18162',bin=path.join(dir,'bin'),sources=path.join(dir,'sources');fs.mkdirSync(bin);
 for(const folder of ['main','second','copied']){fs.mkdirSync(path.join(sources,folder),{recursive:true});fs.writeFileSync(path.join(sources,folder,'original.txt'),'original');fs.writeFileSync(path.join(sources,folder,'unchanged.txt'),'untouched');}
 const second=path.join(sources,'second');
 fs.writeFileSync(path.join(bin,'claude'),`#!/usr/bin/env python3
import os,sys,json,pathlib,re
prompt=sys.stdin.read();args=sys.argv
sid=args[args.index('--session-id')+1] if '--session-id' in args else args[args.index('--resume')+1]
second_turn='SECOND TURN' in prompt
cwd=pathlib.Path.cwd()
(cwd/'original.txt').write_text('second version' if second_turn else 'first version')
(cwd/'.hidden').mkdir(exist_ok=True)
(cwd/'.hidden/result.md').write_text('# Hidden output')
(cwd/'binary.dat').write_bytes(b'\\0\\xffsecond' if second_turn else b'\\0\\xfffirst')
(cwd/'large.bin').write_bytes(b'x'*(1024*1024+1))
pathlib.Path(${JSON.stringify(second)}).joinpath('result.txt').write_text('secondary output')
paths=re.findall(r'"workspace_path"\\s*:\\s*"([^"\\n]+)"',prompt)
for raw in paths:
 p=pathlib.Path(raw)
 if 'copied' in p.parts:
  if not p.is_absolute():p=cwd/p
  if p.is_dir():(p/'copied-output.txt').write_text('copied output')
def emit(x):print(json.dumps(x),flush=True)
emit(dict(type='system',subtype='init',session_id=sid))
emit(dict(type='result',result='<nodo_result>'+json.dumps(dict(status='completed',summary='Fixture output',artifacts=[],questions=[]))+'</nodo_result>',is_error=False))
`,{mode:0o755});
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18162',NODO_SOURCES:sources,PATH:bin+':'+process.env.PATH,ANTHROPIC_AUTH_TOKEN:'offline-artifact-fixture'},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1050}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  for(const file of ['app.js','app.css','i18n.js'])await page.route('**/'+file,r=>r.fulfill({path:path.resolve('web/'+file)}));
  const api=async(p,b)=>{const r=await page.request.fetch(base+'/api'+p,{method:b?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:b});assert(r.ok(),await r.text());return r.json()};
  await api('/auth/register',{email:'artifacts@test.local',password:'temporary-artifacts-password'});
  for(const mode of ['direct','copy']){
   for(const name of ['binary.dat','large.bin','.hidden'])fs.rmSync(path.join(sources,'main',name),{recursive:true,force:true});
   fs.writeFileSync(path.join(sources,'main','original.txt'),'original');fs.rmSync(path.join(second,'result.txt'),{force:true});
   const wf={id:'artifacts-'+mode,name:'Artifacts '+mode,description:'Offline artifact checks',entry:'first',nodes:[
    {id:'source',kind:'Source',title:'Primary',source_type:'folder',source_mode:mode,location:'main'},
    {id:'second',kind:'Source',title:'Second',source_type:'folder',source_mode:'direct',location:'second'},
    {id:'copied',kind:'Source',title:'Copied',source_type:'folder',source_mode:'copy',location:'copied'},
    {id:'agent',kind:'Agent',title:'Claude fixture',runtime:'claude'},
    {id:'first',kind:'Task',title:'First task',agent:'agent',prompt:'FIRST TURN'},
    {id:'last',kind:'Task',title:'Second task',agent:'agent',prompt:'SECOND TURN'}],edges:[
    {from:'source',to:'agent',kind:'context'},{from:'second',to:'agent',kind:'context'},{from:'copied',to:'agent',kind:'context'},
    {from:'agent',to:'first',kind:'context'},{from:'agent',to:'last',kind:'context'},{from:'first',to:'last',kind:'control'}]};
   wf.nodes.forEach((n,i)=>{n.x=80+i*145;n.y=120});
   await api('/workflows',wf);const run=await api('/runs',{workflow_id:wf.id});let data;
   for(let i=0;i<300;i++){data=await api('/runs/'+run.id);assert.notEqual(data.run.status,'failed',data.run.error);if(data.run.status==='completed')break;await new Promise(r=>setTimeout(r,50))}
   assert.equal(data.run.status,'completed');
   const files=data.artifacts.filter(a=>a.file.change!=='report');assert(!files.some(a=>a.path.endsWith('unchanged.txt')));
   assert(files.some(a=>a.path.endsWith('copied-output.txt')),'Additional copied source output');assert(files.some(a=>a.path.endsWith('result.txt')&&a.file.root==='/sources'));
   const versions=files.filter(a=>a.path==='binary.dat'||a.path==='main/binary.dat');assert.equal(versions.length,2);
   const first=versions.find(a=>a.version===1),last=versions.find(a=>a.version===2);assert(first&&last);
   const large=files.find(a=>a.path.endsWith('large.bin'));assert(large);const largePreview=await api('/artifacts/'+large.id);assert(largePreview.artifact.file.preview);assert.equal(largePreview.artifact.content.length,1024*1024+1);assert.equal((await (await page.request.get(base+'/api/artifacts/'+large.id+'/download')).body()).length,1024*1024+1);
   const old=await page.request.get(base+'/api/artifacts/'+first.id+'/download');assert.deepEqual(await old.body(),Buffer.from([0,255,...Buffer.from('first')]));
   const full=await page.request.get(base+'/api/artifacts/'+last.id+'/download');assert.deepEqual(await full.body(),Buffer.from([0,255,...Buffer.from('second')]));
   assert.equal(last.file.root,mode==='direct'?'/sources':'/data');
   await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(id=>loadRun(id),run.id);await page.locator('.node-main[data-id=first]').click();
   await page.locator('[data-artifact-query]').waitFor();
   const list=page.locator('[data-inspector-artifacts]>.artifact-item');assert.equal(await list.count(),new Set(files.map(a=>a.file.location)).size);assert.equal(await list.locator('time.artifact-date').count(),await list.count());assert((await list.locator('time.artifact-date').first().innerText()).match(/\d{2}:\d{2}/));
   assert((await page.locator('[data-inspector-artifacts] .artifact-origin').allTextContents()).includes('sources'));
   assert((await page.locator('[data-inspector-artifacts] .artifact-origin').allTextContents()).includes('data'));
   const times=await list.evaluateAll((rows,arts)=>rows.map(row=>{const a=arts.find(a=>a.id===row.dataset.id);return a.file.modified||a.created*1000}),data.artifacts);assert.deepEqual(times,[...times].sort((a,b)=>b-a));
   await page.locator('[data-artifact-query]').fill('binary.dat');
   assert.equal(await list.evaluateAll(rows=>rows.filter(row=>!row.hidden).length),1);
   await page.locator('[data-inspector-artifacts]>.artifact-item:visible').click();await page.locator('#artifact-location').waitFor({state:'visible'});assert((await page.locator('#dialog .modal-body').innerText()).includes('Anteprima non disponibile per questo formato'));
   const download=page.waitForEvent('download');assert.equal(await page.locator('#artifact-download').innerText(),'Download');assert(!(await page.locator('#artifact-download').getAttribute('class')).includes('primary'));const close=page.locator('#dialog .modal-actions [data-action=close-dialog]');assert((await close.getAttribute('class')).includes('primary'));assert((await page.locator('#artifact-download').boundingBox()).x<(await close.boundingBox()).x);await page.locator('#artifact-download').click();assert.equal((await download).suggestedFilename(),'binary.dat');await page.keyboard.press('Escape');await page.locator('#dialog').waitFor({state:'hidden'});
   await page.locator('[data-artifact-query]').fill('.hidden');assert.equal(await list.evaluateAll(rows=>rows.filter(row=>!row.hidden).length),1);
   await page.locator('[data-artifact-query]').fill('');
   await page.setViewportSize({width:390,height:844});assert(await page.locator('#inspector').evaluate(el=>el.scrollWidth<=el.clientWidth));await page.setViewportSize({width:1440,height:1050});
   await page.screenshot({path:path.resolve('test-results/artifacts-'+mode+'.png')});
  }
  // Historical snapshots also sort newest first using their acquisition dates.
  const recovered={id:'recovered',path:'recovered.txt',version:2,created:500,node:'artifact-recovery',file:{location:'/data/recovered.txt',root:'/data',modified:200000,change:'recovered',preview:true,blob:false},hash:'test-hash',content:'Recovery preview',session:'',turn:0};
  await page.route('**/api/artifacts/recovered',r=>r.fulfill({json:{artifact:recovered,html:'<p>Recovery preview</p>'}}));
  await page.evaluate(recovered=>{const meta=[{id:'old',path:'old.txt',version:1,created:1,node:'first'},{id:'recent',path:'recent.txt',version:1,created:99,node:'last'},recovered];app.artifacts=meta;document.querySelector('#inspector').innerHTML=inspectorArtifacts(meta);bindArtifactSearch()},recovered);
  assert.equal(await page.locator('[data-inspector-artifacts]>.artifact-item').first().getAttribute('data-id'),'recovered');
  assert.equal(await page.locator('[data-id=recovered] time').getAttribute('datetime'),'1970-01-01T00:03:20.000Z');
  assert(!(await page.locator('[data-id=recovered]').innerText()).includes('artifact-recovery'));
  await page.locator('[data-inspector-artifacts] [data-id=recovered]').click();await page.locator('#artifact-location').waitFor({state:'visible'});
  const detail=await page.locator('#dialog').innerText();assert(!/artifact-recovery|Recuperato|Recovered|Historical|Storico/.test(detail));assert.match(await page.locator('#artifact-version .caption').innerText(),/Versione 2|Version 2/);assert(!detail.includes('SHA256'));
  assert(!/Recuperato|Recovered|Historical|Storico/.test(await page.locator('[data-inspector-artifacts]').innerText()));
  assert.deepEqual(errors,[]);console.log('PASS: real direct/copy runs, initial-file filtering, additional sources, binary and large outputs, immutable downloads, all-run inspector, hidden files, search, labels, recency order and mobile');
 }finally{await browser?.close();server.kill('SIGINT');fs.rmSync(dir,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
