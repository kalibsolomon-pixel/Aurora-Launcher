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
  const id = 'a'.repeat(32), accountId = 'c'.repeat(32);
  const account = { accountId, minecraftName: 'AuroraPlayer', status: 'signedIn' };
  const instance = { id, displayName: 'Aurora Client', state: 'ready', minecraftVersion: '1.21.11', platform: { kind: 'fabric', version: '0.19.5' }, aurora: { version: '2.1.5', channel: 'stable' }, auroraContentState: 'active', configuration: { minecraftVersion: '1.21.11', loader: { kind: 'fabric', policy: { type: 'pinned', version: '0.19.5' } }, auroraEnabled: true, memoryMib: 2048, additionalJvmArguments: '', window: null } };
  const fixtureLayout = { widgets: [{id:'recent-worlds',enabled:true,size:'small'}, {id:'playtime',enabled:true,size:'small'}, {id:'content-summary',enabled:false,size:'small'}, {id:'instance-details',enabled:false,size:'small'}] };
  const review = new URLSearchParams(location.search);
  setContext('borealis-review', { ...(review.has('time') ? {time: Math.max(0, Number(review.get('time')) || 0)} : {}), reducedMotion: review.has('reduced') });
  const restored = sessionStorage.getItem('phase-f-review');
  const saved = restored ? JSON.parse(restored) : null;
  launcher.launcherState = { config: { schemaVersion: 6, selectedInstanceId: id }, instances: [instance, {...instance, id:'b'.repeat(32), displayName:'Vanilla', platform:{kind:'vanilla'}, aurora:null, configuration:{...instance.configuration, loader:{kind:'vanilla'}, auroraEnabled:false}}], platformCapabilities: [] } as any;
  launcher.accountsState = { selectedAccountId: accountId, accounts: [account, {...account, accountId:'d'.repeat(32), minecraftName:'SecondPlayer'}] } as any;
  launcher.status = { launcherVersion: '1.2.0' } as any;
  launcher.playReadiness = { instanceId: id, accountId, ready: true, blockers: [], processStatus:'stopped' } as any;
  launcher.modInventories = { [id]: { instanceId:id, entries:['Aurora Client','Sodium','Fabric API','Lithium','Iris'].map((name,i)=>({entryId:String(i),displayName:name,fileName:name.toLowerCase().replaceAll(' ','-')+'.jar',enabled:i!==3,fileType:i===3?'disabledJar':'enabledJar',sizeBytes:1245000,modifiedUnixMillis:null,ownership:i===0?'auroraManaged':'userManaged',sha256:null,provenance:null,metadata:{id:name.toLowerCase(),version:'1.0.0',authors:['Visual review']},warnings:[],canToggle:i!==0,canRemove:i!==0,actionBlockedReason:null})),missingManaged:[] } } as any;
  launcher.contentInventories = Object.fromEntries(['resourcePack','shaderPack'].map(kind=>[`${id}:${kind}`,{instanceId:id,contentType:kind,missingManaged:[],entries:(kind==='resourcePack'?['Faithful','Fresh Animations']:['Complementary']).map((name,i)=>({entryId:kind+i,contentType:kind,displayName:name,fileName:name+'.zip',fileType:'zip',sizeBytes:4200000,modifiedUnixMillis:null,ownership:'userManaged',sha256:null,provenance:null,description:'Visual review fixture',packFormat:46,warnings:[],canRemove:true}))}])) as any;
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
  if (review.get('page') === 'instances') navigation.goTo('instances');
  if (review.has('tab')) navigation.openInstance(id, review.get('tab') as any);
  // Only this standalone review server aliases the native invoke boundary.
  setReviewInvoke(async (command:string,args:any) => {
    if (command === 'list_minecraft_versions') return [{id:'1.21.11',versionType:'release'}];
    if (command === 'list_fabric_loader_versions') return [{version:'0.19.5',stable:true}];
    if (command === 'get_instance_content_context') return {instanceId:id,minecraftVersion:'1.21.11',loader:'fabric',loaderVersion:'0.19.5',auroraVersion:'2.1.5',environment:'client',modrinthAvailable:true,modsLoadable:true};
    if (command === 'get_provider_lifecycle') return [];
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
    if (command === 'get_recent_servers') return [];
    if (command === 'get_discord_state' || command === 'connect_discord') return discord.state;
    if (command === 'set_discord_preferences') return {...discord.state,preferences:args.request};
    if (command === 'select_instance') { launcher.launcherState!.config.selectedInstanceId = args.request.instanceId; return launcher.launcherState; }
    if (command === 'select_account') { launcher.accountsState!.selectedAccountId = args.request.accountId; return launcher.accountsState; }
    if (command === 'get_launcher_state') return launcher.launcherState;
    if (command === 'get_accounts') return launcher.accountsState;
    if (command === 'get_account_avatar') return null;
    if (command === 'get_instance_runtime_status') return {instanceId:args.request.instanceId,status:'ready'};
    if (command === 'get_play_readiness') return {...launcher.playReadiness, instanceId:launcher.selectedInstance!.id,accountId:launcher.selectedAccount!.accountId};
    if (command === 'play_instance' || command === 'quick_play_history') throw {code:'visual_review_only',message:'Visual review fixture. No game was launched.'};
    throw {code:'visual_review_only',message:`Unavailable in visual review: ${command}`};
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
