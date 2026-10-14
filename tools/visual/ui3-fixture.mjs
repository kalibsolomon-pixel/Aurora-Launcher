import {openReview} from './ui1-fixture.mjs';
export async function openContentReview(page, surface = 'mods', options = {}) {
  await openReview(page, 'home', options);
  await page.evaluate(async () => {
    const source = await (await fetch('/Review.svelte')).text();
    const module = name => source.match(new RegExp('from "([^"\\n]*' + name + '[^"\\n]*)"'))[1];
    const {launcher} = await import(module('store.svelte'));
    const {wrapReviewInvoke} = await import(module('review-core'));
    const names = {
      mod: ['Sodium', 'Lithium', 'Mod Menu', 'FerriteCore', 'Continuity', 'Entity Culling', 'ImmediatelyFast', 'Dynamic FPS'],
      modpack: ['Highland Adventures', 'Create & Explore', 'Quiet Horizons', 'Boreal Journeys', 'Vanilla Refined', 'Beyond the Valley', 'Building Together', 'New Frontiers'],
      resourcePack: ['Faithful 32x', 'Fresh Animations', 'Vanilla Tweaks', 'Boreal Textures', 'Better Leaves', 'Clear Glass', 'Soft Skies', 'Classic Details'],
      shaderPack: ['Complementary Reimagined', 'Soft Lighting', 'Distant Skies', 'Boreal Shaders'],
    };
    window.ui3Requests = []; window.ui3Mode = 'normal'; window.ui3InstallMode = 'success'; window.ui3Pending = null;
    const id = 'a'.repeat(32);
    const record = (kind, index) => ({contentType: kind, provider: 'modrinth', projectId: `${kind[0]}U${String(index).padStart(6,'0')}`, versionId:'v0000001', fileId:'f'.repeat(128), fileName:`fixture-${kind}-${index}.${kind === 'mod' ? 'jar' : 'zip'}`, sha256:'a'.repeat(64), displayVersion:'1.4.2', compatibility:{minecraftVersions:['1.21.11'],loader:kind === 'mod' ? 'fabric' : 'minecraft',environment:'client'}, dependencies:[],explicitlyRetained:index === 0,requires:[],origin:index === 0 ? 'direct' : 'dependency',installedAtUnixSeconds:1760000000,pinned:false,updateChannel:'stable'});
    const life = [record('mod',0),record('mod',1),record('resourcePack',0)].map(record => ({record,requiredBy:[],requires:[]}));
    const mods = launcher.modInventories[id];
    mods.entries = mods.entries.map((e, i) => ({...e, entryId:`mod-${i}`,ownership:i < 2 ? 'providerManaged' : 'userManaged',provenance:i < 2 ? life[i].record : null,canToggle:true,canRemove:true,removalBlockedReason:null,metadata:{...e.metadata,name:e.displayName,authors:['Fixture author'],depends:[],recommends:[],suggests:[],conflicts:[],breaks:[],environment:'client'},warnings:[]}));
    mods.entries[0].displayName = 'Sodium'; mods.entries[1].displayName = 'Lithium';
    mods.entries[2].displayName = 'A long installed mod title — 日本語の冒険と描画オプション '.repeat(2);
    mods.entries[2].metadata.version = '1.0.0+long-local-version';
    mods.entries[6].warnings = [{message:'Fixture metadata warning; review the full requirements before launch.'}];
    const packs = launcher.contentInventories[`${id}:resourcePack`];
    packs.entries[0].provenance = life[2].record;
    packs.entries[1].management.active = false;
    packs.entries[1].description = 'Local pack description with multilingual details 日本語 中文 العربية. '.repeat(10);
    const installed = new Set([life[0].record.projectId, life[2].record.projectId]);
    const images = {};
    const hits = Object.fromEntries(Object.entries(names).map(([kind, list]) => [kind,Array.from({length:24},(_,i) => {
      const projectId = `${kind[0]}U${String(i).padStart(6,'0')}`;
      const canvas = document.createElement('canvas'); canvas.width = canvas.height = 128;
      const g = canvas.getContext('2d'); g.fillStyle = ['#386c76','#735889','#51734c','#8d6848'][i % 4]; g.fillRect(0,0,128,128);
      g.fillStyle = '#edf1fb'; g.font = '600 52px system-ui'; g.textAlign='center';g.fillText(list[i % list.length][0],64,82);
      images[projectId] = i === 7 ? null : canvas.toDataURL();
      return {projectId, title:i === 9 ? 'A very long content title — 日本語の冒険と探索 '.repeat(4) : list[i % list.length] + (i >= list.length ? ' Plus' : ''), summary:kind === 'modpack' ? 'Explore a carefully assembled collection of worlds, building tools and adventures with friends.' : kind === 'resourcePack' ? 'Refresh the look of your worlds with clear, thoughtfully crafted textures and details.' : 'Improves Minecraft while preserving the familiar look and feel of your worlds.', author:'Fixture author', downloads:1250000 + i*11800, categories:[i % 2 ? 'utility' : 'adventure'],projectType:kind,iconUrl:null};
    })]));
    wrapReviewInvoke(previous => async (command, args) => {
      window.ui3Requests.push({command,request:structuredClone(args?.request ?? args)});
      const r = args?.request ?? {};
      if (command === 'get_provider_lifecycle') return life;
      if (command === 'resolve_project_artwork') return {source:images[r.projectId] ?? null,status:images[r.projectId] ? 'available' : 'unavailable',retryAfterMs:null};
      if (command === 'resolve_installed_artwork') return {projects:{},retryAfterMs:null};
      if (command === 'browse_modrinth_tags') return [{name:'adventure',projectType:'mod'},{name:'utility',projectType:'mod'},{name:'adventure',projectType:'modpack'},{name:'utility',projectType:'resourcepack'}];
      if (command === 'browse_modrinth') {
        if (window.ui3Mode === 'loading') await new Promise(resolve => window.ui3Pending = resolve);
        if (window.ui3Mode === 'error') throw {code:'provider_network_error',message:'Fixture: Modrinth is unavailable. Try again.'};
        const found = window.ui3Mode === 'empty' ? [] : hits[r.contentType].filter(h => h.title.toLowerCase().includes(r.search.toLowerCase()) && (!r.categories.length || r.categories.some(c => h.categories.includes(c))));
        if (r.sort === 'downloads') found.reverse();
        return {offset:r.offset,totalHits:found.length,hits:found.slice(r.offset,r.offset + 20)};
      }
      if (command === 'quick_install_modrinth') {
        if (window.ui3InstallMode === 'pending') await new Promise(resolve => window.ui3Pending = resolve);
        if (window.ui3InstallMode === 'error') throw {code:'provider_integrity_failure',message:'Fixture verification failed.'};
        if (window.ui3InstallMode === 'blocked') throw {code:'provider_content_collision',message:'Fixture: an existing local file conflicts with this project.'};
        if (installed.has(r.projectId)) return [];
        installed.add(r.projectId);
        const rec = {...record(r.contentType,Number(r.projectId.slice(2))),explicitlyRetained:true,origin:'direct'};
        const existing = life.find(l => l.record.projectId === r.projectId);
        if (existing) existing.record = rec; else life.push({record:rec,requiredBy:[],requires:[]});
        if (r.contentType === 'mod') {
          const matching = mods.entries.find(e => e.provenance?.projectId === r.projectId);
          if (matching) matching.provenance = rec;
          else mods.entries.push({...mods.entries[3],entryId:`installed-${r.projectId}`,displayName:hits.mod.find(h => h.projectId === r.projectId).title,ownership:'providerManaged',provenance:rec});
        }
        return [rec];
      }
      if (command === 'get_modrinth_browse_project') {
        const h = hits[r.contentType].find(h => h.projectId === r.projectId);
        return {projectId:r.projectId,title:h.title,summary:h.summary,license:'MIT',gameVersions:['1.21.11'],loaders:['fabric'],environments:['client'],versions:[{id:'v0000001',name:'Reviewed version',versionNumber:'1.4.2',versionType:'release',datePublished:'2026-01-01T00:00:00Z',environment:'client',loaders:['fabric']}],defaultVersionId:'v0000001',projectType:r.contentType};
      }
      if (command === 'preview_modrinth_pack') return {projectId:r.projectId,versionId:r.versionId,name:'Highland Adventures',packVersion:'1.4.2',minecraftVersion:'1.21.11',fabricLoaderVersion:'0.19.5',recognized:[],unresolved:['mods/example.jar'],optional:['resourcepacks/optional.zip'],excluded:[],overrides:['config/example.json'],fingerprint:'fixture-pack-fingerprint'};
      if (command === 'install_modrinth_pack') throw {code:'provider_network_error',message:'Fixture: pack installation failed safely. Review and retry.'};
      if (command === 'set_instance_mod_enabled') {const entry=mods.entries.find(e=>e.entryId===r.entryId);entry.enabled=r.enabled;return structuredClone(mods);}
      if (command === 'set_instance_pack_enabled') {const entry=packs.entries.find(e=>e.entryId===r.entryId);entry.management.active=r.enabled;return structuredClone(packs);}
      return previous(command,args);
    });
    window.ui3SetLibrary = state => {
      if (state === 'empty') {mods.entries=[]; packs.entries=[];}
      if (state === 'error') launcher.modError = {code:'mod_inventory_unavailable',message:'Fixture inventory unavailable.'};
    };
  });
  if (surface === 'modpacks') {
    await page.locator('.rail-link[aria-label="Instances"]').click();
    await page.getByRole('button',{name:'Browse Modpacks',exact:true}).click();
  } else {
    const tab = surface.includes('packs') ? 'resourcePacks' : 'mods';
    await page.evaluate(async tab => {
      const source=await(await fetch('/Review.svelte')).text();
      const {navigation}=await import(source.match(/from "([^"\n]*navigation.svelte[^"\n]*)"/)[1]);
      navigation.openInstance('a'.repeat(32),tab);
    },tab);
    if (!surface.startsWith('installed')) await page.getByRole('button',{name:'Browse Modrinth',exact:true}).click();
  }
  if (!surface.startsWith('installed')) await page.locator('.browse-row').first().waitFor();
  else await page.locator(surface.includes('packs') ? '.pack-row' : '.mod-row').first().waitFor();
  await page.locator('main.content').evaluate(e=>e.scrollTop=0);
  await page.waitForTimeout(200);
}

export async function contentGeometry(page) {
  return page.evaluate(() => {
    const main=document.querySelector('main.content');
    const box=e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom};};
    const list=document.querySelector('.browse-list');
    return {viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,documentOverflow:document.documentElement.scrollWidth>innerWidth,contentOverflow:main.scrollWidth>main.clientWidth+1,columns:list ? getComputedStyle(list).gridTemplateColumns : null,rows:[...document.querySelectorAll('.browse-row,.mod-row,.pack-row')].slice(0,5).map(box),controls:[...document.querySelectorAll('.browse-search input,.browse-search .btn,.filter-input')].map(box),filters:document.querySelector('.browse-filters') ? box(document.querySelector('.browse-filters')) : null};
  });
}
