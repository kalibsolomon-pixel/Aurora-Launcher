import assert from 'node:assert/strict';
import { before, after, it } from 'node:test';
import { createServer } from 'vite';
let server: Awaited<ReturnType<typeof createServer>>, CosmeticsStore: any;
const a='a'.repeat(32), b='b'.repeat(32);
const preset={id:'local',name:'Local skin',model:'classic',importedAt:1,sha256:'1'.repeat(64),favorite:false};
const profile=(id=a,cape='first')=>({accountId:id,currentSkinModel:'classic',hasCurrentSkin:true,currentSkin:null,capes:[{id:cape,name:cape,selected:true}]});
let handler: (name:string,args:any)=>Promise<any>;
before(async()=>{
  (globalThis as any).window={__TAURI_INTERNALS__:{invoke:(name:string,args:any)=>handler(name,args)}};
  server=await createServer({server:{middlewareMode:true},logLevel:'silent'});
  ({CosmeticsStore}=await server.ssrLoadModule('/src/lib/launcher/cosmetics.svelte.ts'));
});
after(async()=>{delete (globalThis as any).window; await server?.close();});
it('local library loads without an account and remote failures preserve it',async()=>{
  handler=async(name)=>{if(name==='list_skin_presets') return [preset];throw {code:'cosmetics_service_unavailable',message:'Offline profile'};};
  const store=new CosmeticsStore(); await store.load(null); await store.loadPresets();
  assert.equal(store.presets.length,1); assert.equal(store.remote,null);
  await store.load(a); assert.equal(store.offline,true);assert.match(store.remoteError,/Offline/);assert.equal(store.presets.length,1);assert.equal(store.libraryError,'');
});
it('overlapping library reads coalesce; an edit waits before publishing its result',async()=>{
  let resolve: any, reads=0;
  handler=async(name)=>{if(name==='list_skin_presets'){reads++;return new Promise(r=>resolve=r);}if(name==='update_skin_preset')return [{...preset,name:'Renamed'}];};
  const store=new CosmeticsStore(); const first=store.loadPresets();const second=store.loadPresets();const edit=store.rename('local','Renamed');resolve([preset]);
  await Promise.all([first,second,edit]);assert.equal(reads,1);assert.equal(store.presets[0].name,'Renamed');
});
it('A to B to A rejects a prior pending mutation, even when account identity matches again',async()=>{
  let resolve: any;
  handler=async(name,args)=>name==='list_skin_presets'?[preset]:name==='get_cosmetics'?profile(args.request.accountId,'fresh'):new Promise(r=>resolve=r);
  const store=new CosmeticsStore();await store.load(a);const changing=store.selectCape('first');await store.load(b);await store.load(a);resolve(profile(a,'obsolete'));await changing;
  assert.equal(store.remote.accountId,a);assert.equal(store.remote.capes[0].id,'fresh');assert.equal(store.remoteBusy,false);
});
it('wrong-account and out-of-order profile responses never become current',async()=>{
  let resolve: any;
  handler=async(name,args)=>name==='list_skin_presets'?[preset]:args.request.accountId===a?new Promise(r=>resolve=r):profile(b);
  const store=new CosmeticsStore();const pending=store.load(a);await store.load(b);resolve(profile(a));await pending;assert.equal(store.remote.accountId,b);
  handler=async()=>profile(a);await store.load(b,true);assert.equal(store.offline,true);assert.equal(store.remote.accountId,b);
});
it('mutation failure is stale, explicit retry refreshes, and duplicate actions invoke once',async()=>{
  let resolve: any,calls=0,fail=false;
  handler=async(name,args)=>{if(name==='list_skin_presets')return[preset];if(name==='get_cosmetics')return profile(args.request.accountId);calls++;if(fail)throw {code:'cosmetics_service_rejected',message:'Service rejected'};return new Promise(r=>resolve=r);};
  const store=new CosmeticsStore();await store.load(a);const first=store.disableCape();assert.equal(await store.disableCape(),false);resolve({...profile(),capes:[]});assert.equal(await first,true);assert.equal(calls,1);assert.equal(store.remote.capes.length,0);
  fail=true;await store.apply('local');assert.equal(store.offline,true);assert.equal(store.presets.length,1);assert.match(store.remoteError,/Service rejected/);await store.load(a,true);assert.equal(store.offline,false);assert.equal(store.remoteError,'');
});
