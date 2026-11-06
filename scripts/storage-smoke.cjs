// Isolated Storage UI checks: metadata totals, GB formatting, downloads and mobile.
const fs=require('fs'),os=require('os'),path=require('path'),assert=require('node:assert/strict'),{spawn}=require('child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'rightcommit-storage-')),sources=path.join(dir,'sources'),base='http://127.0.0.1:18114';
 fs.mkdirSync(path.join(sources,'nested'),{recursive:true});fs.writeFileSync(path.join(sources,'small.txt'),Buffer.alloc(1024));
 const fd=fs.openSync(path.join(sources,'nested','large.bin'),'w');fs.ftruncateSync(fd,4*1024**3);fs.closeSync(fd);
 fs.symlinkSync(sources,path.join(sources,'nested','loop'));
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:path.join(dir,'data'),NODO_LISTEN:'127.0.0.1:18114',NODO_SOURCES:sources}});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  const call=async(url,body)=>{const r=await page.request.fetch(base+'/api'+url,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:body});assert.equal(r.status(),200,await r.text());return r.json()};
  await call('/auth/register',{email:'storage@test.local',password:'temporary-storage-password'});
  assert.equal((await call('/storage/entry-sizes?root=sources&path=')).sizes.nested,4*1024**3);
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>openStorage('','sources'));
  await page.waitForFunction(()=>document.querySelector('[data-storage-size="nested"]')?.textContent==='4.0 GB');
  assert((await page.locator('.storage-entry[data-storage-path="small.txt"]').innerText()).includes('1.0 KB'));
  await page.locator('.storage-entry[data-storage-path="small.txt"]').click();await page.locator('.storage-file-head .storage-entry-size').waitFor();assert.equal(await page.locator('.storage-file-head .storage-entry-size').textContent(),'1.0 KB');
  const workflow={id:'storage-ui',name:'Storage UI',description:'',entry:'a',nodes:[{id:'source',kind:'Source',title:'Source',source_type:'example',x:80,y:100},{id:'a',kind:'Connection',title:'Read',provider:'local',operation:'read_ticket',location:'README.md',x:200,y:100},{id:'review',kind:'Review',title:'Review',mode:'human',x:300,y:100}],edges:[{from:'source',to:'a',kind:'context'},{from:'a',to:'review',kind:'control'}]};
  await call('/workflows',workflow);const run=await call('/runs',{workflow_id:workflow.id});
  await page.evaluate(id=>openStorage(id,'workspaces'),run.id);
  const zip=page.locator('.storage-header-actions a[href*="/storage/download"]'),rag=page.locator('.storage-header-actions .rag-folder-action');
  await zip.waitFor();const z=await zip.boundingBox(),r=await rag.boundingBox();assert(z.y>=r.y+r.height);assert(Math.abs(z.x+z.width-r.x-r.width)<2);
  const downloadPromise=page.waitForEvent('download');await zip.click();assert((await downloadPromise).suggestedFilename().endsWith('.zip'));
  await page.setViewportSize({width:390,height:900});assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
  const zm=await zip.boundingBox(),rm=await rag.boundingBox();assert(zm.y>=rm.y+rm.height);assert(zm.x>=0&&zm.x+zm.width<=390);
  await page.screenshot({path:'/tmp/rightcommit-storage-mobile.png'});assert.deepEqual(errors,[]);
  console.log('PASS: file/folder sizes, 4 GB metadata total without content reads, symlink loop skipped, file viewer size, result ZIP below RAG/right-aligned, ZIP download and mobile layout');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(dir,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
