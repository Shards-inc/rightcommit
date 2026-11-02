// Offline integration: exercise native control protocols, settings, auth and UI without paid model calls.
const fs=require('fs'),os=require('os'),path=require('path'),assert=require('assert'),{spawn}=require('child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{const dir=fs.mkdtempSync(path.join(os.tmpdir(),'compaction-')),base='http://127.0.0.1:18109',bin=path.join(dir,'bin');fs.mkdirSync(bin);
const fixture=`#!/usr/bin/env python3
import os,sys,json,time,pathlib,uuid
rt=pathlib.Path(sys.argv[0]).name;a=sys.argv[1:];cwd=pathlib.Path.cwd()
def emit(x):print(json.dumps(x),flush=True)
def flag(k):return a[a.index(k)+1] if k in a else ''
def record(sid):
 p=cwd/'.compact-fixture.json';p.write_text(json.dumps(dict(runtime=rt,session=sid,args=a,window=os.environ.get('CLAUDE_CODE_AUTO_COMPACT_WINDOW'),percent=os.environ.get('CLAUDE_AUTOCOMPACT_PCT_OVERRIDE'))))
def fail():return (cwd/'.fail-compact').exists()
if rt=='codex' and 'app-server' in a:
 sid=''
 for line in sys.stdin:
  v=json.loads(line);m=v.get('method');i=v.get('id')
  if m=='initialize':emit(dict(id=i,result={}))
  elif m=='account/login/start':
   assert 'cli_auth_credentials_store="ephemeral"' in a;assert v['params']['apiKey']==os.environ['CODEX_API_KEY'];emit(dict(id=i,result=dict(type='apiKey')))
  elif m=='thread/resume':
   sid=v['params']['threadId'];record(sid);assert v['params']['approvalPolicy']=='never';emit(dict(id=i,result=dict(thread=dict(id=sid))))
  elif m=='thread/compact/start':
   assert v['params']['threadId']==sid;time.sleep(.6);emit(dict(id=i,result={}))
   if fail():emit(dict(method='error',params=dict(threadId=sid,error=dict(message='ECONNREFUSED '+os.environ['OPENAI_API_KEY']),willRetry=False)))
   else:
    emit(dict(method='item/completed',params=dict(threadId=sid,item=dict(type='contextCompaction'))));emit(dict(method='turn/completed',params=dict(threadId=sid,turn=dict(status='completed'))))
elif rt=='pi' and flag('--mode')=='rpc':
 sid=flag('--session');record(sid);assert sid;assert '--no-tools' in a
 for line in sys.stdin:
  v=json.loads(line);assert v['type']=='compact';time.sleep(.6)
  emit(dict(type='response',id=v['id'],command='compact',success=not fail(),error='ECONNREFUSED '+os.environ['OPENAI_API_KEY'] if fail() else ''))
else:
 prompt=sys.stdin.read();sid=flag('--session-id') or flag('--resume') or str(uuid.uuid4())
 if rt=='codex' and 'resume' in a:sid=a[a.index('resume')+1]
 if rt=='claude' and prompt=='/compact':
  record(sid);time.sleep(.6)
  if fail():emit(dict(type='result',subtype='success',is_error=False,result='Error during compaction: ECONNREFUSED '+os.environ['ANTHROPIC_AUTH_TOKEN']))
  else:
   emit(dict(type='system',subtype='compact_boundary',compact_metadata=dict(trigger='manual',pre_tokens=180000)));emit(dict(type='result',is_error=False,result='Compacted'))
 else:
  record(sid)
  result='<nodo_result>'+json.dumps(dict(status='completed',summary='Offline fixture',questions=[],artifacts=[]))+'</nodo_result>'
  if rt=='codex':
   emit(dict(type='thread.started',thread_id=sid));emit(dict(type='item.completed',item=dict(type='context_compaction')));emit(dict(type='item.completed',item=dict(type='agent_message',text=result)))
  elif rt=='claude':
   emit(dict(type='system',subtype='init',session_id=sid));time.sleep(1.5 if 'LONG_TURN_COMPACT_GUARD' in prompt else 0);emit(dict(type='system',subtype='status',status='compacting'));emit(dict(type='system',subtype='compact_boundary',compact_metadata=dict(trigger='auto',pre_tokens=180000)));emit(dict(type='result',is_error=False,result=result))
  else:
   emit(dict(type='session',id=sid));emit(dict(type='compaction_start',reason='threshold'));emit(dict(type='compaction_end',reason='threshold',result=dict(summary='PRIVATE SUMMARY',tokensBefore=180000,estimatedTokensAfter=30000)));emit(dict(type='message_end',message=dict(role='assistant',content=[dict(type='text',text=result)])))
`;
for(const rt of ['claude','codex','pi'])fs.writeFileSync(path.join(bin,rt),fixture,{mode:0o755});
const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18109',NODO_SOURCES:path.resolve('sources'),PATH:bin+':'+process.env.PATH,CLAUDE_CODE_AUTO_COMPACT_WINDOW:'100000',CLAUDE_AUTOCOMPACT_PCT_OVERRIDE:'37',ANTHROPIC_AUTH_TOKEN:'compact-secret-sentinel',OPENAI_API_KEY:'compact-openai-secret'},stdio:'ignore'});let browser;
try{for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1050}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
const request=async(p,b,method)=>page.request.fetch(base+'/api'+p,{method:method||(b?'POST':'GET'),headers:{'X-Nodo-Request':'1'},data:b});
const api=async(p,b,method)=>{const r=await request(p,b,method);assert(r.ok(),await r.text());return r.json()};
await api('/auth/register',{email:'compact@test.local',password:'temporary-compact-password'});
let settings=await api('/settings');assert.equal(settings.workflow_parallelism,5);assert.equal(settings.compaction.pi.reserve_tokens,16384);assert.equal(settings.compaction.claude.trigger_percent,80);assert.equal(settings.compaction.claude.context_window_tokens,0);
settings.workflow_parallelism=0;assert.equal((await request('/settings',settings,'PUT')).status(),400);settings.workflow_parallelism=6;
settings.compaction.claude={...settings.compaction.claude,context_window_tokens:200000,trigger_percent:70};settings.compaction.codex={...settings.compaction.codex,context_window_tokens:200000,threshold_tokens:160000};settings.compaction.pi.reserve_tokens=32000;
settings=await api('/settings',settings,'PUT');assert.equal(settings.workflow_parallelism,6);assert.equal(settings.compaction.claude.context_window_tokens,0);
const runs={};for(const rt of ['claude','codex','pi']){const wf={id:'compact-'+rt,name:'Compact '+rt,description:'Offline compaction fixture',entry:'task',nodes:[{id:'source',kind:'Source',title:'Source',source_type:'example'},{id:'agent',kind:'Agent',title:rt,runtime:rt},{id:'task',kind:'Task',title:'Work',agent:'agent',prompt:'Offline compact test'}],edges:[{from:'source',to:'agent',kind:'context'},{from:'agent',to:'task',kind:'context'}]};await api('/workflows',wf);const created=await api('/runs',{workflow_id:wf.id});let run;
for(let i=0;i<200;i++){run=(await api('/runs/'+created.id)).run;assert.notEqual(run.status,'failed',run.error);if(run.status==='completed')break;await new Promise(r=>setTimeout(r,50))}assert.equal(run.status,'completed');runs[rt]=run;
const before=JSON.stringify(run.sessions),response=await api('/runs/'+run.id+'/compact',{binding:'agent'});assert.equal((await request('/runs/'+run.id+'/compact',{binding:'agent'})).status(),400);assert.equal((await request('/runs/'+run.id+'/start',{acknowledge:true})).status(),400);
let found;for(let i=0;i<100;i++){const events=await api('/runs/'+run.id+'/events');found=events.map(e=>e.kind==='compaction'?JSON.parse(e.text):null).find(e=>e?.operation===response.operation&&e.phase==='end');if(found)break;await new Promise(r=>setTimeout(r,50))}assert(found,'manual compact did not finish for '+rt+' '+JSON.stringify(await api('/runs/'+run.id+'/events')));assert.equal(JSON.stringify((await api('/runs/'+run.id)).run.sessions),before);
const log=JSON.stringify(await api('/runs/'+run.id+'/events'));assert(!log.includes('PRIVATE SUMMARY'));assert(log.includes('automatic')||log.includes('threshold'));
const args=JSON.parse(fs.readFileSync(path.join(run.workspace,'.compact-fixture.json')));assert.equal(args.session,run.sessions.agent.native_id);if(rt==='claude'){assert.equal(args.window,null);assert.equal(args.percent,'70');assert(args.args.includes('--autocompact'));assert(args.args.includes('auto'))}if(rt==='codex')assert(args.args.includes('model_auto_compact_token_limit=160000'));
fs.writeFileSync(path.join(run.workspace,'.fail-compact'),'');const failure=await api('/runs/'+run.id+'/compact',{binding:'agent'});let error;
for(let i=0;i<100;i++){const log=await api('/runs/'+run.id+'/events');error=log.map(e=>e.kind==='compaction'?JSON.parse(e.text):null).find(e=>e?.operation===failure.operation&&e.phase==='error');if(error)break;await new Promise(r=>setTimeout(r,50))}assert(error,rt+' error missing');assert(error.message.includes('ECONNREFUSED'));assert(!error.message.includes('compact-secret-sentinel'));assert(!error.message.includes('compact-openai-secret'));assert.equal(JSON.stringify((await api('/runs/'+run.id)).run.sessions),before);fs.unlinkSync(path.join(run.workspace,'.fail-compact'));
}
const guardWf=structuredClone(runs.claude.workflow);guardWf.id='compact-active-guard';guardWf.name='Active guard';guardWf.nodes.find(n=>n.id==='task').prompt='LONG_TURN_COMPACT_GUARD';await api('/workflows',guardWf);const guarded=await api('/runs',{workflow_id:guardWf.id});
for(let i=0;i<100;i++){const r=(await api('/runs/'+guarded.id)).run;if(r.sessions.agent?.native_id){assert.equal(r.status,'running');break}await new Promise(r=>setTimeout(r,20))}
const rejected=await request('/runs/'+guarded.id+'/compact',{binding:'agent'});assert.equal(rejected.status(),400);assert((await rejected.text()).includes('pausa'));
for(let i=0;i<100;i++){if((await api('/runs/'+guarded.id)).run.status==='completed')break;await new Promise(r=>setTimeout(r,30))}
await api('/users',{email:'other-compact@test.local',password:'temporary-compact-password'});
const other=await browser.newContext();await other.request.post(base+'/api/auth/login',{headers:{'X-Nodo-Request':'1'},data:{email:'other-compact@test.local',password:'temporary-compact-password'}});
assert(!(await other.request.post(base+'/api/runs/'+runs.claude.id+'/compact',{headers:{'X-Nodo-Request':'1'},data:{binding:'agent'}})).ok());assert.equal((await other.request.get(base+'/api/settings')).status(),200);await other.close();
await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(id=>loadRun(id),runs.claude.id);await page.evaluate(()=>settingsDialog());await page.waitForFunction(()=>document.querySelector('#compact-binding')?.value==='agent');assert.equal(await page.locator('[name="workflow_parallelism"]').inputValue(),'6');assert.equal(await page.locator('[name="compact_claude_context_window_tokens"]').count(),0);await page.locator('[data-settings-tab="compaction"]').click();await page.locator('[name="compact_claude_trigger_percent"]').fill('80');
await page.locator('[data-settings-tab="execution"]').click();await page.locator('[name="workflow_parallelism"]').fill('5');await page.locator('[data-action="save-settings"]').click();await page.waitForFunction(()=>app.settings.workflow_parallelism===5&&app.settings.compaction.claude.trigger_percent===80&&app.settings.compaction.claude.context_window_tokens===0);await page.waitForFunction(()=>document.querySelector('#compact-binding')?.value==='agent');await page.locator('[data-settings-tab="compaction"]').click();await page.locator('[name="compact_claude_trigger_percent"]').fill('0');await page.locator('[data-action="save-settings"]').click();await page.waitForFunction(()=>app.settings.compaction.claude.trigger_percent===0);await page.waitForFunction(()=>document.querySelector('#compact-binding')?.value==='agent');assert.equal(await page.locator('[name="compact_claude_trigger_percent"]').inputValue(),'0');
await page.locator('[data-action="compact-session"]').click();await page.waitForFunction(()=>document.querySelector('#compact-status')?.textContent.includes('Compattazione completata'));
const nativeArgs=JSON.parse(fs.readFileSync(path.join(runs.claude.workspace,'.compact-fixture.json')));assert.equal(nativeArgs.window,null);assert.equal(nativeArgs.percent,null);
await page.locator('[data-settings-tab="compaction"]').click();await page.locator('[name="compact_claude_trigger_percent"]').fill('80');await page.locator('[data-action="save-settings"]').click();await page.waitForFunction(()=>app.settings.compaction.claude.trigger_percent===80);
await page.screenshot({path:'/tmp/compaction-settings.png'});await page.locator('[data-action="close-dialog"]').last().click();await page.locator('#toggle-logs').click();await page.waitForFunction(()=>document.querySelector('#log-lines')?.textContent.includes('Compattazione completata'));await page.setViewportSize({width:390,height:900});await page.evaluate(()=>settingsDialog());await page.waitForFunction(()=>document.querySelector('#compact-binding')?.value==='agent');assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));assert.deepEqual(errors,[]);
console.log('PASS: native Claude/Codex/Pi compact protocols, threshold propagation, automatic/manual events, unchanged sessions, duplicate/run guards, redacted network failures, defaults/range validation, settings/UI and mobile');
}finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(dir,{recursive:true,force:true})}})().catch(e=>{console.error(e);process.exit(1)});
