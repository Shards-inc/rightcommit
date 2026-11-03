// Usage: node scripts/sync-pi-providers.mjs /path/to/@earendil-works/pi-coding-agent
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const root=path.resolve(process.argv[2]||'node_modules/@earendil-works/pi-coding-agent');
const ai=path.join(root,'node_modules/@earendil-works/pi-ai/dist');
const {MODELS}=await import(pathToFileURL(path.join(ai,'models.generated.js')));
const {findEnvKeys}=await import(pathToFileURL(path.join(ai,'env-api-keys.js')));
const names=[...new Set(fs.readFileSync(path.join(ai,'env-api-keys.js'),'utf8').match(/\b[A-Z][A-Z0-9_]{3,}\b/g))];
const probe=Object.fromEntries(names.map(name=>[name,'present']));
const labels={anthropic:'Anthropic',openai:'OpenAI','openai-codex':'OpenAI Codex',xai:'xAI (Grok)',deepseek:'DeepSeek',google:'Google Gemini','google-vertex':'Google Vertex AI','amazon-bedrock':'Amazon Bedrock',azure:'Azure OpenAI / Foundry',nvidia:'NVIDIA NIM',openrouter:'OpenRouter','github-copilot':'GitHub Copilot',groq:'Groq',cerebras:'Cerebras',mistral:'Mistral',huggingface:'Hugging Face',together:'Together AI',fireworks:'Fireworks',baseten:'Baseten','cloudflare-ai-gateway':'Cloudflare AI Gateway','cloudflare-workers-ai':'Cloudflare Workers AI','vercel-ai-gateway':'Vercel AI Gateway',radius:'Radius',zai:'ZAI Coding Plan','kimi-coding':'Kimi For Coding',meta:'Meta'};
const extras={
 anthropic:['ANTHROPIC_FEDERATION_RULE_ID','ANTHROPIC_ORGANIZATION_ID','ANTHROPIC_SERVICE_ACCOUNT_ID','ANTHROPIC_IDENTITY_TOKEN_FILE','ANTHROPIC_WORKSPACE_ID'],
 azure:['AZURE_OPENAI_BASE_URL','AZURE_OPENAI_RESOURCE_NAME','AZURE_OPENAI_API_VERSION','AZURE_OPENAI_DEPLOYMENT_NAME_MAP','AZURE_CLIENT_ID','AZURE_TENANT_ID','AZURE_CLIENT_SECRET','AZURE_FEDERATED_TOKEN_FILE'],
 'amazon-bedrock':['AWS_REGION','AWS_DEFAULT_REGION','AWS_BEARER_TOKEN_BEDROCK','AWS_ACCESS_KEY_ID','AWS_SECRET_ACCESS_KEY','AWS_SESSION_TOKEN','AWS_PROFILE','AWS_CONFIG_FILE','AWS_SHARED_CREDENTIALS_FILE','AWS_WEB_IDENTITY_TOKEN_FILE','AWS_ROLE_ARN','AWS_ROLE_SESSION_NAME','AWS_CONTAINER_CREDENTIALS_RELATIVE_URI','AWS_CONTAINER_CREDENTIALS_FULL_URI','AWS_CONTAINER_AUTHORIZATION_TOKEN','AWS_CONTAINER_AUTHORIZATION_TOKEN_FILE'],
 'google-vertex':['GOOGLE_APPLICATION_CREDENTIALS','GOOGLE_CLOUD_PROJECT','GCLOUD_PROJECT','GOOGLE_CLOUD_LOCATION'],
 'cloudflare-workers-ai':['CLOUDFLARE_ACCOUNT_ID'],
 'cloudflare-ai-gateway':['CLOUDFLARE_ACCOUNT_ID','CLOUDFLARE_GATEWAY_ID']
};
const providers=Object.entries(MODELS).filter(([,models])=>Object.keys(models).length).map(([id])=>{
 const keys=findEnvKeys(id,probe)||[];
 return {id,label:labels[id]||id.split('-').map(word=>word[0].toUpperCase()+word.slice(1)).join(' '),key_env:keys.find(key=>key.endsWith('API_KEY'))||keys[0]||'',env:[...new Set([...keys,...extras[id]||[]])]};
});
providers.push({id:'local',label:'Locale / OpenAI compatibile',key_env:'PI_LOCAL_API_KEY',env:['PI_LOCAL_API_KEY','PI_LOCAL_BASE_URL']});
fs.writeFileSync('distribution/pi-providers.json',JSON.stringify({version:JSON.parse(fs.readFileSync(path.join(root,'package.json'))).version,providers},null,2)+'\n');
console.log(`Synced ${providers.length} chat providers`);
