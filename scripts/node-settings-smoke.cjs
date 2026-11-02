// Disposable instance: inspector edits must survive selection, saving and reload, without Applica.
const fs=require('fs'),os=require('os'),path=require('path'),assert=require('node:assert/strict'),{spawn}=require('child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'nodo-node-settings-')),base='http://127.0.0.1:18099';
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18099',NODO_SOURCES:path.resolve('sources')}});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1050}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  // Exercise current frontend even when the local debug binary embeds older assets.
  await page.route('**/app.css',route=>route.fulfill({path:path.resolve('web/app.css'),contentType:'text/css'}));
  await page.route('**/app.js',route=>route.fulfill({path:path.resolve('web/app.js'),contentType:'text/javascript'}));
  const call=async(url,body)=>{const r=await page.request.fetch(base+'/api'+url,{method:body?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:body});assert.equal(r.status(),200,await r.text());return r.json()};
  await call('/auth/register',{email:'node-settings@test.local',password:'temporary-node-settings-password'});
  const wf={id:'node-settings-qa',name:'Node settings QA',description:'',entry:'task',nodes:[{id:'source',kind:'Source',title:'Source',source_type:'example',x:80,y:100},{id:'agent',kind:'Agent',title:'Agent',runtime:'codex',x:300,y:100},{id:'task',kind:'Task',title:'Task',agent:'agent',x:520,y:100},{id:'gate',kind:'Gate',title:'Gate',x:750,y:100,outcomes:['pass','rework'],max_rework:2}],edges:[]};await call('/workflows',wf);
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.locator('#workflow-select').selectOption(wf.id);
  const select=async id=>page.locator('.node-main[data-id="'+id+'"]').click();const field=name=>page.locator('#node-form [name="'+name+'"]');
  await select('source');await field('source_type').selectOption('folder');await field('location').fill('analisi');assert(await page.evaluate(()=>app.dirty));
  await select('agent');await field('runtime').selectOption('claude');await field('model').fill('opus');
  await select('task');await field('prompt').fill('Inspect UBS without changing files');await field('command').fill('analyze');
  await select('gate');await field('outcomes').fill('accepted, revise');await field('max_rework').fill('3');await field('require_tests').selectOption('true');
  await select('source');assert.equal(await field('source_type').inputValue(),'folder');assert.equal(await field('location').inputValue(),'analisi');
  // Save while the last field still has focus, with no Apply or Save click.
  await field('location').fill('esempio');await page.waitForFunction(()=>!app.dirty);
  let stored=(await call('/workflows')).find(w=>w.id===wf.id),node=id=>stored.nodes.find(n=>n.id===id);
  assert.equal(node('source').source_type,'folder');assert.equal(node('source').location,'esempio');assert.equal(node('agent').runtime,'claude');assert.equal(node('agent').model,'opus');assert.equal(node('task').prompt,'Inspect UBS without changing files');assert.equal(node('task').command,'analyze');assert.deepEqual(node('gate').outcomes,['accepted','revise']);assert.equal(node('gate').max_rework,3);assert.equal(node('gate').require_tests,true);
  await page.reload();await page.locator('#app').waitFor({state:'visible'});await page.locator('#workflow-select').selectOption(wf.id);await select('source');assert.equal(await field('source_type').inputValue(),'folder');assert.equal(await field('location').inputValue(),'esempio');
  await field('source_type').selectOption('git');await field('location').fill('https://example.com/ubs.git');await page.waitForFunction(()=>!app.dirty);await page.reload();await page.locator('#app').waitFor({state:'visible'});await page.locator('#workflow-select').selectOption(wf.id);await select('source');assert.equal(await field('source_type').inputValue(),'git');assert.equal(await field('location').inputValue(),'https://example.com/ubs.git');assert.deepEqual(errors,[]);
  // Synthetic catalogs exercise saved references without contacting external services.
  const ragIndex={id:'rag-qa',name:'QA index',root:'sources',path:'material',directory:true,generation:1,progress:{state:'completed'}};
  await page.route('**/api/rag',route=>route.fulfill({json:{indices:[ragIndex],languages:[]}}));
  await page.route('**/api/storage?root=sources&path=',route=>route.fulfill({json:{entries:[{name:'plain.txt',directory:false},...Array.from({length:35},(_,i)=>({name:'folder-'+String(i).padStart(2,'0'),directory:true}))]}}));
  const seed=()=>page.evaluate(()=>{app.settings.mcp_servers={'mcp-qa':{name:'QA MCP'}};a2aConnections=[{id:'a2a-qa',name:'QA remote'}];ragCatalog=[{id:'rag-qa',name:'QA index',root:'sources',path:'material',directory:true,generation:1,progress:{state:'completed'}}]});
  await seed();await page.evaluate(()=>{for(const kind of ['Review','Memory','Connection','Workflow','MCP','RAG','A2A','Comment'])app.wf.nodes.push({id:kind.toLowerCase(),kind,title:kind,phase:'Workflow',x:100,y:400,agent:'agent',text:'',markdown:false,width:300,height:220,max_rework:2,rag_top_k:8,provider:'local',operation:'test'});render()});
  const inspect=async id=>page.evaluate(id=>{setNodeSelection([id]);render()},id);
  await inspect('source');await field('source_type').selectOption('folder');await page.locator('#source-folder-options [role=option]').first().waitFor({state:'attached'});await field('location').click();
  assert.equal(await page.locator('#source-folder-options [role=option]').count(),35);assert(await page.locator('#source-folder-options').evaluate(el=>el.scrollHeight>el.clientHeight&&getComputedStyle(el).overflowY==='auto'));
  await page.locator('[data-folder="folder-34"]').click();assert.equal(await field('location').inputValue(),'folder-34');assert(await page.locator('#source-folder-options').isHidden());
  await field('location').fill('custom/subfolder');await field('location').press('Escape');assert.equal(await field('location').inputValue(),'custom/subfolder');await field('location').press('ArrowDown');await field('location').press('Enter');assert.equal(await field('location').inputValue(),'folder-00');await field('location').fill('custom/subfolder');await field('location').press('Tab');assert(await page.locator('#source-folder-options').isHidden());
  const changes={
   review:{agent:'agent',mode:'human',prompt:'Review criteria'},
   memory:{agent:'task',target:'agent'},
   connection:{provider:'atlassian',command:'["jira","workitem","get","QA-1"]'},
   workflow:{workflow_id:'child-qa',prompt:'Child context'},
   mcp:{mcp_id:'mcp-qa',mcp_tools:'search, read\nlist'},
   rag:{rag_index:'rag-qa',rag_mode:'keyword',rag_top_k:'13'},
   a2a:{a2a_id:'a2a-qa',prompt:'Remote request'}
  };
  await call('/workflows',{id:'child-qa',name:'Child QA',description:'',entry:'',nodes:[],edges:[]});await page.evaluate(async()=>{app.workflows=await api('/workflows')});
  for(const [id,fields] of Object.entries(changes)){await inspect(id);await field('title').fill('Configured '+id);await field('phase').fill('QA phase');for(const [name,value] of Object.entries(fields)){const f=field(name);if(await f.evaluate(el=>el.tagName==='SELECT'))await f.selectOption(value);else await f.fill(value)}}
  // Dynamic hidden operation must persist when leaving Connection without Apply/Save.
  assert.equal(await page.evaluate(()=>app.wf.nodes.find(n=>n.id==='connection').operation),'twg');
  await inspect('gate');await field('max_rework').fill('0');await field('require_tasks_closed').selectOption('true');await field('location').fill('specs/qa/tasks.md');
  await inspect('comment');await page.locator('.comment-node[data-id=comment] textarea').fill('## Saved comment');await page.locator('.comment-node[data-id=comment] [data-comment-preview]').click();
  await inspect('source');assert.equal(await field('location').inputValue(),'custom/subfolder');
  const expected=await page.evaluate(()=>structuredClone(app.wf.nodes));await page.waitForFunction(()=>!app.dirty);stored=(await call('/workflows')).find(w=>w.id===wf.id);
  for(const original of expected){const actual=stored.nodes.find(n=>n.id===original.id);for(const [key,value] of Object.entries(original))assert.deepEqual(actual[key],value,original.kind+'.'+key)}
  await page.reload();await page.locator('#app').waitFor({state:'visible'});await page.locator('#workflow-select').selectOption(wf.id);await seed();
  for(const [id,fields] of Object.entries(changes)){await inspect(id);assert.equal(await field('title').inputValue(),'Configured '+id);for(const [name,value] of Object.entries(fields)){const expectedValue=name==='mcp_tools'?'search\nread\nlist':value;assert.equal(await field(name).inputValue(),expectedValue,id+'.'+name)}}
  await inspect('gate');assert.equal(await field('max_rework').inputValue(),'0');await inspect('source');assert.equal(await field('location').inputValue(),'custom/subfolder');assert.equal(await field('source_type').inputValue(),'folder');assert.equal(await page.evaluate(()=>app.wf.nodes.find(n=>n.id==='comment').text),'## Saved comment');assert.equal(await page.evaluate(()=>app.wf.nodes.find(n=>n.id==='comment').markdown),true);assert.deepEqual(errors,[]);
  // Missing external catalogs must not silently replace previously saved references.
  await page.unroute('**/api/rag');await page.evaluate(()=>{ragCatalog=[];a2aConnections=[];app.settings.mcp_servers={}});
  for(const id of ['mcp','rag','a2a']){await inspect(id);await page.waitForFunction(()=>!app.dirty)}
  stored=(await call('/workflows')).find(w=>w.id===wf.id);assert.equal(node('mcp').mcp_id,'mcp-qa');assert.equal(node('rag').rag_index,'rag-qa');assert.equal(node('a2a').a2a_id,'a2a-qa');
  console.log('PASS: all 12 node kinds retain settings after node switches, automatic save without Apply/Save and reload; dynamic Connection operation and zero values; Source editable scrolling folder list, mouse and keyboard selection');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(dir,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
