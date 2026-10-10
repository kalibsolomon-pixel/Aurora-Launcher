import assert from "node:assert/strict";
import { after, before, it } from "node:test";
import { createServer } from "vite";
let server: Awaited<ReturnType<typeof createServer>>;
let render: any, Skin: any, Cape: any, launcher: any, cosmetics: any, backend: any;
const a = "a".repeat(32), b = "b".repeat(32);
const presets = Array.from({length:80},(_,i)=>({id:`preset-${i}`,name:i===0?'Long skin name '.repeat(5):`Saved ${i}`,model:i%2?'slim':'classic',favorite:i===0,importedAt:i,sha256:String(i).padStart(64,'0')}));
function account(id: string | null) {
  launcher.accountsState={selectedAccountId:id,accounts:id?[{accountId:id,minecraftName:'Player',status:'signedIn'}]:[]}; cosmetics.accountId=id;
}
function state(id=a,active=true,count=3) {
  return {accountId:id,currentSkinModel:'classic',hasCurrentSkin:true,currentSkin:null,capes:Array.from({length:count},(_,i)=>({id:`cape-${i}`,name:`Cape ${i}`,selected:active&&i===0,preview:null}))};
}
function html(component:any,size='small') {return render(component,{props:{size}}).body;}
before(async()=>{
  server=await createServer({server:{middlewareMode:true},logLevel:'silent'});
  ({render}=await server.ssrLoadModule('svelte/server'));
  Skin=(await server.ssrLoadModule('/src/lib/launcher/widgets/SkinManagerWidget.svelte')).default;
  Cape=(await server.ssrLoadModule('/src/lib/launcher/widgets/CapeSelectorWidget.svelte')).default;
  ({launcher}=await server.ssrLoadModule('/src/lib/launcher/store.svelte.ts'));
  ({cosmetics}=await server.ssrLoadModule('/src/lib/launcher/cosmetics.svelte.ts'));
  backend=await server.ssrLoadModule('/src/lib/backend.ts');
});
after(async()=>{await server?.close();});
it('Home skin summary bounds an eighty-entry library and prioritizes favorites',()=>{
  account(a);cosmetics.presets=presets;cosmetics.remote=state();cosmetics.loading=false;cosmetics.offline=false;cosmetics.libraryError='';cosmetics.remoteError='';
  const compact=html(Skin),large=html(Skin,'large');
  assert.equal((compact.match(/<li\b/g)??[]).length,2);assert.equal((large.match(/<li\b/g)??[]).length,4);
  assert.match(compact,/Long skin name/);assert.match(compact,/Saved 79/);assert.match(compact,/80 saved/);
  assert.match(compact,/Current skin · Classic/);assert.match(compact,/Open Skin Library/);
  assert.doesNotMatch(compact,/Remove from library|>Apply<|Import skin model/);
});
it('no account and offline states retain local previews and navigation',()=>{
  account(null);cosmetics.remote=null;assert.match(html(Skin),/without an account/);
  account(a);cosmetics.offline=true;assert.match(html(Skin),/Saved skins remain available/);assert.match(html(Skin),/Open saved skin/);
  assert.match(html(Cape),/Inventory unavailable/);assert.match(html(Cape),/changes are paused/);
});
it('cape summary shows current choice, owned count, No Cape, and empty inventory',()=>{
  account(a);cosmetics.remote=state();cosmetics.offline=false;
  const view=html(Cape);assert.match(view,/Cape 0/);assert.match(view,/3 owned/);assert.match(view,/Open Cape Library/);
  assert.doesNotMatch(view,/Select Cape 1|Disable active cape/);
  cosmetics.remote=state(a,false);assert.match(html(Cape),/No Cape/);
  cosmetics.remote=state(a,false,0);assert.match(html(Cape),/No capes are reported/);
});
it('loading, stale, and remote failures remain honest without hiding the library',()=>{
  account(a);cosmetics.remote=null;cosmetics.loading=true;
  assert.match(html(Skin),/Loading current skin/);assert.match(html(Cape),/Loading owned capes/);
  cosmetics.remote=state();cosmetics.loading=false;cosmetics.offline=true;cosmetics.remoteError='Service is unavailable';
  assert.match(html(Cape),/Last known choice/);assert.match(html(Cape),/Service is unavailable/);
  assert.match(html(Skin),/80 saved/);
});
it('account switch hides the previous account cape and current model immediately',()=>{
  account(a);cosmetics.remote=state(a);cosmetics.offline=false;account(b);
  assert.doesNotMatch(html(Cape),/Cape 0|3 owned/);assert.doesNotMatch(html(Skin),/Current skin · Classic/);
});
it("semantic native DTOs carry no token or user-selected filesystem path", async () => {
  const calls: Array<{ name: string; args: any }> = [];
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (name: string, args: any) => { calls.push({ name, args }); return state(); } } };
  await backend.applySkinPreset(a, "preset-id", "slim");
  await backend.selectOwnedCape(a, "owned-cape");
  await backend.disableOwnedCape(a);
  await backend.importSkinPreset("Name", "classic", [1, 2, 3]);
  await backend.updateSkinPreset("preset-id", { name: "New", model: "slim" });
  await backend.skinPresetThumbnail("preset-id");
  await backend.saveCurrentSkin(a);
  assert.deepEqual(calls.map(call => call.name), [
    "apply_skin_preset", "select_cape", "disable_cape", "import_skin_preset",
    "update_skin_preset", "skin_preset_thumbnail", "save_current_skin",
  ]);
  assert.deepEqual(calls[0].args.request, { accountId: a, presetId: "preset-id", model: "slim" });
  assert.deepEqual(calls[1].args.request, { accountId: a, capeId: "owned-cape" });
  assert.deepEqual(calls[2].args.request, { accountId: a });
  assert.deepEqual(calls[3].args.request, { name: "Name", model: "classic", bytes: [1, 2, 3] });
  assert.deepEqual(calls[4].args.request, { presetId: "preset-id", changes: { name: "New", model: "slim" } });
  assert.deepEqual(calls[5].args.request, { presetId: "preset-id" });
  assert.deepEqual(calls[6].args.request, { accountId: a });
  assert.doesNotMatch(JSON.stringify(calls), /token|filePath|Authorization/i);
});
