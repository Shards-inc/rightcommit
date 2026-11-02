const assert=require('assert');
const path=require('path');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 try{
  // Deliberately use a browser locale different from the selected app language.
  const page=await browser.newPage({locale:'it-IT'});
  await page.route('http://validation.test/**',route=>route.fulfill({contentType:'text/html',body:'<html><body><select data-language><option>it</option><option>en</option></select><form><input id="required" required><input id="email" type="email"><input id="number" type="number" min="2" max="8" step="2"><input id="pattern" pattern="[A-Z]+"><input id="length" minlength="5" maxlength="8"><textarea id="text" required></textarea><select id="select" required><option value="">—</option><option value="ok">OK</option></select><button>Submit</button></form></body></html>'}));
  await page.goto('http://validation.test/');
  await page.addScriptTag({path:path.resolve('web/i18n.js')});
  const language=value=>page.selectOption('[data-language]',value);
  const message=id=>page.locator('#'+id).evaluate(field=>{field.reportValidity();return field.validationMessage});
  await language('en');
  assert.equal(await message('required'),'Please fill out this field.');
  await language('it');
  assert.equal(await page.locator('#required').evaluate(f=>f.validationMessage),'Compila questo campo.');
  await language('en');
  await page.locator('#required').fill('ok');
  assert(await page.locator('#required').evaluate(f=>f.checkValidity()));
  await page.locator('#email').fill('invalid');
  assert.equal(await message('email'),'Please enter a valid email address.');
  await page.locator('#email').fill('valid@example.org');
  assert(await page.locator('#email').evaluate(f=>f.checkValidity()));
  for(const [value,expected] of [['1','Please enter a value greater than or equal to 2.'],['9','Please enter a value less than or equal to 8.'],['3','Please enter a value that matches the allowed step.']]){
   await page.locator('#number').fill(value);assert.equal(await message('number'),expected);
  }
  await page.locator('#pattern').fill('abc');assert.equal(await message('pattern'),'Please match the requested format.');
  await page.locator('#length').pressSequentially('abc');assert.equal(await message('length'),'Please enter at least 5 characters.');
  assert.equal(await message('text'),'Please fill out this field.');
  assert.equal(await message('select'),'Please select an option.');
  await page.selectOption('#select','ok');assert(await page.locator('#select').evaluate(f=>f.checkValidity()));
  // Newly inserted forms use the same capture handler, including native submit.
  await page.evaluate(()=>{const form=document.createElement('form');form.id='dynamic';form.innerHTML='<input required><button>Go</button>';document.body.append(form)});
  await page.locator('#dynamic button').click();
  assert.equal(await page.locator('#dynamic input').evaluate(f=>f.validationMessage),'Please fill out this field.');
  await language('it');assert.equal(await page.locator('#dynamic input').evaluate(f=>f.validationMessage),'Compila questo campo.');
  await page.locator('#dynamic input').evaluate(f=>f.required=false);
  assert(await page.locator('#dynamic input').evaluate(f=>f.checkValidity()));
  await page.locator('form').first().evaluate(f=>f.reset());
  assert(await page.locator('#email').evaluate(f=>f.checkValidity()));
  await page.locator('#email').evaluate(f=>f.setCustomValidity('Application-specific error'));
  await language('en');assert.equal(await message('email'),'Application-specific error');
  console.log('PASS: IT/EN validation, language changes, corrected values, required fields, email, numeric bounds/step, pattern, length, dynamic forms, native submit, reset and custom errors');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exit(1)});
