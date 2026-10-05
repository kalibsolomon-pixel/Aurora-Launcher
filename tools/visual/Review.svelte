<script lang="ts">
  import '../../src/app.css';
  import { setContext } from 'svelte';
  import { setReviewInvoke, isTauri, nativeInvoke } from './review-core';
  import AppShell from '$lib/shell/AppShell.svelte';
  import Home from '$lib/pages/HomePage.svelte';
  import Settings from '$lib/pages/SettingsPage.svelte';
  import Instances from '$lib/pages/InstancesPage.svelte';
  import InstanceWorkspace from '$lib/instances/InstanceWorkspace.svelte';
  import { launcher } from '$lib/launcher/store.svelte';
  import { navigation } from '$lib/launcher/navigation.svelte';
  import { appearance } from '$lib/launcher/appearance.svelte';
  import { homeWidgets } from '$lib/launcher/homeLayout.svelte';
  import { discord } from '$lib/launcher/discord.svelte';
  import packageMetadata from '../../package.json';
  const id = 'a'.repeat(32), accountId = 'c'.repeat(32);
  const review = new URLSearchParams(location.search);
  // Phase H update-review fixtures: real provider records and a typed
  // availability report for the Content workspace Updates card.
  const phaseH = review.has('updates');
  const zeroUpdates = review.has('zero');
  const provenance = (project: string, version: string, display: string, origin: string, pinned = false, channel: 'stable'|'beta'|'alpha' = 'stable') => ({
    contentType: 'mod', provider: 'modrinth', projectId: project, versionId: version, fileId: 'f'.repeat(128),
    fileName: `${display}.jar`, sha256: 'a'.repeat(64), displayVersion: display,
    compatibility: { minecraftVersions: ['1.21.11'], loader: 'fabric', environment: 'client' },
    dependencies: [], explicitlyRetained: true, requires: [], origin, installedAtUnixSeconds: 1760000000, pinned, updateChannel: channel,
  });
  const lifecycleFixtures: any[] = phaseH ? [
    { record: provenance('AAAA0001', 'aaaa0002', 'Sodium 1.4.2', 'direct'), requiredBy: [], requires: [] },
    { record: provenance('BBBB0002', 'bbbb0002', 'Mod Menu 7.0.0', 'recovered', true), requiredBy: [], requires: [] },
    { record: provenance('CCCC0003', 'cccc0002', 'FerriteCore 8.0.0', 'direct'), requiredBy: [], requires: [] },
    { record: { ...provenance('DEPD0009', 'depd0010', 'Cloth Config 21.11.153', 'dependency', false), explicitlyRetained: false }, requiredBy: [], requires: [] },
  ] : [];
  if (phaseH) lifecycleFixtures[3].requiredBy = [lifecycleFixtures[0].record];
  const account = { accountId, minecraftName: 'AuroraPlayer', status: 'signedIn' };
  const instance = { id, displayName: 'Aurora Client', state: 'ready', minecraftVersion: '1.21.11', platform: { kind: 'fabric', version: '0.19.5' }, aurora: { version: '2.1.5', channel: 'stable' }, auroraContentState: 'active', configuration: { minecraftVersion: '1.21.11', loader: { kind: 'fabric', policy: { type: 'pinned', version: '0.19.5' } }, auroraEnabled: true, memoryMib: 2048, additionalJvmArguments: '', window: null } };
  // Hierarchy-review fixtures: ?instances=N seeds 0..4 instances so the
  // Instances page can be reviewed empty, single and multi-instance.
  const instanceRoster = [
    instance,
    { ...instance, id: 'b'.repeat(32), displayName: 'Vanilla', platform: { kind: 'vanilla' }, aurora: null, configuration: { ...instance.configuration, loader: { kind: 'vanilla' }, auroraEnabled: false } },
    { ...instance, id: 'e'.repeat(32), displayName: 'Creative Sandbox', configuration: { ...instance.configuration, loader: { kind: 'fabric', policy: { type: 'automatic' } } } },
    { ...instance, id: 'f'.repeat(32), displayName: 'Snapshot Trials', platform: { kind: 'vanilla' }, aurora: null, configuration: { ...instance.configuration, minecraftVersion: '1.21.11', loader: { kind: 'vanilla' }, auroraEnabled: false } },
  ];
  const instanceCount = Math.max(0, Math.min(instanceRoster.length, Number(review.get('instances') ?? 2) || 0));
  const seededInstances = instanceRoster.slice(0, instanceCount);
  const fixtureLayout = { widgets: [{id:'recent-worlds',enabled:true,size:'small'}, {id:'playtime',enabled:true,size:'small'}, {id:'content-summary',enabled:false,size:'small'}, {id:'instance-details',enabled:false,size:'small'}] };
  if (review.has('servers')) fixtureLayout.widgets = [{id:'recent-servers',enabled:true,size:'small'}, {id:'playtime',enabled:true,size:'small'}];
  setContext('borealis-review', { ...(review.has('time') ? {time: Math.max(0, Number(review.get('time')) || 0)} : {}), reducedMotion: review.has('reduced') });
  const restored = sessionStorage.getItem('phase-f-review');
  const saved = restored ? JSON.parse(restored) : null;
  launcher.launcherState = { config: { schemaVersion: 6, selectedInstanceId: instanceCount > 0 ? id : null }, instances: seededInstances, platformCapabilities: [
    { kind: 'vanilla', canCreate: true, canInstall: true, canValidate: true, canLaunch: true, auroraSupported: false },
    { kind: 'fabric', canCreate: true, canInstall: true, canValidate: true, canLaunch: true, auroraSupported: true },
  ] } as any;
  launcher.accountsState = { selectedAccountId: accountId, accounts: [account, {...account, accountId:'d'.repeat(32), minecraftName:'SecondPlayer'}] } as any;
  launcher.status = { launcherVersion: packageMetadata.version } as any;
  launcher.playReadiness = { instanceId: id, accountId, ready: true, blockers: [], processStatus:'stopped' } as any;
  const modEntry = (name: string, i: number, prov: any = null) => ({ entryId: String(i), displayName: name, fileName: prov ? prov.fileName : name.toLowerCase().replaceAll(' ', '-') + '.jar', enabled: i !== 3, fileType: i === 3 ? 'disabledJar' : 'enabledJar', sizeBytes: 1245000, modifiedUnixMillis: null, ownership: prov ? 'providerManaged' : (i === 0 ? 'auroraManaged' : 'userManaged'), sha256: null, provenance: prov, metadata: { id: name.toLowerCase(), version: prov ? prov.displayVersion : '1.0.0', authors: ['Visual review'] }, warnings: [], canToggle: i !== 0, canRemove: i !== 0, actionBlockedReason: null });
  const modNames = ['Aurora Client','Sodium','Fabric API','Lithium','Iris','Mod Menu','FerriteCore'];
  launcher.modInventories = { [id]: { instanceId:id, entries: modNames.map((name,i)=>modEntry(name, i, phaseH && i === 1 ? lifecycleFixtures[0].record : phaseH && i === 2 ? lifecycleFixtures[3].record : phaseH && i === 5 ? lifecycleFixtures[1].record : phaseH && i === 6 ? lifecycleFixtures[2].record : null)), missingManaged: [] } } as any;
  const managedPackRecord = provenance('SHDR0001', 'shdr0002', 'Complementary r5.9.3', 'direct');
  managedPackRecord.contentType = 'shaderPack'; managedPackRecord.fileId = 'f'.repeat(128); managedPackRecord.fileName = 'ComplementaryReimagined_r5.9.3.zip'; managedPackRecord.sha256 = 'a'.repeat(64);
  const managedResourceRecord = provenance('RESR0001', 'resr0002', 'Faithful 32x 1.21', 'direct');
  managedResourceRecord.contentType = 'resourcePack'; managedResourceRecord.fileName = 'Faithful 32x.zip';
  launcher.contentInventories = Object.fromEntries(['resourcePack','shaderPack'].map(kind=>[`${id}:${kind}`,{instanceId:id,contentType:kind,missingManaged:[],entries:(kind==='resourcePack'?[['Faithful 32x', managedResourceRecord],['Fresh Animations', null]]:[['Complementary Reimagined', managedPackRecord]]).map(([name, prov],i)=>({entryId:kind+i,contentType:kind,displayName:name,fileName:(prov ? prov.fileName : name+'.zip'),fileType:'zip',sizeBytes:4200000,modifiedUnixMillis:null,ownership:prov?'providerManaged':'userManaged',sha256:null,provenance:prov,description:'Visual review fixture',packFormat:46,warnings:[],canRemove:true}))}])) as any;
  for (const inventory of Object.values(launcher.contentInventories)) for (const entry of inventory.entries) {
    entry.management = { active: inventory.contentType === 'resourcePack' ? true : null, canToggle: inventory.contentType === 'resourcePack', toggleBlockedReason: null,
      activationManagedInGame: inventory.contentType === 'shaderPack', canRemove: true, removalPath: entry.provenance ? 'providerGraph' : 'localFile', removalBlockedReason: null };
  }
  if (review.has('skin')) launcher.accountAvatars[accountId] = { rgba: Array(256).fill(200), model: 'classic', skinHeight: 64, skinRgba: Array(64*64*4).fill(180) } as any;
  launcher.minecraftVersions = [{id:'1.21.11',versionType:'release'}];
  launcher.createMinecraftVersion = '1.21.11';

  homeWidgets.layout = saved?.layout ?? fixtureLayout;
  const themes = [{id:'aurora-dark',label:'Aurora Dark'},{id:'midnight',label:'Midnight'},{id:'oled',label:'OLED Black'}];
  const accents = ['violet','blue','cyan','green','amber','rose','neutral'].map((id,i)=>({id,label:['Aurora violet','Blue','Cyan','Green','Amber','Rose','Neutral'][i],hex:['#8b80ff','#7ba4ff','#5bd0e0','#5ec98f','#e2b655','#ef8fa5','#dfe3ee'][i]}));
  appearance.apply({auroraMotionSpeed:50,theme:'aurora-dark',background:'borealis',accent:{type:'preset',id:'violet'},themes,accents,palette:{accent:'#8b80ff',accentStrong:'#6f5df2',accentHover:'#6f63e8',accentPressed:'#6152e0',accentContrast:'#ffffff',accentSoft:'rgba(139,128,255,.14)',accentOutline:'rgba(139,128,255,.55)'},...saved?.appearance, ...(review.has('speed') ? {auroraMotionSpeed:Math.max(0,Math.min(100,Number(review.get('speed')) || 0))} : {}), ...(review.has('theme') ? {theme:review.get('theme')} : {}), ...(review.has('background') ? {background:review.get('background')} : {})} as any);
  if (appearance.accent.type === 'custom' && !appearance.state?.palette.accent) appearance.apply({...appearance.state!,accent:{type:'preset',id:'violet'}});
  discord.state = {configured:true,connection:'connected',preferences:{enabled:false,instanceName:false,minecraftVersion:false,platform:false,auroraActive:false,elapsedTime:false,world:false,server:false,serverAddress:false}} as any;
  if (review.has('edit')) homeWidgets.editing = true;
  if (review.get('page') === 'settings') navigation.goTo('settings');
  if (review.get('category') === 'General' || review.get('category') === 'Discord & privacy') navigation.settingsCategory = review.get('category') as 'General' | 'Discord & privacy';
  if (review.get('page') === 'instances') navigation.goTo('instances');
  if (review.has('tab')) navigation.openInstance(id, review.get('tab') as any);
  // Only this standalone review server aliases the native invoke boundary.
  setReviewInvoke(async (command:string,args:any) => {
    try {
    if (command === 'list_minecraft_versions') return [{id:'1.21.11',versionType:'release'}];
    if (command === 'list_fabric_loader_versions') return [{version:'0.19.5',stable:true}];
    if (command === 'get_instance_content_context') return {instanceId:id,minecraftVersion:'1.21.11',loader:'fabric',loaderVersion:'0.19.5',auroraVersion:'2.1.5',environment:'client',modrinthAvailable:true,modsLoadable:true};
    if (command === 'get_provider_lifecycle') return lifecycleFixtures;
    if (command === 'list_instance_mods') return launcher.modInventories[id];
    if (command === 'list_instance_content') return launcher.contentInventories[`${id}:${args.contentType ?? args.request?.contentType}`];
    if (command === 'get_home_widgets') return homeWidgets.layout;
    if (command === 'set_home_widgets' || command === 'reset_home_widgets') { const layout = command === 'set_home_widgets' ? args.request : fixtureLayout; sessionStorage.setItem('phase-f-review',JSON.stringify({layout,appearance:appearance.state})); return layout; }
    if (command === 'set_appearance') { const selected = args.request.accent; const hex = selected.type === 'custom' ? selected.hex : accents.find(a=>a.id===selected.id)?.hex ?? '#8b80ff'; const next = isTauri() ? await nativeInvoke<any>('set_appearance',args) : {...appearance.state,...args.request,palette:{accent:hex,accentStrong:hex,accentHover:hex,accentPressed:hex,accentContrast:'#101116',accentSoft:hex+'24',accentOutline:hex+'88'}}; sessionStorage.setItem('phase-f-review',JSON.stringify({layout:homeWidgets.layout,appearance:next})); return next; }
    if (command === 'get_playtime_summary' && review.has('empty')) return {allTimeMs:0,last7DaysMs:0,last30DaysMs:0};
    if (command === 'get_recent_worlds' && review.has('empty')) return [];
    if (command === 'get_playtime_summary') return {allTimeMs:192600000,last7DaysMs:48900000,last30DaysMs:148320000};
    if (command === 'get_daily_playtime') return [30,80,45,110,55,90,25].map((minutes,i)=>({day:20718+i,durationMs:minutes*60000}));
    if (command === 'get_recent_worlds') return [{id:'fixture-world',displayName:'Highland retreat',instanceId:id,available:true,lastPlayedAt:Date.now()/1000},{id:'fixture-world-2',displayName:'Creative workshop',instanceId:id,available:true,lastPlayedAt:Date.now()/1000-86400}];
    if (command === 'get_recent_servers') return review.has('servers') ? ['mcpvp.club','minemen.club','pvphq.com'].map((displayName,i)=>({id:String(i).repeat(64),displayName,instanceId:id,available:true,lastPlayedAt:Date.now()/1000})) : [];
    if (command === 'refresh_recent_server_status') return [];
    if (command === 'get_discord_state' || command === 'connect_discord') return discord.state;
    if (command === 'get_desktop_integration') return { supported:true, manageable:false, desktopShortcut:{state:'present'}, startMenuShortcut:{state:'present'} };
    // Native-boundary fixtures only: a valid static PNG, a rejected null, or a
    // deliberately malformed DTO to exercise the renderer's last-resort fallback.
    if (command === 'get_modrinth_project_artwork') return review.has('validated-artwork')
      ? 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAN0lEQVR4nO3QQREAMAgDQYpC5CC2XspUBZ+Ngcvs6bovFpebcQcIECBAgAABAgQIECBAgACBLzCTbALj8Oz2OgAAAABJRU5ErkJggg=='
      : review.has('broken-artwork') ? 'data:image/png;base64,AAAA' : null;
    if (command === 'set_discord_preferences') return {...discord.state,preferences:args.request};
    if (command === 'select_instance') { launcher.launcherState!.config.selectedInstanceId = args.request.instanceId; return launcher.launcherState; }
    if (command === 'select_account') { launcher.accountsState!.selectedAccountId = args.request.accountId; return launcher.accountsState; }
    if (command === 'get_launcher_state') return launcher.launcherState;
    if (command === 'get_accounts') return launcher.accountsState;
    if (command === 'get_account_avatar') return null;
    if (command === 'get_instance_runtime_status') return {instanceId:args.request.instanceId,status:'ready'};
    if (command === 'get_play_readiness') return {...launcher.playReadiness, instanceId:launcher.selectedInstance!.id,accountId:launcher.selectedAccount!.accountId};
    if (command === 'browse_modrinth') {
      const browseKeyType: string = args.request.contentType;
      const hitsFor: Record<string, any[]> = {
        mod: [1,2,3,4].map((n)=>({ projectId:'AAAA000'+n, title:['Sodium','Lithium','Iris','Mod Menu'][n-1], summary:'A visual review fixture project with a short description.', author:'Visual review', downloads: 1200000*n, iconUrl:null, projectType:'mod', categories:['performance','rendering'] })),
        modpack: [1,2].map((n)=>({ projectId:'PACK000'+n, title:['Fabulously Optimized','Simply Optimized'][n-1], summary:'A curated modpack fixture for browsing only.', author:'Visual review', downloads: 900000*n, iconUrl:null, projectType:'modpack', categories:['optimization'] })),
        resourcePack: [1,2].map((n)=>({ projectId:'RESR000'+n, title:['Faithful 32x','Fresh Animations'][n-1], summary:'A resource pack fixture.', author:'Visual review', downloads: 400000*n, iconUrl:null, projectType:'resourcePack', categories:['32x'] })),
        shaderPack: [1,2].map((n)=>({ projectId:'SHDR000'+n, title:['Complementary Reimagined','BSL Shaders'][n-1], summary:'A shader pack fixture.', author:'Visual review', downloads: 700000*n, iconUrl:null, projectType:'shaderPack', categories:['fantasy','cartoon'] })),
      };
      const hits = hitsFor[browseKeyType] ?? [];
      // Deliberately failed provider image for the shared component's browser regression.
      if (review.has('broken-artwork') && hits.length) hits[0].iconUrl = 'https://cdn.modrinth.com/data/AAAA0001/icon.png';
      return { offset: args.request.offset, totalHits: hits.length, hits };
    }
    if (command === 'get_modrinth_project') {
      const kind = args.request.contentType;
      const versionLists: Record<string, any[]> = {
        modpack: [
          { id:'pack0002', name:'v9.2.1', versionNumber:'9.2.1', versionType:'release', datePublished:'2026-09-01', environment:'client', loaders:['fabric'] },
          { id:'pack0003', name:'v9.3.0-beta', versionNumber:'9.3.0-beta', versionType:'beta', datePublished:'2026-09-20', environment:'client', loaders:['fabric'] },
        ],
        mod: [ { id:'aaaa0009', name:'Sodium 1.5.0', versionNumber:'1.5.0', versionType:'release', datePublished:'2026-09-01', environment:'client', loaders:['fabric'] } ],
      };
      return { projectId: args.request.projectId, title: 'Fabulously Optimized', summary: 'A curated modpack fixture.', license: 'MIT', gameVersions: ['1.21.11'], loaders: ['fabric'], environments: ['client'], versions: versionLists[kind] ?? versionLists.mod, defaultVersionId: (versionLists[kind] ?? versionLists.mod)[0]?.id ?? null, projectType: kind };
    }
    if (command === 'browse_modrinth_tags') {
      // Provider-shaped fixture data: real Modrinth tags arrive as lowercase
      // slug names attributed to a project type, including multi-word and
      // hyphen-compound entries that exercise the picker's presentation.
      const tagSet = (names: string[], projectType: string) => names.map((name) => ({ name, projectType }));
      return [
        ...tagSet(['adventure','cursed','decoration','economy','equipment','food','game-mechanics','library','magic','management','minigame','mobs','optimization','social','storage','technology','transportation','utility','worldgen'], 'mod'),
        ...tagSet(['adventure','challenging','combat','exploration','hardcore','kitchen-sink','magic','multiplayer','progression','questing','technology','vanilla-like'], 'modpack'),
        ...tagSet(['8x','16x','32x','64x','128x','256x','512x','animated','core-shaders','decoration','font','game-mechanics','gui','hardcore','medieval','modern','movie','realistic','semi-realistic','vanilla-like'], 'resourcepack'),
        ...tagSet(['atmosphere','cartoon','colored-lighting','foliage','path-tracing','pixelated','potato','realistic','reflection','semi-realistic','vanilla-like'], 'shader'),
      ];
    }
    if (command === 'get_aurora_compatibility') {
      return { available: true, reason: 'Aurora 2.1.5 supports Minecraft 1.21.11 with Fabric Loader 0.19.5.', version: '2.1.5', loaderVersion: '0.19.5' };
    }
    if (command === 'check_instance_updates') {
      if (!phaseH) throw {code:'visual_review_only',message:'Add ?updates=1 for update fixtures.'};
      if (zeroUpdates) return { instanceId: id, dependencyManaged: 1, entries: [
        { contentType:'mod', projectId:'AAAA0001', currentVersion:'1.4.2', status:'upToDate', block:null, candidate:null, pinned:false, channel:'stable', detail:null },
        { contentType:'mod', projectId:'BBBB0002', currentVersion:'7.0.0', status:'noNewerUnderPolicy', block:null, candidate:null, pinned:false, channel:'stable', detail:'Newer versions exist but none is allowed by the Stable release-channel policy for this content.' },
      ] };
      return { instanceId: id, dependencyManaged: 1, entries: [
        { contentType:'mod', projectId:'AAAA0001', currentVersion:'1.4.2', status:'updateAvailable', block:null, candidate:{id:'aaaa0009',name:'Sodium 1.5.0',versionNumber:'1.5.0',versionType:'release',datePublished:'2026-09-01',environment:'client_and_server',loaders:['fabric']}, pinned:false, channel:'stable', detail:null },
        { contentType:'mod', projectId:'BBBB0002', currentVersion:'7.0.0', status:'pinnedUpdateAvailable', block:null, candidate:{id:'bbbb0009',name:'Mod Menu 7.1.0-beta',versionNumber:'7.1.0-beta',versionType:'beta',datePublished:'2026-09-02',environment:'client_and_server',loaders:['fabric']}, pinned:true, channel:'stable', detail:null },
        { contentType:'mod', projectId:'CCCC0003', currentVersion:'8.0.0', status: review.has('mixed') ? 'noNewerUnderPolicy' : 'updateAvailable', block:null, candidate: review.has('mixed') ? null : {id:'cccc0009',name:'FerriteCore 8.1.0',versionNumber:'8.1.0',versionType:'release',datePublished:'2026-09-03',environment:'client_and_server',loaders:['fabric']}, pinned:false, channel:'stable', detail: review.has('mixed') ? 'Newer versions exist but none is allowed by the Stable release-channel policy for this content.' : null },
        { contentType:'mod', projectId:'DDDD0004', currentVersion:'2.0.0', status:'upToDate', block:null, candidate:null, pinned:false, channel:'stable', detail:null },
        { contentType:'mod', projectId:'EEEE0005', currentVersion:'3.0.0', status:'blocked', block:'locallyModified', candidate:null, pinned:false, channel:'stable', detail:'The local file changed since Aurora recorded it. Aurora will not overwrite local modifications; use Recognize local files or restore the file first.' },
      ] };
    }
    if (command === 'set_provider_update_policy') {
      const record = lifecycleFixtures.find((entry:any)=>entry.record.projectId === args.request.projectId)?.record;
      if (!record) throw {code:'content_changed_since_scan',message:'Visual review fixture: unknown project.'};
      if (args.request.pinned !== undefined) record.pinned = args.request.pinned;
      if (args.request.channel !== undefined) record.updateChannel = args.request.channel;
      return record;
    }
    if (command === 'preview_modrinth_bulk_update' || command === 'preview_modrinth_update') {
      const targets = command === 'preview_modrinth_update' ? [args.request] : args.request.targets;
      const recordFor = (project:string)=>lifecycleFixtures.find((entry:any)=>entry.record.projectId === project)?.record ?? lifecycleFixtures[0].record;
      const candidateVersion = (project:string)=> project === 'AAAA0001' ? '1.5.0' : '7.1.0-beta';
      const candidateType = (project:string)=> project === 'AAAA0001' ? 'release' : 'beta';
      return {
        targets: targets.map((t:any)=>({ current:{contentType:'mod', provider:'modrinth', projectId:t.projectId, fileName:recordFor(t.projectId).fileName, displayVersion:recordFor(t.projectId).displayVersion}, candidate:{id:'c'+t.projectId, name:'Update', versionNumber:candidateVersion(t.projectId), versionType:candidateType(t.projectId), datePublished:'2026-09-01', environment:'client', loaders:['fabric']}, changelog: ['## Changes', '', '- Improved frame pacing on wide screens.', '- Fixed a crash when reloading shaders mid-world.', '', 'Provider-authored release notes rendered as plain text.'].join('\n'), channel: recordFor(t.projectId).updateChannel })),
        delta: { willInstall: targets.map((t:any)=>({contentType:'mod', provider:'modrinth', projectId:t.projectId, fileName:candidateVersion(t.projectId)+'.jar', displayVersion:candidateVersion(t.projectId)})), willRemove: targets.map((t:any)=>({contentType:'mod', provider:'modrinth', projectId:t.projectId, fileName:recordFor(t.projectId).fileName, displayVersion:recordFor(t.projectId).displayVersion})), willRetain: [], newRequirements: [{contentType:'mod', provider:'modrinth', projectId:'FFFF0006', fileName:'shared-lib-2.1.0.jar', displayVersion:'2.1.0'}], removedRequirements: [] },
        warnings: ['Optional dependency Iris is not installed automatically.'],
        previewFingerprint: 'visual-review',
      };
    }
    if (command === 'apply_modrinth_bulk_update' || command === 'apply_modrinth_update') {
      // Simulated success: bump fixture records so the updated state renders.
      const targets = command === 'apply_modrinth_update' ? [args.request] : args.request.targets;
      for (const t of targets) {
        const record = lifecycleFixtures.find((entry:any)=>entry.record.projectId === t.projectId)?.record;
        const display = t.projectId === 'AAAA0001' ? 'Sodium 1.5.0' : t.projectId === 'CCCC0003' ? 'FerriteCore 8.1.0' : 'Mod Menu 7.1.0';
        if (record) { record.versionId = record.versionId + 'u'; record.displayVersion = display; }
        const row = (launcher.modInventories[id] as any)?.entries?.find((entry:any)=>entry.provenance?.projectId === t.projectId);
        if (row) { row.metadata = { ...row.metadata, version: display }; row.fileName = display + '.jar'; }
      }
      return;
    }
    if (command === 'check_modrinth_update') {
      const record = lifecycleFixtures.find((entry:any)=>entry.record.projectId === args.request.projectId)?.record;
      if (!record) throw {code:'content_changed_since_scan',message:'Visual review fixture: unknown project.'};
      if (record.pinned) return {id:'x1',name:'Update',versionNumber:'1.5.0',versionType:'release',datePublished:'2026-09-01',environment:'client',loaders:['fabric']};
      return args.request.projectId === 'AAAA0001' ? {id:'aaaa0009',name:'Sodium 1.5.0',versionNumber:'1.5.0',versionType:'release',datePublished:'2026-09-01',environment:'client',loaders:['fabric']} : null;
    }
    if (command === 'preview_provider_removal' || command === 'apply_provider_removal') throw {code:'visual_review_only',message:'Visual review fixture.'};
    if (command === 'play_instance' || command === 'quick_play_history') throw {code:'visual_review_only',message:'Visual review fixture. No game was launched.'};
    throw {code:'visual_review_only',message:`Unavailable in visual review: ${command}`};
    } catch (error) {
      if (error && typeof error === 'object' && 'code' in (error as any)) throw error;
      throw {code:'review_handler_error',message:`Review fixture failed for ${command}: ${String(error)}`};
    }
  });
</script>
<AppShell>
  {#if navigation.state.kind === 'instance'}<InstanceWorkspace instanceId={navigation.state.instanceId} tab={navigation.state.tab} />
  {:else if navigation.state.kind === 'global' && navigation.state.page === 'settings'}<Settings />
  {:else if navigation.state.kind === 'global' && navigation.state.page === 'instances'}<Instances />

  {:else}<Home />{/if}
</AppShell>
<div class="review-label">PHASE F · VISUAL FIXTURES</div>
<style>.review-label { position:fixed; top:12px; left:50%; transform:translateX(-50%); pointer-events:none; color:#b2bdc9; font:9px system-ui; letter-spacing:.15em; z-index:45; }</style>
