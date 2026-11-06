// Disposable native app/browser performance regression. No agents or external writes.
// Run after cargo build: node scripts/large-source-smoke.cjs
// 3.3 GB: LARGE_SOURCE_BYTES=327680 node scripts/large-source-smoke.cjs
// Docker: also set LARGE_SOURCE_DOCKER=1 (uses disposable bind mounts).
// LARGE_SOURCE_SKIP_RESTART=1 skips the shared-continuation phase; LARGE_SOURCE_PORT
// and LARGE_SOURCE_BINARY select the test port and native executable.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const {spawn, spawnSync} = require('node:child_process');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rightcommit-large-source-'));
  const sources = path.join(dir, 'sources'), material = path.join(sources, 'large');
  fs.mkdirSync(material, {recursive: true});
  const bytesPerFile = Number(process.env.LARGE_SOURCE_BYTES || 3017);
  for (let i = 0; i < 10000; i++) {
    const folder = path.join(material, `part-${Math.floor(i / 100)}`);
    fs.mkdirSync(folder, {recursive: true});
    fs.writeFileSync(path.join(folder, `file-${i}.txt`), Buffer.alloc(i === 0 ? Math.min(1024, bytesPerFile) : bytesPerFile, 65 + i % 26));
  }
  const port = Number(process.env.LARGE_SOURCE_PORT || 18112);
  const base = `http://127.0.0.1:${port}`;
  const docker = process.env.LARGE_SOURCE_DOCKER === '1';
  const container = 'rightcommit-large-source-benchmark';
  fs.mkdirSync(path.join(dir, 'data'), {recursive:true});
  const server = docker ? spawn('docker', ['run','--rm','--name',container,
    '-p',`127.0.0.1:${port}:8080`,'-v',`${path.join(dir,'data')}:/data`,
    '-v',`${sources}:/sources`,'-e','NODO_ISOLATE_USERS=0',
    process.env.LARGE_SOURCE_IMAGE || 'rightcommit:local']) :
    spawn(path.resolve(process.env.LARGE_SOURCE_BINARY || 'target/debug/rightcommit'), [], {env: {...process.env,
      NODO_DATA: path.join(dir, 'data'), NODO_SOURCES: sources, NODO_ASSETS: process.cwd(),
      NODO_LISTEN: `127.0.0.1:${port}`, NODO_ISOLATE_USERS: '0', RUST_LOG: 'rightcommit=warn'}});
  let browser;
  const errors = [], results = [];
  server.stderr.on('data', () => {});
  server.stdout.on('data', () => {});
  try {
    let ready = false;
    for (let i = 0; i < 200; i++) {
      try { if ((await fetch(base + '/healthz')).ok) {ready = true; break;} } catch {}
      if (server.exitCode !== null) throw Error(`Test server exited: ${server.exitCode}`);
      await delay(100);
    }
    assert(ready, 'test server did not start');
    browser = await chromium.launch({channel: 'chrome', headless: true});
    const page = await browser.newPage({viewport: {width: 1440, height: 1000}});
    page.setDefaultTimeout(900000);
    page.on('pageerror', error => errors.push(error.message));
    const call = async (route, body, method) => {
      const response = await page.request.fetch(base + '/api' + route, {
        method: method || (body ? 'POST' : 'GET'), headers: {'X-Nodo-Request': '1'}, data: body,
        timeout: 900000});
      assert.equal(response.status(), 200, await response.text());
      return response.json();
    };
    await call('/auth/register', {email: 'large-source@test.local', password: 'temporary-large-source-password'});
    const workflow = {id: 'large-source', name: '10,000-file responsiveness', description: '', entry: 'read',
      nodes: [
        {id: 'source', kind: 'Source', title: '10,000 files', source_type: 'folder', source_mode: 'direct', location: 'large', x: 80, y: 100},
        {id: 'read', kind: 'Connection', title: 'Read one input', provider: 'local', operation: 'read_ticket', location: 'part-0/file-0.txt', x: 300, y: 100},
        {id: 'review', kind: 'Review', title: 'Human review', mode: 'human', x: 600, y: 100}],
      edges: [{from: 'source', to: 'read', kind: 'context'}, {from: 'read', to: 'review', kind: 'control'}]};
    workflow.nodes.push(
      {id:'again',kind:'Connection',title:'Read after host changes',provider:'local',operation:'read_ticket',location:'part-0/file-0.txt',x:800,y:100},
      {id:'done',kind:'Review',title:'Second review',mode:'human',x:1000,y:100});
    workflow.edges.push({from:'review',to:'again',kind:'control'},{from:'again',to:'done',kind:'control'});
    workflow.nodes.push(
      {id:'third',kind:'Connection',title:'Read second version',provider:'local',operation:'read_ticket',location:'part-0/file-0.txt',x:1200,y:100},
      {id:'final',kind:'Review',title:'Final review',mode:'human',x:1400,y:100});
    workflow.edges.push({from:'done',to:'third',kind:'control'},{from:'third',to:'final',kind:'control'});
    await call('/workflows', workflow);
    await page.goto(base);
    await page.locator('#app').waitFor({state: 'visible'});
    await page.evaluate(() => {window.largeSourceGaps = []; let last = performance.now();
      window.largeSourceTimer = setInterval(() => {const now = performance.now(); window.largeSourceGaps.push(now-last); last=now;}, 10);});
    async function measure(name, operation) {
      let done = false, failure, value;
      const times = [], started = performance.now();
      const task = Promise.resolve().then(operation).then(v => {value=v;}, error => {failure=error;}).finally(() => {done=true;});
      let index = 0;
      do {
        const before = performance.now();
        await call(['/auth/me', '/workflows', '/settings'][index++ % 3]);
        times.push(performance.now() - before);
        await delay(10);
      } while (!done);
      await task;
      if (failure) throw failure;
      times.sort((a,b) => a-b);
      const result = {name, operationMs: Math.round(performance.now()-started), probes: times.length,
        p95RequestMs: Math.round(times[Math.min(times.length-1, Math.floor(times.length*0.95))]),
        maxRequestMs: Math.round(times.at(-1))};
      results.push(result);
      console.log(JSON.stringify(result));
      assert(result.maxRequestMs < 1500, `${name}: unrelated API request stalled: ${JSON.stringify(result)}`);
      return value;
    }
    await measure('Source size (10,000 files)', () => call('/storage/source-size?root=sources&path=large'));
    let run = await measure('Run preparation and artifact scan (10,000 files)', async () => {
      const created = await call('/runs', {workflow_id: workflow.id});
      for (let i = 0; i < 15000; i++) {
        const live = await call('/runs/' + created.id);
        assert.notEqual(live.run.status, 'failed', live.run.error);
        if (live.run.status === 'waiting_review' && live.revision_available) return live;
        await delay(20);
      }
      throw Error('run did not reach human review');
    });
    assert.equal(run.artifacts.filter(a => a.file?.change !== 'report').length, 0, 'unchanged inputs became artifacts');
    // Deliberately modify from the host while the worker/watch is paused. This
    // exercises reconciliation independently of Docker forwarding notifications.
    const changed=Buffer.alloc(1024,90);
    fs.writeFileSync(path.join(material,'part-0/file-0.txt'),changed);
    fs.renameSync(path.join(material,'part-0/file-1.txt'),path.join(material,'part-0/renamed.txt'));
    fs.unlinkSync(path.join(material,'part-0/file-2.txt'));
    fs.writeFileSync(path.join(material,'part-0/new.txt'),'new output');
    const dataRoot=path.join(dir,'data','users','1','workspaces',run.run.id);
    fs.mkdirSync(dataRoot,{recursive:true});
    fs.writeFileSync(path.join(dataRoot,'data-output.txt'),'data output');
    run=await measure('Recover host source/data changes after pause', async()=>{
      await call('/runs/'+run.run.id+'/decide',{action:'approve',answer:'Test fixture approval'});
      for(let i=0;i<15000;i++){
        const live=await call('/runs/'+run.run.id);
        assert.notEqual(live.run.status,'failed',live.run.error);
        if(live.run.current==='done'&&live.run.status==='waiting_review'&&live.revision_available){
          const files=live.artifacts.filter(a=>a.file?.change!=='report');
          for(const name of ['file-0.txt','renamed.txt','new.txt','data-output.txt'])assert(files.some(a=>a.file.location.endsWith(name)),name);
          const first=files.find(a=>a.file.location.endsWith('file-0.txt'));
          const download=await page.request.get(base+'/api/artifacts/'+first.id+'/download');
          assert.equal(download.status(),200);assert.deepEqual(await download.body(),changed);
          assert(!files.some(a=>a.file.location.endsWith('file-2.txt')));
          return live;
        }
        await delay(20);
      }
      throw Error('host changes were not reconciled');
    });
    fs.writeFileSync(path.join(material,'part-0/file-0.txt'),Buffer.alloc(1024,89));
    await call('/runs/'+run.run.id+'/decide',{action:'approve',answer:'Test second version'});
    for(let i=0;i<15000;i++){
      const live=await call('/runs/'+run.run.id);
      assert.notEqual(live.run.status,'failed',live.run.error);
      if(live.run.current==='final'&&live.run.status==='waiting_review'&&live.revision_available){run=live;break;}
      await delay(20);
    }
    const versions=run.artifacts.filter(a=>a.file?.location.endsWith('file-0.txt')).sort((a,b)=>a.version-b.version);
    assert.equal(versions.length,2);assert.equal(versions[0].version,1);assert.equal(versions[1].version,2);
    assert.deepEqual(await (await page.request.get(base+'/api/artifacts/'+versions[0].id+'/download')).body(),changed);
    await page.evaluate(id => loadRun(id), run.run.id);
    await page.evaluate(()=>closeModal());
    await page.evaluate(id=>artifact(id),versions[1].id);
    await page.locator('[data-action=artifact-mode][data-mode=diff]').click();
    await page.locator('#dialog-content .diff-del').waitFor();
    assert.equal(await page.locator('#dialog-content .diff-del').count(),1);
    assert.equal(await page.locator('#dialog-content .diff-add').count(),1);
    assert.match(await page.locator('#dialog-content .diff-del').innerText(),/Z{20}/);
    assert.match(await page.locator('#dialog-content .diff-add').innerText(),/Y{20}/);
    await page.evaluate(()=>closeModal());

    await page.evaluate(() => closeModal());
    await page.locator('#edit-continue').click();
    await page.waitForFunction(() => !!app.editBase && !app.run);
    let preview;
    await measure('Restart preview and browser rendering (10,000 files)', async () => {
      const response = page.waitForResponse(r => r.url().endsWith('/revision/preview') && r.ok());
      await page.locator('#revision-preview').click();
      preview = await (await response).json();
      await page.locator('[data-revision-files=workspace]').waitFor();
    });
    assert.equal(preview.workspace_files.length, 10000);
    const list = page.locator('[data-revision-files=workspace]');
    assert.equal(await list.locator('li').count(), 0, 'closed list eagerly rendered files');
    await list.locator('summary').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-revision-files=workspace] li').length === 100);
    await list.locator('[data-revision-more]').click();
    assert.equal(await list.locator('li').count(), 200);
    assert.equal(await list.locator('[data-revision-more]').isVisible(), true);
    if (!process.env.LARGE_SOURCE_SKIP_RESTART) await measure('Continue same project with 10,000 files', async () => {
      const draft = await page.evaluate(() => structuredClone(app.wf));
      const plan = await call('/runs/' + run.run.id + '/revision/preview', {workflow: draft, node: 'read'});
      const fork = await call('/runs/' + run.run.id + '/revision', {
        workflow: draft, node: 'read', token: plan.token, acknowledge: true});
      assert.notEqual(fork.id, run.run.id);
      assert.equal(fork.workspace, run.run.workspace);
      assert(!fs.existsSync(path.join(dir,'data','users','1','workspaces',fork.id)), 'continuation created another workspace');
      assert(!fs.existsSync(path.join(material,'.git')), 'continuation initialized Git in direct Sources');
      assert.equal(fs.readFileSync(path.join(docker ? material : fork.workspace, 'part-0/file-0.txt'), 'utf8'),
        fs.readFileSync(path.join(material, 'part-0/file-0.txt'), 'utf8'));
      for (let i = 0; i < 15000; i++) {
        const live = await call('/runs/' + fork.id);
        assert.notEqual(live.run.status, 'failed', live.run.error);
        if (live.run.status === 'waiting_review' && live.revision_available) return;
        await delay(20);
      }
      throw Error('restarted run did not reach human review');
    });
    const maxBrowserGapMs = await page.evaluate(() => {clearInterval(window.largeSourceTimer); return Math.round(window.largeSourceGaps.reduce((a,b)=>Math.max(a,b),0));});
    assert(maxBrowserGapMs < 1000, `browser stalled for ${maxBrowserGapMs}ms`);
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({files: 10000, bytes: bytesPerFile * 10000, results, maxBrowserGapMs,
      previewRowsInitially: 0, previewRowsAfterOpen: 100, previewRowsAfterMore: 200, pageErrors: errors}, null, 2));
  } finally {
    if (browser) await browser.close();
    if (docker) spawnSync('docker',['rm','-f',container]);
    server.kill('SIGTERM');
    await new Promise(resolve => {if(server.exitCode !== null) resolve(); else server.once('exit', resolve);});
    fs.rmSync(dir, {recursive: true, force: true});
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
