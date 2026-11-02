const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),assert=require('node:assert/strict'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'markdown-tables-')),base='http://127.0.0.1:18164',sources=path.join(dir,'sources');fs.mkdirSync(sources);
 const markdown='# Catalogo\n\n| Progetto (catene) | Letti | Scritti | Evidenza: primo riscontro per oggetto |\n|---|---|---|---|\n| `adapterPagoPa` (3) | `ASSOC_IUV_FATT`, `RENPPA`, `RENPPA_RETTIFICHE`, `TPARM`, `TPSP`, `WWASSIUVFAT` | `RENPPA_RETTIFICHE` | `ASSOC_IUV_FATT` AssIUVFattDao.java:67; `RENPPA` RenPPADao.java:130; `RENPPA_RETTIFICHE` RenPPAFrtDao.java:130; `TPARM` ParametroDao.java:59; `TPSP` PspDao.java:30; `WWASSIUVFAT` AssIUVFattDao.java:29 |\n| `CheckIbanDomiEstera` | `UTEDOMES` | — | `progetto/cartella/molto/lunga/'+ 'sottocartella/'.repeat(15)+'UteDomesDao.java:41` |\n';
 fs.writeFileSync(path.join(sources,'table.md'),markdown);
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18164',NODO_SOURCES:sources},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1050}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  for(const file of ['app.js','app.css','i18n.js'])await page.route('**/'+file,r=>r.fulfill({path:path.resolve('web/'+file)}));
  assert((await page.request.post(base+'/api/auth/register',{headers:{'X-Nodo-Request':'1'},data:{email:'tables@test.local',password:'temporary-table-password'}})).ok());
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});
  const response=await page.request.get(base+'/api/storage?root=sources&path=table.md');assert(response.ok());const data=await response.json();assert(data.html.includes('<table>'));
  const check=async(label)=>{
   const wrapper=page.locator('#dialog .markdown-table-scroll');await wrapper.waitFor({state:'visible'});
   const metrics=await wrapper.evaluate(el=>{const cells=[...el.querySelectorAll('tbody tr:first-child td')],project=el.querySelector('tbody tr:nth-child(2) td:first-child code'),range=document.createRange();range.selectNodeContents(project);const projectLines=range.getClientRects().length;el.scrollLeft=200;return {projectLines,client:el.clientWidth,total:el.scrollWidth,left:el.scrollLeft,widths:cells.map(c=>c.getBoundingClientRect().width),body:el.closest('.modal-body').scrollWidth,bodyWidth:el.closest('.modal-body').clientWidth}});
   assert.equal(metrics.projectLines,1,label+': project identifier is not split');assert(metrics.widths.every(w=>w>=139),label+': readable column minimum');assert(metrics.widths.every(w=>w<=530),label+': long content wraps');
   if(page.viewportSize().width<600)assert(metrics.total>metrics.client,label+': narrow table scrolls');if(metrics.total>metrics.client)assert(metrics.left>0,label+': scrolling works');assert(metrics.body<=metrics.bodyWidth,label+': body does not overflow horizontally');
   assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),label+': page does not overflow');await wrapper.evaluate(el=>el.scrollLeft=0);
  };
  for(const width of [1440,390]){await page.setViewportSize({width,height:1050});await page.evaluate(()=>openStorage('table.md','sources'));await check('Storage '+width)}
  const item={id:'table-test',path:'table.md',version:1,created:1793476800,node:'task',content:markdown,hash:'fixture',session:'',turn:1,file:{location:'/sources/table.md',root:'/sources',preview:true,modified:1793476800000,change:'created'}};
  await page.route('**/api/artifacts/table-test',r=>r.fulfill({json:{artifact:item,html:data.html}}));
  await page.evaluate(async item=>{app.artifacts=[item];await artifact(item.id)},item);
  for(const width of [1440,390]){await page.setViewportSize({width,height:1050});await check('Artifacts '+width);if(width===1440)await page.screenshot({path:'test-results/markdown-artifact-table.png'})}
  const buttons=page.locator('#dialog .modal-actions');assert((await buttons.locator('[data-action=close-dialog]').getAttribute('class')).includes('primary'));assert(!(await buttons.locator('#artifact-download').getAttribute('class')).includes('primary'));
  assert.deepEqual(errors,[]);console.log('PASS: readable Markdown columns, bounded long identifiers, horizontal scrolling, Storage and Artifacts, desktop/mobile, primary Close');
 }finally{await browser?.close();server.kill('SIGINT');fs.rmSync(dir,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
