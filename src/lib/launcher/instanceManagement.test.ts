import assert from "node:assert/strict";
import { after, before, it } from "node:test";
import { createServer } from "vite";
let server: any, launcher: any, render: any, Rename: any, Settings: any, Recent: any;
const id = "a".repeat(32);
const configuration = { minecraftVersion: "1.21.11", loader: {kind:"fabric",policy:{type:"pinned",version:"0.19.5"}},auroraEnabled:true,memoryMib:2048,additionalJvmArguments:"",window:null };
const instance = () => ({id,displayName:"Original",state:"ready",minecraftVersion:"1.21.11",platform:{kind:"fabric",version:"0.19.5"},aurora:{version:"2.1.5",channel:"stable"},configuration:structuredClone(configuration)});
before(async () => {
 server = await createServer({server:{middlewareMode:true},logLevel:"silent"});
 ({render}=await server.ssrLoadModule("svelte/server"));
 ({launcher}=await server.ssrLoadModule("/src/lib/launcher/store.svelte.ts"));
 Rename=(await server.ssrLoadModule("/src/lib/instances/InstanceRenameDialog.svelte")).default;
 Settings=(await server.ssrLoadModule("/src/lib/instances/InstanceSettingsPanel.svelte")).default;
 Recent=(await server.ssrLoadModule("/src/lib/launcher/widgets/RecentTargetsWidget.svelte")).default;
 launcher.launcherState={config:{selectedInstanceId:id},instances:[instance()],platformCapabilities:[{kind:"fabric",canInstall:true},{kind:"vanilla",canInstall:true}]};
 launcher.refreshPlayReadiness=async()=>{};
});
after(async()=>{await server?.close();delete (globalThis as any).window;});
it("server list contains twenty older entries in one keyboard scroll region",()=>{
 const entries=Array.from({length:20},(_,i)=>({id:String(i).padStart(64,"0"),instanceId:id,displayName:`Server ${i}`,lastPlayedAt:20-i,durationMs:1000,available:i!==19,favorite:i===18}));
 const html=render(Recent,{props:{mode:"server",size:"small",testFixture:entries}}).body;
 assert.equal((html.match(/<li\b/g)??[]).length,20);assert.match(html,/Scrollable server history/);assert.match(html,/tabindex="0"/);assert.match(html,/Favorites · 1/);assert.match(html,/Unavailable/);assert.match(html,/Quick Launch server Server 19/);
});
it("rename sends original name, blocks duplicate submissions and synchronizes state",async()=>{
 let resolve: any;const calls:any[]=[];
 (globalThis as any).window={__TAURI_INTERNALS__:{invoke:async(command:string,args:any)=>{calls.push({command,args});return new Promise(r=>resolve=r);}}};
 launcher.beginRename(id);assert.equal(launcher.renaming.name,"Original");launcher.renaming.name="Renamed 雪";
 const first=launcher.runRename();await launcher.runRename();assert.equal(calls.length,1);
 assert.deepEqual(calls[0].args.request,{instanceId:id,newDisplayName:"Renamed 雪",expectedDisplayName:"Original"});
 resolve({...instance(),displayName:"Renamed 雪"});await first;
 assert.equal(launcher.launcherState.instances[0].displayName,"Renamed 雪");assert.equal(launcher.renaming,null);assert.equal(launcher.renameBusy,false);
});
it("rename failure retains the original display and editable draft",async()=>{
 (globalThis as any).window={__TAURI_INTERNALS__:{invoke:async()=>{throw {code:"instance_registry_write_failure",message:"Fixture write refused"};}}};
 launcher.beginRename(id);launcher.renaming.name="Failed";await launcher.runRename();
 assert.equal(launcher.launcherState.instances[0].displayName,"Renamed 雪");assert.equal(launcher.renaming.name,"Failed");assert.match(launcher.renameError.message,/write refused/);
 launcher.renaming.name=" ";assert.match(render(Rename).body,/Enter an instance name/);
 launcher.renaming.name="雪".repeat(27);assert.match(render(Rename).body,/at most 80 UTF-8 bytes/);
 launcher.renaming=null;
});
it("settings removes the redundant Aurora card, exposes snapshots and preserves arbitrary whole MiB",()=>{
 launcher.detailDrafts[id]=structuredClone(configuration);launcher.loaderVersions=[{version:"0.19.5",stable:true}];
 const html=render(Settings,{props:{instance:instance()}}).body;
 assert.doesNotMatch(html,/Aurora configuration/);assert.match(html,/Include snapshots/);assert.match(html,/Memory \(MiB\)/);assert.match(html,/step="1"/);assert.match(html,/Maintenance/);assert.match(html,/Delete instance/);assert.match(html,/Manage Aurora content in Mods/);
});
it("configuration sends the original draft baseline and refreshes readiness after save",async()=>{
 launcher.detailDrafts={};launcher.detailBases={};launcher.loaderVersionsKey="fabric:1.21.11";launcher.openDetail(id);
 launcher.detailDrafts[id].memoryMib=4097;const calls:any[]=[];let readiness=0;launcher.refreshPlayReadiness=async()=>{readiness++;};
 (globalThis as any).window={__TAURI_INTERNALS__:{invoke:async(command:string,args:any)=>{calls.push({command,args});if(command==="update_instance_configuration"){launcher.launcherState.instances[0].configuration=args.request.configuration;return launcher.launcherState.instances[0];}return launcher.launcherState;}}};
 await launcher.runSaveConfiguration(id);
 const saved=calls.find(c=>c.command==="update_instance_configuration");assert.equal(saved.args.request.expectedConfiguration.memoryMib,2048);assert.equal(saved.args.request.configuration.memoryMib,4097);assert.equal(launcher.detailBases[id].memoryMib,4097);assert.equal(readiness,1);
});
