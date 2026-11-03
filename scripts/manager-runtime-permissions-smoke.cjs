// Exercise the real HTTP manager and native adapters with deterministic CLI stand-ins.
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),assert=require('node:assert/strict'),{spawn}=require('node:child_process');
const {request}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'manager-permissions-')),bin=path.join(dir,'system/tools/bin'),base='http://127.0.0.1:18161';fs.mkdirSync(bin,{recursive:true});
 const fixture=`#!/usr/bin/env python3
import json,sys,pathlib,uuid
rt=pathlib.Path(sys.argv[0]).name
args=sys.argv[1:]
prompt=sys.stdin.read()
key='resume' if rt=='codex' else '--resume' if rt=='claude' else '--session-id'
sid=args[args.index(key)+1] if key in args else args[args.index('--session-id')+1] if '--session-id' in args else str(uuid.uuid4())
with open('permissions-trace.jsonl','a') as f:f.write(json.dumps({'runtime':rt,'args':args,'prompt':prompt,'sid':sid})+'\\n')
result='<nodo_result>'+json.dumps({'status':'completed','summary':'Checked adapter','details':json.dumps({'workflow':None}),'questions':[],'artifacts':[],'decision':'explain'})+'</nodo_result>'
if rt=='codex':
 print(json.dumps({'type':'thread.started','thread_id':sid}))
 print(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':result}}))
elif rt=='claude':
 print(json.dumps({'type':'system','subtype':'init','session_id':sid}))
 print(json.dumps({'type':'result','result':result}))
else:
 print(json.dumps({'type':'session','id':sid}))
 print(json.dumps({'type':'message_end','message':{'role':'assistant','content':[{'type':'text','text':result}]}}))
`;
 for(const rt of ['codex','claude','pi'])fs.writeFileSync(path.join(bin,rt),fixture,{mode:0o755});
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{cwd:dir,env:{...process.env,PATH:bin+path.delimiter+process.env.PATH,NODO_DATA:dir,NODO_LISTEN:'127.0.0.1:18161',NODO_SOURCES:path.resolve('sources')},stdio:'ignore'});
 let api;try{
 for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
 api=await request.newContext({baseURL:base,extraHTTPHeaders:{'X-Nodo-Request':'1'}});
 const call=async(method,url,data)=>{const r=await api.fetch('/api'+url,{method,data});assert.equal(r.status(),200,await r.text());return r.json()};
 await call('POST','/auth/register',{email:'permissions@test.local',password:'temporary-permissions-password'});
 let settings=await call('GET','/settings');
 const patch=async changes=>{settings=await call('PATCH','/settings',{revision:settings._revision,changes})};
 for(const rt of ['codex','claude','pi']){
  const workflow={id:'permissions-'+rt,name:'Permissions '+rt,description:'Adapter permission regression',nodes:[],edges:[],entry:''};
  let sid;
  for(const enabled of [true,false,true,false]){
   await patch({default_runtime:rt,manager_write_enabled:enabled,agents:{[rt]:{model:'test-model'}}});
   const response=await call('POST','/manager/chat',{workflow,message:'Check permissions '+enabled});assert(!response.error,JSON.stringify(response));
   const userDirs=fs.readdirSync(path.join(dir,'users'));
   const traces=userDirs.flatMap(user=>{const root=path.join(dir,'users',user,'manager');return fs.existsSync(root)?fs.readdirSync(root).map(chat=>path.join(root,chat,'permissions-trace.jsonl')).filter(f=>fs.existsSync(f)):[]});
   const turns=traces.flatMap(f=>fs.readFileSync(f,'utf8').trim().split('\n').map(JSON.parse)).filter(t=>t.runtime===rt);
   const turn=turns.at(-1);assert(turn,JSON.stringify(response));
   const value=key=>turn.args[turn.args.indexOf(key)+1];
   if(sid){assert.equal(turn.sid,sid);if(rt!=='pi')assert(turn.args.includes(rt==='codex'?'resume':'--resume'))}sid=turn.sid;
   assert(turn.prompt.includes(enabled?'Current permissions: read, write':'Current permissions: READ ONLY'));
   if(rt==='codex'){assert.equal(value('--sandbox'),enabled?'workspace-write':'read-only');assert(!turn.args.includes('--disable'));if(!enabled)assert(turn.prompt.includes('Shell remains available for read-only inspection'))}
   if(rt==='claude'){
    if(enabled){assert(!turn.args.includes('--tools'));assert(value('--allowedTools').includes('Bash'))}
    else {assert.equal(value('--tools'),'Read,Glob,Grep');assert.equal(value('--allowedTools'),'Read,Glob,Grep')}
   }
   if(rt==='pi'){
    if(enabled)for(const tool of ['+write','+edit','+bash'])assert(value('--tools').split(',').includes(tool));
    else assert.equal(value('--tools'),'read,grep,find,ls');
   }
  }
  console.log('PASS: '+rt+' same native session, enabled → disabled → enabled → disabled');
 }
 }finally{await api?.dispose();server.kill();fs.rmSync(dir,{recursive:true,force:true})}
})().catch(e=>{console.error(e);process.exit(1)});
