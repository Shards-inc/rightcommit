const assert=require('node:assert/strict');
const fs=require('node:fs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 try{
  const page=await browser.newPage();
  await page.setContent('<dialog><div class="modal-body"></div><div class="modal-actions"><button class="primary" type="button">Save</button></div></dialog>');
  const source=fs.readFileSync('web/app.js','utf8');
  await page.evaluate(source=>{
   window.$$=(selector,root=document)=>[...root.querySelectorAll(selector)];
   eval(source.slice(source.indexOf('function submitSingleTextDialog('),source.indexOf('function modal('))+'\nwindow.handler=submitSingleTextDialog;');
   const dialog=document.querySelector('dialog');dialog.onkeydown=window.handler;dialog.showModal();
   window.clicks=0;document.querySelector('.primary').onclick=()=>window.clicks++;
  },source);
  async function check(body,expected,{disabled=false,key='Enter'}={}){
   await page.evaluate(({body,disabled})=>{document.querySelector('.modal-body').innerHTML=body;document.querySelector('.primary').disabled=disabled;window.clicks=0},{body,disabled});
   await page.locator('#field').focus();await page.keyboard.press(key);
   assert.equal(await page.evaluate(()=>window.clicks),expected,body+' / '+key);
  }
  await check('<input id="field" value="Name">',1);
  await check('<form><input id="field" required></form>',0);
  await check('<form><input id="field" required value="Name"><input type="hidden" value="id"></form>',1);
  await check('<input id="field"><input>',0);
  await check('<input id="field"><textarea></textarea>',0);
  await check('<input id="field"><select><option>A</option></select>',0);
  await check('<input id="field"><jui-select style="display:block">Choice</jui-select>',0);
  await check('<input id="field"><jui-switch style="display:block">Toggle</jui-switch>',0);
  await check('<textarea id="field"></textarea>',0);
  await check('<input id="field" readonly>',0);
  await check('<input id="field">',0,{disabled:true});
  await check('<input id="field">',0,{key:'Shift+Enter'});
  await check('<input id="field">',1);
  await page.evaluate(()=>{window.clicks=0;const f=document.querySelector('#field');for(const options of [{isComposing:true},{repeat:true}])f.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true,cancelable:true,...options}));});
  assert.equal(await page.evaluate(()=>window.clicks),0);
  // A primary submit button must submit once, with native validation preserved.
  await page.evaluate(()=>{
   document.querySelector('.modal-body').innerHTML='<form id="rename"><input id="field" required value="Renamed"></form>';
   const button=document.querySelector('.primary');button.type='submit';button.setAttribute('form','rename');
   window.submits=0;document.querySelector('form').onsubmit=e=>{e.preventDefault();window.submits++};
  });
  await page.locator('#field').press('Enter');
  assert.equal(await page.evaluate(()=>window.submits),1);
  console.log('PASS: single text dialog submission, native forms, validation, multiple controls, readonly, disabled, modifiers, composition and repeat');
 }finally{await browser.close()}
})().catch(error=>{console.error(error);process.exit(1)});
