// Verify real native Pi provider selection against an isolated OpenAI-compatible fixture.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),http=require('node:http'),{spawn}=require('node:child_process');
(async()=>{
 const cli=process.env.PI_CLI;assert(cli&&fs.existsSync(cli),'Set PI_CLI to the Pi CLI entry point');
 const data=fs.mkdtempSync(path.join(os.tmpdir(),'pi-multi-native-')),requests=[],base='http://127.0.0.1:18257';
 const mock=http.createServer(async(req,res)=>{
  let raw='';for await(const chunk of req)raw+=chunk;const body=JSON.parse(raw);requests.push({url:req.url,model:body.model,auth:req.headers.authorization});
  res.writeHead(200,{'Content-Type':'text/event-stream'});const event=(delta,finish_reason)=>({id:'chatcmpl-multi',object:'chat.completion.chunk',created:1,model:body.model,choices:[{index:0,delta,finish_reason}]});
  res.end('data: '+JSON.stringify(event({role:'assistant',content:'OK'},null))+'\n\ndata: '+JSON.stringify(event({},'stop'))+'\n\ndata: [DONE]\n\n');
 });await new Promise(resolve=>mock.listen(0,'127.0.0.1',resolve));const endpoint='http://127.0.0.1:'+mock.address().port;
 const quote=s=>"'"+s.replaceAll("'","'\\''")+"'",binary=path.join(data,'system/tools/bin/pi');fs.mkdirSync(path.dirname(binary),{recursive:true});fs.writeFileSync(binary,'#!/bin/sh\nexec '+quote(process.execPath)+' '+quote(cli)+' "$@"\n',{mode:0o700});
 const server=spawn(path.resolve('target/debug/rightcommit'),[],{env:{...process.env,NODO_DATA:data,NODO_LISTEN:'127.0.0.1:18257',OPENAI_API_KEY:'native-openai-key',XAI_API_KEY:'native-xai-key',PI_LOCAL_API_KEY:'native-local-key',PI_LOCAL_BASE_URL:endpoint+'/local/v1'},stdio:'ignore'});
 try{
  for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok)break}catch{}await new Promise(r=>setTimeout(r,100))}
  let cookie='';const api=async(p,method='GET',body)=>{const r=await fetch(base+'/api'+p,{method,headers:{'Content-Type':'application/json','X-Nodo-Request':'1',Cookie:cookie},body:body?JSON.stringify(body):undefined});const set=r.headers.get('set-cookie');if(set)cookie=set.split(';')[0];assert(r.ok,await r.clone().text());return r.json()};
  await api('/auth/register','POST',{email:'native-multi@test.local',password:'temporary-native-multi-password'});
  const home=path.join(data,'users/1/.pi/agent');fs.mkdirSync(home,{recursive:true});
  fs.writeFileSync(path.join(home,'models.json'),JSON.stringify({providers:{openai:{baseUrl:endpoint+'/openai/v1',api:'openai-completions',models:[{id:'gpt-fixture'}]},xai:{baseUrl:endpoint+'/xai/v1',api:'openai-completions',models:[{id:'grok-fixture'}]}}}));
  const settings=await api('/settings');await api('/settings','PATCH',{revision:settings._revision,changes:{agents:{pi:{multi_provider:true,provider:'openai',model:'gpt-fixture',api_key:'generic-key-must-not-be-used'}}}});
  const original=await api('/settings');
  for(const [provider,model,key] of [['openai','gpt-fixture','native-openai-key'],['xai','grok-fixture','native-xai-key'],['local','local-fixture','native-local-key']]){
   const result=await api('/agents/pi/status','POST',{provider,model,multi_provider:true,api_key:'draft-key-must-not-be-used'});assert(result.connected);assert.deepEqual(requests.at(-1),{url:'/'+provider+'/v1/chat/completions',model,auth:'Bearer '+key});assert.deepEqual(await api('/settings'),original);
  }
  const single=await api('/agents/pi/status','POST',{provider:'openai',model:'gpt-fixture',multi_provider:false,api_key:'single-draft-key'});assert(single.connected);assert.equal(requests.at(-1).auth,'Bearer single-draft-key');assert.deepEqual(await api('/settings'),original);
  console.log('PASS: real Pi 1.4 selects OpenAI/xAI/local with distinct environment keys, ignores generic/draft keys, uses the selected endpoint/model and does not save probe settings');
 }finally{server.kill('SIGINT');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));await new Promise(r=>mock.close(r));fs.rmSync(data,{recursive:true,force:true})}
})().catch(error=>{console.error(error);process.exit(1)});
