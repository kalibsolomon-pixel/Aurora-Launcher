import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {openSettings,settingsModules} from './ui5-fixture.mjs';
const {chromium}=await import('file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs');
const browser=await chromium.launch({headless:true}),page=await browser.newPage({viewport:{width:1280,height:800},reducedMotion:'reduce'});
const checks=[];await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());
try {
 await openSettings(page);await page.getByRole('radio',{name:'Midnight',exact:true}).check();
 await page.getByText('Saved. Changes are live.',{exact:true}).waitFor();
 assert.equal((await settingsModules(page,'snapshot')).appearance.theme,'midnight');checks.push('Theme saves and renders native result');
 await settingsModules(page,'failure','set_appearance');await page.getByRole('radio',{name:'OLED Black',exact:true}).click();
 await page.getByRole('alert').waitFor();assert.equal(await page.getByRole('radio',{name:'Midnight',exact:true}).isChecked(),true);
 assert.equal(await page.getByText('Saved. Changes are live.',{exact:true}).count(),0);
 await settingsModules(page,'failure','');await page.getByRole('button',{name:'Retry save',exact:true}).click();
 await page.getByText('Saved. Changes are live.',{exact:true}).waitFor();assert.equal((await settingsModules(page,'snapshot')).appearance.theme,'oled');checks.push('Failed radio write restores authority, announces error and retries requested value');
 await page.getByLabel('Aurora motion',{exact:true}).fill('72');await page.getByLabel('Aurora motion',{exact:true}).dispatchEvent('change');
 await page.waitForFunction(()=>document.querySelector('output')?.textContent==='72 / 100');assert.equal((await settingsModules(page,'snapshot')).appearance.auroraMotionSpeed,72);checks.push('Motion value and native save stay synchronized');
 await page.getByRole('radio',{name:'Cyan',exact:true}).check();await page.waitForFunction(()=>document.documentElement.style.getPropertyValue('--color-accent')==='#5bd0e0');
 checks.push('Alternative accent reaches document tokens');
 await page.locator('.settings-nav').getByRole('button',{name:'Launcher',exact:true}).click();await page.getByRole('heading',{name:'Desktop integration',exact:true}).waitFor();checks.push('Launcher navigation preserves live desktop capability');
 await mkdir('docs/ui-5/pilot',{recursive:true});await writeFile('docs/ui-5/pilot/results.json',JSON.stringify({checks},null,2));console.log('PASS',checks.length,'pilot cases');
}finally{await browser.close();}
