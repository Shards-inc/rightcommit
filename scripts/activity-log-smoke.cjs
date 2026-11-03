const fs=require('fs'),os=require('os'),path=require('path'),assert=require('assert'),{spawn}=require('child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{const dir=fs.mkdtempSync(path.join(os.tmpdir(),'activity-log-')),base='http://127.0.0.1:18108';const bin=path.join(dir,'bin');fs.mkdirSync(bin);
fs.writeFileSync(path.join(bin,'claude'),`#!/usr/bin/env python3
import os,sys,json,pathlib,time
sys.stdin.read();args=sys.argv;sid=args[args.index('--session-id')+1]
p=pathlib.Path(os.environ['HOME'])/'tmp/counter';n=int(p.read_text())+1 if p.exists() else 1;p.write_text(str(n))
def emit(x):print(json.dumps(x),flush=True)
emit(dict(type='system',subtype='init',session_id=sid))
for i in range(260 if n==1 else 1):
 emit(dict(type='assistant',message=dict(content=[dict(type='tool_use',id=str(i),name='Bash',input=dict(command='printf '+sid+' '+os.environ['ANTHROPIC_AUTH_TOKEN']))])))
 if n==2:time.sleep(4)
 emit(dict(type='user',message=dict(content=[dict(type='tool_result',tool_use_id=str(i),content='private tool output')])) )
emit(dict(type='result',result='<nodo_result>'+json.dumps(dict(status='completed',summary='Finished '+sid,artifacts=[],questions=[]))+'</nodo_result>',is_error=False))
`,{mode:0o755});
const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18108',NODO_SOURCES:path.resolve('sources'),PATH:bin+':'+process.env.PATH,ANTHROPIC_AUTH_TOKEN:'secret-activity-sentinel'},stdio:'ignore'});let browser;
try{for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage({viewport:{width:1440,height:1050}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
const api=async(p,b)=>{const r=await page.request.fetch(base+'/api'+p,{method:b?'POST':'GET',headers:{'X-Nodo-Request':'1'},data:b});assert(r.ok(),await r.text());return r.json()};
await api('/auth/register',{email:'activity@test.local',password:'temporary-activity-password'});
const wf={id:'activity-test',name:'Activity workflow',description:'Offline fixture',entry:'task',nodes:[{id:'source',kind:'Source',title:'Source',source_type:'example'},{id:'agent',kind:'Agent',title:'Claude',runtime:'claude'},{id:'task',kind:'Task',title:'Work',agent:'agent',prompt:'Offline log test'}],edges:[{from:'source',to:'agent',kind:'context'},{from:'agent',to:'task',kind:'context'}]};await api('/workflows',wf);
const first=await api('/runs',{workflow_id:wf.id});for(let i=0;i<200;i++){const d=await api('/runs/'+first.id);assert.notEqual(d.run.status,'failed',d.run.error);if(d.run.status==='completed')break;await new Promise(r=>setTimeout(r,50))}
const second=await api('/runs',{workflow_id:wf.id});await page.goto(base);await page.locator('#app').waitFor({state:'visible'});await page.evaluate(id=>loadRun(id),second.id);await page.locator('#toggle-logs').click();
await page.waitForFunction(()=>document.querySelector('.activity-current')?.textContent.includes('Bash'));assert(!(await page.locator('#log-lines').textContent()).includes('secret-activity-sentinel'));
await page.locator('[data-log-run="'+first.id+'"]').click();await page.waitForFunction(id=>activityLog.runs.get(id)?.loaded,first.id);assert(await page.locator('#activity-panel .activity-event').count()>500);
assert.equal(await page.evaluate(()=>app.run.id),second.id);assert.equal(await page.locator('[data-log-run]').count(),2);
await page.waitForFunction(()=>app.run.status==='completed');assert.equal(await page.locator('.activity-current').count(),0);
await page.locator('[data-log-run="'+second.id+'"]').click();const secondText=await page.locator('#activity-panel').textContent();assert(!secondText.includes('private tool output'));assert(secondText.includes('Bash'));assert(!secondText.includes('secret-activity-sentinel'));
// Compact details, persisted personal setting, accessible tabs and separated scrolling.
assert.equal(await page.locator('#log-lines details').count(),0);
assert.equal(await page.locator('[data-log-run][aria-selected="true"]').count(),1);
const longDetail='printf a deliberately long command with more than thirty characters\nsecond line';
const preview=await page.evaluate(detail=>activityMessage({kind:'activity',text:JSON.stringify({phase:'end',runtime:'claude',tool:'Bash',detail})}),longDetail);
assert(preview.includes(' · '+longDetail.slice(0,30)+'…'));assert(!preview.includes('second line'));
assert.equal(await page.evaluate(()=>activityDetail('😀'.repeat(31))), '😀'.repeat(30)+'…');
// Preview applies only to tool details; diagnostics and agent responses stay complete.
for(const kind of ['error','output','restart']){
 assert.equal(await page.evaluate(({kind,text})=>activityMessage({kind,text}),{kind,text:longDetail}),longDetail);
}
assert((await page.evaluate(detail=>activityMessage({kind:'activity',text:JSON.stringify({phase:'error',runtime:'pi',tool:'rag_status',detail})}),longDetail)).includes(longDetail));
assert((await page.evaluate(message=>activityMessage({kind:'compaction',text:JSON.stringify({phase:'error',runtime:'pi',message})}),longDetail)).includes(longDetail));
assert.equal(await page.evaluate(summary=>activityMessage({kind:'output',text:'<nodo_result>'+JSON.stringify({summary})+'</nodo_result>'}),longDetail),longDetail);
await page.evaluate(()=>settingsDialog('activity'));
await page.locator('[name="activity_log_page_size"]').fill('3');
await page.locator('[name="activity_log_verbosity"]').selectOption('full');
await page.locator('[data-action="save-settings"]').click();
await page.waitForFunction(()=>app.settings.activity_log_verbosity==='full');
assert.equal((await api('/settings')).activity_log_verbosity,'full');assert.equal((await api('/settings')).activity_log_page_size,3);
assert((await page.evaluate(detail=>activityMessage({kind:'activity',text:JSON.stringify({phase:'end',runtime:'claude',tool:'Bash',detail})}),longDetail)).includes(longDetail));
await page.locator('[data-action="close-dialog"]').first().click();
await page.reload();await page.locator('#app').waitFor({state:'visible'});
assert.equal(await page.evaluate(()=>app.settings.activity_log_verbosity),'full');assert.equal(await page.evaluate(()=>app.settings.activity_log_page_size),3);
await page.evaluate(()=>settingsDialog('activity'));await page.locator('[name="activity_log_verbosity"]').selectOption('preview');await page.locator('[data-action="save-settings"]').click();await page.waitForFunction(()=>app.settings.activity_log_verbosity==='preview');await page.locator('[data-action="close-dialog"]').first().click();
await page.evaluate(id=>loadRun(id),second.id);await page.evaluate(()=>logs(true));
await page.locator('[data-log-run="'+first.id+'"]').click();
await page.waitForFunction(id=>activityLog.runs.get(id)?.loaded,first.id);
await page.locator('[data-log-run="'+first.id+'"]').press('ArrowLeft');
assert.equal(await page.locator('[data-log-run="'+second.id+'"]').getAttribute('aria-selected'),'true');
await page.locator('[data-log-run="'+first.id+'"]').click();
await page.locator('#activity-panel').evaluate(el=>el.scrollTop=el.scrollHeight);
const geometry=await page.evaluate(()=>{const heading=document.querySelector('#logs .panel-heading').getBoundingClientRect(),tabs=document.querySelector('.activity-tabs').getBoundingClientRect(),content=document.querySelector('#activity-panel').getBoundingClientRect();return {heading:heading.bottom,tabsTop:tabs.top,tabsBottom:tabs.bottom,contentTop:content.top}});
assert(geometry.tabsTop>=geometry.heading-1);assert(geometry.contentTop>=geometry.tabsBottom-1);
// Events are single-line, newest first, including multiline full details.
const rows=await page.locator('.activity-event').evaluateAll(rows=>rows.map(r=>({seq:Number(r.dataset.logSeq),nowrap:getComputedStyle(r.querySelector('pre')).whiteSpace,top:r.querySelector('pre').getBoundingClientRect().top,metaTop:r.firstElementChild.getBoundingClientRect().top})));
assert(rows.length>500);assert(rows.every((r,i)=>r.nowrap==='nowrap'&&Math.abs(r.top-r.metaTop)<5&&(!i||rows[i-1].seq>r.seq)));
await page.screenshot({path:'/tmp/activity-log.png'});
await page.route(/\/api\/runs\/fixture-.*\/events/,route=>route.fulfill({json:[]}));
// A large metadata history renders only the configured number of tabs and one panel.
await page.evaluate(()=>{const now=Math.floor(Date.now()/1000)+1000;app.runs=Array.from({length:125},(_,i)=>({id:'fixture-'+String(i).padStart(4,'0'),workflow_id:app.run.workflow.id,name:'Synthetic history',created:now+i,status:'completed'}));for(const meta of app.runs){const entry=logRun(meta);entry.loaded=true}renderActivityLog()});
assert.equal(await page.locator('[data-log-run]').count(),2); // open older run on final page (101 entries, page size 3)
assert.equal(await page.locator('[role="tabpanel"]#activity-panel').count(),1);
assert(await page.evaluate(()=>activityLog.runs.size<=101));
await page.locator('[data-log-page="-1"]').click();
assert.equal(await page.locator('[data-log-run]').count(),3);
assert.equal(await page.evaluate(()=>app.run.id),second.id);
const invalid=await page.request.put(base+'/api/settings',{headers:{'X-Nodo-Request':'1'},data:{...(await api('/settings')),activity_log_page_size:0}});assert.equal(invalid.status(),400);
await page.setViewportSize({width:390,height:900});assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));assert.deepEqual(errors,[]);
console.log('PASS: newest-first single-line events, bounded configurable tabs, page-size validation and persistence, verbosity persistence, separate scrolling, live Claude command, redacted secrets, full history beyond 500 events, stable canvas context, completed tools and mobile');
}finally{await browser?.close();server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));fs.rmSync(dir,{recursive:true,force:true})}})().catch(e=>{console.error(e);process.exit(1)});
