const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),{spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const data=fs.mkdtempSync(path.join(os.tmpdir(),'pi-multi-')),base='http://127.0.0.1:18256',binary=path.join(data,'system/tools/bin/pi');fs.mkdirSync(path.dirname(binary),{recursive:true});
 fs.writeFileSync(binary,`#!/usr/bin/env python3
import os,sys,json,pathlib
args=sys.argv
provider=args[args.index('--provider')+1];model=args[args.index('--model')+1]
expected={'xai':('XAI_API_KEY','multi-xai-key'),'openai':('OPENAI_API_KEY','multi-openai-key'),'local':('PI_LOCAL_API_KEY','multi-local-key')}
key,value=expected[provider]
assert os.environ.get(key)==value
for other in ['XAI_API_KEY','OPENAI_API_KEY','PI_LOCAL_API_KEY']:
 if other!=key:assert other not in os.environ
assert 'personal-key-must-not-leak' not in str(os.environ)
assert model=={'xai':'grok-fixture','openai':'gpt-fixture','local':'local-fixture'}[provider]
text='OK' if '--no-tools' in args else '<nodo_result>'+json.dumps(dict(status='completed',summary='Completed '+provider,artifacts=[],questions=[]))+'</nodo_result>'
if '--session-id' in args:print(json.dumps(dict(type='session',id=args[args.index('--session-id')+1])))
print(json.dumps(dict(type='message_end',message=dict(role='assistant',stopReason='stop',content=[dict(type='text',text=text)]))))
`,{mode:0o700});
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{cwd:data,env:{...process.env,NODO_DATA:data,NODO_LISTEN:'127.0.0.1:18256',NODO_SOURCES:path.resolve('sources'),XAI_API_KEY:'multi-xai-key',OPENAI_API_KEY:'multi-openai-key',PI_LOCAL_API_KEY:'multi-local-key',PI_LOCAL_BASE_URL:'http://127.0.0.1:1234/v1'},stdio:'ignore'});let browser;
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  for(const file of ['app.js','app.css','i18n.js','controls.js'])await page.route('**/'+file,r=>r.fulfill({path:path.resolve('web/'+file)}));
  const api=async(p,method='GET',body)=>{const r=await page.request.fetch(base+'/api'+p,{method,headers:{'X-Nodo-Request':'1'},data:body});assert(r.ok(),await r.text());return r.json()};
  await api('/auth/register','POST',{email:'multi@test.local',password:'temporary-multi-password'});
  let settings=await api('/settings');await api('/settings','PATCH',{revision:settings._revision,changes:{agents:{pi:{provider:'openai',model:'gpt-fixture',api_key:'personal-key-must-not-leak'}}}});
  await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(()=>settingsDialog('agents'));
  const select=async(name,value)=>{const field=page.locator('[name='+name+']');await field.getByRole('combobox').click();await field.getByRole('option',{name:value,exact:true}).click()};
  const picker=page.locator('[name=provider_pi]');await picker.getByRole('combobox').click();
  assert.equal(await picker.getByRole('option').first().textContent().then(text=>text.replace('✓','').trim()),'Multi-provider');assert.equal(await picker.getByRole('separator').count(),2);assert(await picker.evaluate(el=>el.shadowRoot.querySelector('.option:last-child').previousElementSibling.matches('hr.menu-separator')));
  await page.keyboard.press('Home');await page.keyboard.press('ArrowDown');await page.keyboard.press('Enter');assert.equal(await picker.evaluate(el=>el.value),settings.pi_providers[0].id);
  await picker.getByRole('combobox').click();await page.keyboard.press('End');await page.keyboard.press('Enter');assert.equal(await picker.evaluate(el=>el.value),'local');
  await select('provider_pi','Multi-provider');assert(await page.locator('[name=key_pi]').isHidden());assert(await page.locator('[data-action=clear-key][data-field=key_pi]').isHidden());assert(await page.locator('#key_pi-help').isHidden());assert(await page.locator('[name=pi_default_provider]').isVisible());
  await page.locator('[data-action=save-settings]').click();await page.waitForFunction(()=>app.settings.agents.pi.multi_provider);settings=await api('/settings');assert(settings.agents.pi.configured);assert(settings.pi_providers.length>30);
  await page.locator('[data-action=pi-check]').click();await page.locator('[data-pi-status]').filter({hasText:'Pi connection successful'}).waitFor();
  await page.keyboard.press('Escape');await page.evaluate(()=>{app.wf={id:'multi-ui',name:'Multi UI',description:'',entry:'',nodes:[{id:'agent-ui',kind:'Agent',title:'Pi UI',runtime:'pi',model:'gpt-fixture',x:0,y:0}],edges:[]};app.run=null;app.selected='agent-ui';app.selectedNodes=['agent-ui'];render()});
  const nodeProvider=page.locator('#node-form [name=pi_provider]');assert(await nodeProvider.isVisible());assert.equal(await nodeProvider.locator('jui-option').count(),settings.pi_providers.length+1);
  await nodeProvider.getByRole('combobox').click();await nodeProvider.getByRole('option',{name:'xAI (Grok)',exact:true}).click();await page.locator('#node-form [name=model]').fill('grok-fixture');
  await page.evaluate(()=>captureInspectorFields());assert.equal(await page.evaluate(()=>app.wf.nodes[0].pi_provider),'xai');
  await page.locator('#node-form [name=pi_provider]').getByRole('combobox').click();await page.locator('#node-form [name=pi_provider]').getByRole('option',{name:'Local / OpenAI compatible',exact:true}).click();assert(await page.locator('#node-form [name=pi_base_url]').isVisible());
  await page.locator('#node-form [name=pi_base_url]').fill('http://127.0.0.1:1235/v1');await page.evaluate(()=>captureInspectorFields());await page.evaluate(()=>saveWorkflow());
  const persisted=(await api('/workflows')).find(workflow=>workflow.id==='multi-ui');assert.equal(persisted.nodes[0].pi_base_url,'http://127.0.0.1:1235/v1');assert(!JSON.stringify(persisted).includes('multi-local-key'));
  const wf={id:'multi-run',name:'Multi-provider workflow',description:'',entry:'task-xai',nodes:[{id:'source',kind:'Source',title:'Source',source_type:'example'},...['xai','openai','local'].flatMap((provider,index)=>[{id:'agent-'+provider,kind:'Agent',title:'Agent '+provider,runtime:'pi',pi_provider:provider,model:{xai:'grok-fixture',openai:'gpt-fixture',local:'local-fixture'}[provider]},{id:'task-'+provider,kind:'Task',title:'Task '+provider,agent:'agent-'+provider,prompt:'Offline test',x:index*200,y:200}])],edges:[{from:'source',to:'agent-xai',kind:'context'},{from:'source',to:'agent-openai',kind:'context'},{from:'source',to:'agent-local',kind:'context'},{from:'agent-xai',to:'task-xai',kind:'context'},{from:'agent-openai',to:'task-openai',kind:'context'},{from:'agent-local',to:'task-local',kind:'context'},{from:'task-xai',to:'task-openai',kind:'control'},{from:'task-openai',to:'task-local',kind:'control'}]};
  wf.nodes.push({id:'handoff-openai',kind:'Memory',title:'Handoff OpenAI',agent:'task-xai',target:'agent-openai'},{id:'handoff-local',kind:'Memory',title:'Handoff local',agent:'task-openai',target:'agent-local'});
  wf.edges=wf.edges.filter(edge=>edge.kind!=='control');wf.edges.push({from:'task-xai',to:'handoff-openai',kind:'control'},{from:'handoff-openai',to:'task-openai',kind:'control'},{from:'task-openai',to:'handoff-local',kind:'control'},{from:'handoff-local',to:'task-local',kind:'control'});
  await api('/workflows','POST',wf);const run=await api('/runs','POST',{workflow_id:wf.id,input:''});
  let result;for(let i=0;i<150;i++){result=(await api('/runs/'+run.id)).run;if(['completed','failed'].includes(result.status))break;await new Promise(r=>setTimeout(r,100))}
  assert.equal(result.status,'completed',result.error);for(const provider of ['xai','openai','local'])assert.equal(result.sessions['agent-'+provider].provider,provider);assert.equal(result.sessions['agent-local'].base_url,'http://127.0.0.1:1234/v1');assert(!JSON.stringify(result.workflow).includes('multi-xai-key'));
  const missing=structuredClone(wf);missing.id='multi-missing';missing.nodes.find(n=>n.id==='agent-xai').pi_provider='openai-codex';await api('/workflows','POST',missing);const rejected=await page.request.post(base+'/api/runs',{headers:{'X-Nodo-Request':'1'},data:{workflow_id:missing.id,input:''}});assert.equal(rejected.status(),400);assert((await rejected.json()).error.includes('Agent xai'));
  await page.evaluate(()=>settingsDialog('agents'));await page.evaluate(()=>{const language=document.querySelector('[data-language]');language.value='it';language.dispatchEvent(new Event('change',{bubbles:true}))});assert.equal(await page.locator('[data-pi-multi-default] label').first().textContent().then(s=>s.trim().startsWith('Provider predefinito Pi')),true);
  await page.setViewportSize({width:390,height:844});assert(await page.locator('#settings-form').evaluate(el=>el.scrollWidth<=el.clientWidth));assert.deepEqual(errors,[]);
  console.log('PASS: Multi-provider settings IT/EN, 42-provider node picker, local URL overrides, actual three-provider workflow, scoped env keys, ignored generic key, frozen sessions and missing-credentials preflight');
 }finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(data,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
