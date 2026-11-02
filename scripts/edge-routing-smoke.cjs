const fs=require('fs'),os=require('os'),path=require('path'),assert=require('node:assert/strict'),{spawn}=require('child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{const root=process.cwd(),dir=fs.mkdtempSync(path.join(os.tmpdir(),'nodo-returns-')),base='http://127.0.0.1:18109';const server=spawn(path.join(root,'target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18109',NODO_SOURCES:path.join(root,'sources')},stdio:'ignore'});let browser;try{
for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:950}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.route('**/app.js',r=>r.fulfill({path:path.join(root,'web/app.js'),contentType:'text/javascript'}));
await page.route('**/app.css',r=>r.fulfill({path:path.join(root,'web/app.css'),contentType:'text/css'}));
const call=async(url,body)=>{const r=await page.request.fetch(base+'/api'+url,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:body});assert.equal(r.status(),200,await r.text());return r.json()};
await call('/auth/register',{email:'returns@test.local',password:'temporary-return-password'});
await page.route(base+'/',r=>r.fulfill({path:path.join(root,'web/index.html'),contentType:'text/html'}));
await page.goto(base);await page.locator('#app').waitFor({state:'visible'});
const templates=await call('/workflow-templates');
const baseline=templates.find(t=>t.workflow.nodes.some(n=>n.title==='Pacchetto per Spec Kit')&&t.workflow.id==='as-is-sdd').workflow;
await page.evaluate(w=>{app.run=null;app.wf=w;render();fit()},baseline);
const betweenRows=await page.evaluate(()=>{
 const w=app.wf,target=w.nodes.find(n=>n.title==='Pacchetto per Spec Kit'),edge=w.edges.find(e=>e.to===target.id&&e.kind==='control'),source=w.nodes.find(n=>n.id===edge.from);
 const check=()=>{
  const geometry=edgeGeometry(w,edge),path=[...document.querySelectorAll('#edges .edge.control')].find(p=>p.dataset.from===source.id&&p.dataset.to===target.id),box=path.getBBox();
  let collisions=0;
  for(let d=10;d<path.getTotalLength()-10;d+=2){const point=path.getPointAtLength(d);for(const node of document.querySelectorAll('#nodes .node')){const n=w.nodes.find(n=>n.id===node.dataset.id);if(n.id===source.id&&d<40||n.id===target.id&&d>path.getTotalLength()-40)continue;if(point.x>n.x-8&&point.x<n.x+124&&point.y>n.y-12&&point.y<n.y+Math.max(150,node.offsetHeight)+12)collisions++}}
  return {route:geometry.labelY+8,top:box.y,source:source.y,target:target.y,collisions};
 };
 const checks=[check()];
 // The rule follows geometry after moving the second row, without node-name exceptions.
 for(const spacing of [220,260]){for(const n of w.nodes)if(n.y>100)n.y=100+spacing;render();checks.push(check())}
 for(const n of w.nodes)if(n.y>100)n.y=300;render();fit();return checks;
});
assert(betweenRows.every(r=>r.route>r.source+162&&r.route<r.target-12&&r.top>=r.source+45&&r.collisions===0),JSON.stringify(betweenRows));
await page.screenshot({path:'/tmp/jui-spec-kit-corridor.png'});
await page.evaluate(()=>{app.run=null;app.wf={id:'return-visual',name:'Verifica collegamenti di ritorno',description:'',entry:'task',nodes:[{id:'task',kind:'Task',title:'Preparazione',x:180,y:180},{id:'middle',kind:'Task',title:'Implementazione',x:440,y:180},{id:'review',kind:'Review',title:'Approvazione finale',x:700,y:180}],edges:[{from:'task',to:'middle',kind:'control',outcome:''},{from:'middle',to:'review',kind:'control',outcome:'pass'},{from:'review',to:'task',kind:'rework',outcome:'rework'},{from:'review',to:'middle',kind:'rework',outcome:'correzione'}]};render();fit()});
const result=await page.evaluate(()=>{const w=app.wf;const paths=[...document.querySelectorAll('#edges path.edge.rework')];const measurements=paths.map(p=>{const b=p.getBBox();return {height:b.height,y:b.y,length:p.getTotalLength(),d:p.getAttribute('d')}});const labels=[...document.querySelectorAll('#edges .edge-label')].filter(p=>p.textContent!=='pass').map(p=>+p.getAttribute('y'));return {measurements,labels,forward:edgeGeometry(w,w.edges[0]).d}});
assert.equal(result.measurements.length,2);assert(result.measurements.every(p=>p.height>=70));assert.notEqual(result.measurements[0].y,result.measurements[1].y);assert(result.labels.every((y,i)=>y<result.measurements[i].y));assert(!result.forward.includes('L'));
const hoverPoint=await page.evaluate(()=>{const p=document.querySelectorAll('#edges .edge.rework')[0],pt=p.getPointAtLength(p.getTotalLength()/2).matrixTransform(p.getScreenCTM());return {x:pt.x,y:pt.y}});
await page.mouse.move(hoverPoint.x,hoverPoint.y);assert.equal(await page.locator('.edge.hovered').count(),1);assert.equal(await page.locator('.node.edge-endpoint').count(),2);assert.deepEqual(await page.locator('.node.edge-endpoint').evaluateAll(ns=>ns.map(n=>n.dataset.id).sort()),['review','task']);
await page.screenshot({path:'/tmp/nodo-return-hover.png'});
await page.mouse.move(20,20);assert.equal(await page.locator('.edge.hovered,.node.edge-endpoint').count(),0);
const hit=page.locator('#edges .edge-hit').nth(2);await hit.focus();await page.keyboard.press('Enter');assert.equal(await page.locator('#edges .edge.rework.selected').count(),1);
await page.screenshot({path:'/tmp/nodo-return-edges.png'});
await page.evaluate(()=>{app.wf.nodes[0].y=360;drawEdges();fit()});assert(await page.locator('#edges .edge.rework').count()===2);
await page.evaluate(()=>{app.wf.nodes=[{id:'task',kind:'Task',title:'Prima analisi',x:180,y:100},{id:'middle',kind:'Task',title:'Scrivi README',x:440,y:100},{id:'memory',kind:'Memory',title:'Passaggio al revisore',x:700,y:100},{id:'verify',kind:'Task',title:'Verifica avversariale',x:180,y:350},{id:'refine',kind:'Task',title:'Perfezionamento',x:440,y:350},{id:'review',kind:'Review',title:'Approvazione finale',x:960,y:350}];app.wf.edges=[{from:'task',to:'middle',kind:'control'},{from:'middle',to:'memory',kind:'control'},{from:'memory',to:'verify',kind:'control'},{from:'review',to:'verify',kind:'rework',outcome:'rework'},{from:'review',to:'refine',kind:'rework',outcome:'rework'}];render();fit()});
const compact=await page.evaluate(()=>[...document.querySelectorAll('#edges .edge:not(.signal)')].slice(2).map(p=>{const box=p.getBBox();let collisions=0;for(let d=12;d<p.getTotalLength()-12;d+=2){const pt=p.getPointAtLength(d);for(const n of app.wf.nodes){if(pt.x>n.x+14&&pt.x<n.x+102&&pt.y>n.y&&pt.y<n.y+88)collisions++}}return {y:box.y,height:box.height,collisions}}));
assert(compact.slice(1).every(p=>p.y>250&&p.y<340));assert(compact[0].y>=100);assert(compact.every(p=>p.collisions===0));assert.equal(new Set(compact.map(p=>p.y)).size,3);
await page.screenshot({path:'/tmp/nodo-return-compact.png'});
await page.evaluate(()=>{app.wf.nodes.push({id:'comment',kind:'Comment',title:'Comment',text:'Corridoio occupato',x:100,y:255,width:1100,height:85});render();fit()});
const blocked=await page.evaluate(()=>{const p=document.querySelector('#edges .edge.rework'),rect=p.getBBox();let collisions=0;for(let d=12;d<p.getTotalLength()-12;d+=2){const pt=p.getPointAtLength(d);for(const n of app.wf.nodes){const left=n.kind==='Comment'?n.x:n.x+14,right=n.kind==='Comment'?n.x+n.width:n.x+102,bottom=n.y+(n.kind==='Comment'?n.height:88);if(pt.x>left&&pt.x<right&&pt.y>n.y&&pt.y<bottom)collisions++}}return {collisions,height:rect.height}});assert.equal(blocked.collisions,0);assert(blocked.height>100);
const localOnly=await page.evaluate(()=>{
 const geometry=()=>app.wf.edges.map(e=>edgeGeometry(app.wf,e).d);
 const before=geometry();
 app.wf.nodes.push({id:'far-left',kind:'Comment',title:'Comment',text:'Fuori dal percorso',x:-1000,y:-1800,width:300,height:3600},{id:'far-right',kind:'Comment',title:'Comment',text:'Fuori dal percorso',x:2000,y:-2500,width:300,height:5000});
 render();const after=geometry();
 // Moving and resizing remote comments must leave every local path unchanged.
 app.wf.nodes.find(n=>n.id==='far-left').height=9000;
 app.wf.nodes.find(n=>n.id==='far-right').y=-6000;
 render();return {before,after,moved:geometry()};
});assert.deepEqual(localOnly.after,localOnly.before);assert.deepEqual(localOnly.moved,localOnly.before);
assert.deepEqual(errors,[]);console.log('PASS: Spec Kit row-gap routing at 200/220/260px, curved return routes, separate lanes, labels, unchanged forward route, keyboard selection, hover highlights both endpoints and clears on leave, mixed rows, remote comments leave routes unchanged, no browser errors');
}finally{await browser?.close();server.kill();fs.rmSync(dir,{recursive:true,force:true})}})().catch(e=>{console.error(e);process.exitCode=1});
