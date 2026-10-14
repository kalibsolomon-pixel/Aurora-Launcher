import assert from 'node:assert/strict';
import {mkdir, writeFile} from 'node:fs/promises';
import {homeFixture, homeGeometry} from './ui2-fixture.mjs';
const {chromium} = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({headless:true});
const page = await browser.newPage({reducedMotion:'reduce',deviceScaleFactor:1});
const results = [], errors = [], measurements = [];
const out = process.env.AURORA_UI2_OUTPUT || 'docs/ui-2';
page.on('pageerror',e=>errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());
await mkdir(`${out}/states`,{recursive:true});
async function check(name, action) {await action(); results.push(name); console.log('PASS '+name);}
async function change(action) {
  await page.evaluate(async action=>{
    const source = await (await fetch('/Review.svelte')).text();
    const url = source.match(/from "([^"\n]*store.svelte[^"\n]*)"/)[1];
    const {launcher} = await import(url);
    if(action==='switch') {launcher.accountsState.selectedAccountId='d'.repeat(32); delete launcher.accountAvatars['d'.repeat(32)];}
    if(action==='no-widgets') {const u=source.match(/from "([^"\n]*homeLayout.svelte[^"\n]*)"/)[1]; const {homeWidgets}=await import(u); homeWidgets.layout={widgets:[]};}
    if(action==='hidden-slots') {const u=source.match(/from "([^"\n]*homeLayout.svelte[^"\n]*)"/)[1]; const {homeWidgets}=await import(u); homeWidgets.layout.widgets.splice(1,0,{id:'future-ui2',enabled:true,size:'wide'},{id:'session',enabled:false,size:'small'});}
    if(action==='stale-account') launcher.playReadiness={...launcher.playReadiness,accountId:'d'.repeat(32)};
    if(action==='stale-instance') launcher.playReadiness={...launcher.playReadiness,instanceId:'b'.repeat(32)};
  },action);
}
async function contained() {
  const g=await homeGeometry(page);
  assert.equal(g.documentOverflow||g.contentOverflow,false,JSON.stringify(g));
  assert.ok(g.account.bottom<=g.viewport[1]);
  assert.ok(g.play.x>=g.instance.x && g.play.right<=g.instance.right+1);
  for (let i=0;i<g.cards.length;i++) for(let j=i+1;j<g.cards.length;j++) {
    const a=g.cards[i],b=g.cards[j]; assert.ok(a.right<=b.x+1||b.right<=a.x+1||a.bottom<=b.y+1||b.bottom<=a.y+1,'widget overlap');
  }
  return g;
}
try {
  for(const [width,height] of [[1440,900],[1280,800],[1024,768],[900,700],[801,600],[720,520]]) {
    await check(`Hierarchy, external preview and containment ${width}x${height}`,async()=>{
      await page.setViewportSize({width,height}); await homeFixture(page); const g=await contained(); measurements.push(g);
      assert.equal(await page.locator('.instance-card .player-preview').count(),0);
      assert.equal(await page.locator('.instance-card .account-chip').count(),0);
      const heading=await page.locator('.instance-heading').boundingBox();
      assert.ok(heading.y+heading.height<=g.play.y+1);
      assert.equal(g.previewDisplay==='none',width<=800);
      if(width>800) {assert.ok(g.player.x>=g.instance.right); assert.ok(g.widgets.y>=g.player.bottom); assert.ok(g.canvas.height<=420);}
      if(width>=1024) assert.ok(g.cards[0].y<600,'widgets start within desktop view');
      if(width>=1024) {assert.ok(g.cards[1].x>g.cards[0].x);assert.ok(Math.abs(g.cards[1].y-g.cards[0].y)<1);}
      if(width===720) {assert.ok(Math.abs(g.cards[0].width-g.widgets.width)<2);assert.ok(g.cards[1].y>=g.cards[0].bottom);}
      assert.equal(await page.getByRole('button',{name:'Launch',exact:true}).isEnabled(),true);
      const dot=page.locator('.picker-trigger .readiness-dot');
      assert.equal(await dot.getAttribute('aria-label'),'Ready');
      assert.equal(await dot.textContent(),'');
      const nameBox=await page.locator('.picker-name').boundingBox(),dotBox=await dot.boundingBox();
      assert.ok(dotBox.x>=nameBox.x+nameBox.width && dotBox.x+dotBox.width<=g.instance.right);
      assert.equal(await page.locator('.instance-card .status-badge, .instance-footer').count(),0);
      await page.screenshot({path:`${out}/home-${width}x${height}.png`});
    });
  }
  await page.setViewportSize({width:1280,height:800});
  await check('Instance keyboard selection and simplified card',async()=>{
    await homeFixture(page);
    await page.getByRole('button',{name:'Select Play instance'}).focus(); await page.keyboard.press('ArrowDown'); await page.keyboard.press('End'); await page.keyboard.press('Enter');
    await page.waitForFunction(()=>document.querySelector('.picker-trigger').textContent.includes('Vanilla'));
    assert.equal(await page.locator('.instance-versions, .manage-instance').count(),0);
    assert.equal(await page.locator('.picker-trigger .readiness-dot').count(),1);
  });
  await check('Play calls existing native command once and shows Running',async()=>{
    await homeFixture(page);
    await page.locator('.play-hero').evaluate(button=>{button.click();button.click();button.click();});
    await page.waitForFunction(()=>document.querySelector('.play-hero').textContent==='Running');
    assert.equal(await page.locator('.play-hero').isDisabled(),true);
    const requests=await page.evaluate(()=>window.ui2Requests.filter(r=>r.command==='play_instance'));
    assert.equal(requests.length,1); assert.deepEqual(requests[0].request,{instanceId:'a'.repeat(32),accountId:'c'.repeat(32)});
  });
  await check('Play focus, hover, pressed and disabled treatments',async()=>{
    await homeFixture(page); const play=page.locator('.play-hero');
    const fill=()=>play.evaluate(e=>getComputedStyle(e).background);
    const resting=await fill();await play.focus();assert.equal(await play.evaluate(e=>getComputedStyle(e).outlineStyle),'solid');
    await play.hover();const hover=await fill();assert.notEqual(hover,resting);
    await page.mouse.down();assert.notEqual(await fill(),hover);await page.screenshot({path:`${out}/states/play-pressed.png`});await page.mouse.up();
    await homeFixture(page,'blocked');assert.ok(Number(await page.locator('.play-hero').evaluate(e=>getComputedStyle(e).opacity))<1);
  });
  await check('Checking, blocked, unavailable, Starting, Running and failed presentations',async()=>{
    for(const [state,label,disabled] of [['checking','Checking…',true],['blocked','Needs attention',true],['unavailable','Needs attention',true],['starting','Starting…',true],['running','Running',true],['failed','Ready',false]]) {
      await homeFixture(page,state);
      const dot=page.locator('.picker-trigger .readiness-dot');
      assert.equal(await dot.getAttribute('aria-label'),label);
      assert.equal(await dot.getAttribute('title'),label);
      assert.equal(await dot.evaluate(e=>e.classList.contains('ready')),['Ready','Running'].includes(label));
      assert.equal(await dot.evaluate(e=>getComputedStyle(e).backgroundColor),await page.evaluate(ready=>{const e=document.createElement('span');e.style.color=`var(--color-${ready?'success':'error'})`;document.body.append(e);const color=getComputedStyle(e).color;e.remove();return color;},['Ready','Running'].includes(label)));
      assert.equal(await page.locator('.play-hero').isDisabled(),disabled);
      if(state==='failed') assert.match(await page.locator('.instance-card [role=alert]').textContent(),/Synthetic launch failure/);
      if(state==='blocked') {assert.match(await page.locator('.instance-card').textContent(),/Synthetic sign-in required/);await page.screenshot({path:`${out}/blocked-1280x800.png`}); await page.getByRole('button',{name:'Check again'}).click();}
    }
    for(const state of ['stale-account','stale-instance']) {await homeFixture(page);await change(state);assert.equal(await page.locator('.play-hero').isDisabled(),true);}
  });
  await check('Instance management stays available in existing navigation; account entry intact',async()=>{
    await homeFixture(page);await page.getByRole('navigation',{name:'Aurora Launcher'}).getByRole('button',{name:'Instances',exact:true}).click();await page.locator('.instances-page').waitFor();
    await page.getByRole('navigation',{name:'Aurora Launcher'}).getByRole('button',{name:'Home',exact:true}).click();await page.locator('.home-page').waitFor();
    await page.locator('.account-chip').click();await page.getByRole('dialog').waitFor();await page.keyboard.press('Escape');
    assert.equal(await page.locator('.account-chip').evaluate(e=>e===document.activeElement),true);
  });
  await check('Player account isolation, manual controls and fallback',async()=>{
    await homeFixture(page);const before=await page.locator('.player-preview canvas').evaluate(c=>c.toDataURL());
    await page.getByRole('button',{name:'Rotate player left'}).click();const after=await page.locator('.player-preview canvas').evaluate(c=>c.toDataURL());assert.notEqual(before,after);
    await page.getByRole('button',{name:'Reset player view'}).click();assert.equal(await page.locator('.player-preview canvas').evaluate(c=>c.toDataURL()),before);
    await change('switch');assert.equal(await page.locator('.player-preview canvas').count(),0);
    await homeFixture(page,'missing-skin');assert.equal(await page.locator('.player-preview canvas').count(),0);await contained();
  });
  await check('Sparse, two, all widgets, mixed sizes and DOM order',async()=>{
    for(const state of ['sparse','normal','dense']) {
      await homeFixture(page,state);const g=await contained();
      assert.equal(g.cards.length,state==='sparse'?1:state==='normal'?2:8);
      if(state==='sparse') assert.ok(Math.abs(g.cards[0].width-g.widgets.width)<2);
      if(state==='dense') {assert.deepEqual(g.cards.map(c=>c.id),['instance-details','content-summary','session','playtime','recent-worlds','recent-servers','skin-manager','cape-selector']);assert.ok(g.cards[5].width>g.cards[0].width*1.8);}
    }
    await change('no-widgets');assert.equal(await page.locator('[data-home-widget]').count(),0);assert.equal(await page.getByRole('button',{name:'Edit Home widgets'}).isEnabled(),true);
    await page.setViewportSize({width:720,height:520}); await homeFixture(page,'dense'); const narrow=await contained();
    assert.ok(narrow.cards.every(c=>Math.abs(c.width-narrow.widgets.width)<2),'all saved spans clamp to one column');
    await page.setViewportSize({width:1280,height:800});
  });
  await check('Widget size/order/hide and persisted reload; unknown/hidden slots retained',async()=>{
    await homeFixture(page);await change('hidden-slots');await page.getByRole('button',{name:'Edit Home widgets'}).click();
    await page.getByRole('combobox',{name:'Playtime size'}).selectOption('wide');
    await page.getByRole('button',{name:'Move Playtime later'}).click();
    await page.getByRole('button',{name:'Finish editing Home'}).click();
    const saved=await page.evaluate(()=>JSON.parse(sessionStorage.getItem('phase-f-review')).layout);
    assert.deepEqual(saved.widgets.map(w=>w.id),['recent-servers','future-ui2','session','playtime']); assert.equal(saved.widgets[3].size,'wide');
    assert.deepEqual(saved.widgets[1],{id:'future-ui2',enabled:true,size:'wide'});assert.deepEqual(saved.widgets[2],{id:'session',enabled:false,size:'small'});
    await page.reload();await page.locator('[data-home-widget=playtime]').waitFor();
    assert.deepEqual(await page.locator('[data-home-widget]').evaluateAll(es=>es.map(e=>e.dataset.homeWidget)),['recent-servers','playtime']);
    assert.equal(await page.locator('[data-home-widget=playtime]').evaluate(e=>e.classList.contains('widget-wide')),true);
    await page.getByRole('button',{name:'Edit Home widgets'}).click();await page.locator('[data-home-widget=playtime]').getByRole('button',{name:'Hide',exact:true}).click();
    assert.equal(await page.getByRole('button',{name:'Finish editing Home'}).evaluate(e=>e===document.activeElement),true);
    await page.getByRole('button',{name:'Playtime',exact:true}).click();assert.equal(await page.locator('[data-home-widget=playtime]').count(),1);
  });
  await check('History valid, empty, unavailable and loading stay distinct',async()=>{
    for(const [state,text] of [['normal','13h 35m'],['empty','No playtime recorded yet.'],['error','Synthetic history unavailable.'],['loading','Loading playtime…']]) {
      await homeFixture(page,state);assert.ok((await page.locator('[data-home-widget=playtime]').textContent()).includes(text));
      if(state==='empty') assert.match(await page.locator('[data-home-widget=recent-servers]').textContent(),/No recently played servers/);
      if(state==='error') {
        assert.equal(await page.locator('.home-widgets [role=alert]').count(),2);
        assert.doesNotMatch(await page.locator('[data-home-widget=recent-servers]').textContent(),/No recently played/);
      }
    }
  });
  await check('Long names, 200% text, focus, reduced motion and live resizing',async()=>{
    for(const width of [1440,1024,900,720]) {
      await page.setViewportSize({width,height:768});await homeFixture(page,'long');await page.addStyleTag({content:'html{font-size:200% !important}'});await contained();
      await page.getByRole('button',{name:'Select Play instance'}).click();const b=await page.getByRole('listbox').boundingBox();assert.ok(b.x>=0&&b.x+b.width<=width);await page.keyboard.press('Escape');
      await page.getByRole('button',{name:'Launch',exact:true}).focus();assert.equal(await page.locator('.play-hero').evaluate(e=>getComputedStyle(e).outlineStyle),'solid');
      const dot=await page.locator('.readiness-dot').boundingBox(),name=await page.locator('.picker-name').boundingBox();assert.ok(dot.x>=name.x+name.width);
      await page.screenshot({path:`${out}/states/text-200-${width}.png`});
    }
    await homeFixture(page);for(const width of [1280,801,720,1024,1440]) {await page.setViewportSize({width,height:800});await contained();}
    assert.equal(await page.locator('.player-preview canvas').evaluate(e=>getComputedStyle(e).transitionDuration),'0s');
  });
  await check('OLED Simple and alternate cyan accent preserve surfaces and Play contrast',async()=>{
    await page.setViewportSize({width:1280,height:800});await homeFixture(page,'normal',{background:'simple',theme:'oled'});
    await page.evaluate(()=>{document.documentElement.dataset.accent='cyan';document.documentElement.style.setProperty('--color-accent-strong','#5bd0e0');document.documentElement.style.setProperty('--color-accent-contrast','#101116');});
    await contained();await page.screenshot({path:`${out}/states/oled-cyan.png`});
    const colors=await page.locator('.play-hero').evaluate(e=>{const s=getComputedStyle(e);return {color:s.color,fill:s.backgroundColor};});
    const l=c=>{const v=c.match(/[\d.]+/g).slice(0,3).map(Number).map(x=>{x/=255;return x<=.04045?x/12.92:((x+.055)/1.055)**2.4;});return v[0]*.2126+v[1]*.7152+v[2]*.0722;};
    const a=l(colors.color),b=l(colors.fill);assert.ok((Math.max(a,b)+.05)/(Math.min(a,b)+.05)>=4.5);
  });
  assert.deepEqual(errors,[]);
  await writeFile(`${out}/functional.json`,JSON.stringify({cases:results.length,results,errors,measurements,provenance:'Browser fixtures; synthetic DTOs and sessionStorage; no game launches or owner data'},null,2));
  console.log(`PASS ${results.length} UI-2 browser cases; zero page errors`);
} finally {await browser.close();}
