// Synthetic presentation-only inventory; never forwards mutation or owner data.
export async function seed(page, surface, base='http://127.0.0.1:1422/', options={}) {
  await page.goto(base + '?tab=mods&background=borealis');
  await page.locator('.mod-row').first().waitFor();
  await page.evaluate(async ({nativeArtwork=[]}) => {
    const source = await (await fetch('/Review.svelte')).text();
    const module = name => source.match(new RegExp('from "([^"\\n]*'+name+'[^"\\n]*)"'))[1];
    const { launcher } = await import(module('store.svelte'));
    const { wrapReviewInvoke, nativeInvoke } = await import(module('review-core'));
    window.uiRequests = {}; window.uiMutations = 0; window.uiArtworkRecords=[];
    const images = Array.from({length:100},(_,i)=>{const c=document.createElement('canvas');c.width=c.height=64;const g=c.getContext('2d');g.fillStyle=`hsl(${i*37} 60% 42%)`;g.fillRect(0,0,64,64);g.fillStyle='#eaffff';g.font='bold 28px sans-serif';g.fillText(String(i),4,40);return c.toDataURL();});
    const id='a'.repeat(32), entries=Array.from({length:100},(_,i)=>({entryId:'ui-'+i,displayName: ['Continuity','More Culling',"Reese’s Sodium Options"][i]??'Review Mod '+String(i).padStart(3,'0'),fileName:`review-${i}.jar`, enabled:true,fileType:'enabledJar',sizeBytes:1200000,modifiedUnixMillis:null,ownership:'userManaged',sha256:null,provenance:null,metadata:{id:'review'+i,version:'3.0.1-beta.1+1.21.11',authors:['Synthetic fixture']},warnings:i%4===0?[]:[{code:'dependency_version',message:'Requires Sodium >=0.7.0; the installed version does not satisfy this advisory metadata constraint.'},...(i===2?[{code:'recommendation',message:'Optional recommendation: '+ 'A long metadata explanation with wrapping. '.repeat(12)}]:[])],canToggle:true,canRemove:true,actionBlockedReason:null}));
    launcher.modInventories[id]={instanceId:id,entries,missingManaged:[]};
    wrapReviewInvoke(previous=>async(command,args)=>{
      window.uiRequests[command]=(window.uiRequests[command]??0)+1;
      if(command==='get_installed_artwork_identities')return {projects:Object.fromEntries(entries.map((e,i)=>[e.entryId,'UI'+String(i).padStart(6,'0')])),retryAfterMs:null};
      if(command==='resolve_project_artwork'){
        const index=Number(args.request.projectId.slice(2));
        if(nativeArtwork.length){const start=performance.now();const result=await nativeInvoke('resolve_project_artwork',{request:{projectId:nativeArtwork[index%nativeArtwork.length]}});window.uiArtworkRecords.push({status:result.status,ms:performance.now()-start});return result;}
        return {source:images[index%100],status:'available',retryAfterMs:null};
      }
      if(command==='list_instance_mods')return launcher.modInventories[id];
      if(command==='set_instance_mod_enabled')throw {code:'mod_dependency_required',message:'Fixture: this mod is required by another enabled mod. Review the dependency before disabling it.'};
      if(command==='quick_install_modrinth')throw {code:'provider_network_error',message:'Synthetic unavailable response; no acquisition or installation.'};
      if(command==='browse_modrinth'){const offset=args.request.offset??0;return {offset,totalHits:100,hits:Array.from({length:20},(_,j)=>{const i=offset+j;return {projectId:'UI'+String(i).padStart(6,'0'),title:'Aurora Review Project '+String(i).padStart(3,'0'),summary:'Deterministic illustrated content for comparable scrolling performance. Mod artwork is present in every row.',author:'Visual fixture',downloads:1000+i,categories:['optimization'],projectType:args.request.contentType};})};}
      return previous(command,args);
    });
  },options);
  if(surface!=='installed'){
    await page.getByRole('button',{name:'Browse Modrinth',exact:true}).click();
    if(surface!=='mod')await page.getByRole('group',{name:'Browse content type'}).getByRole('button',{name:{resourcePack:'Resource Packs',shaderPack:'Shaders',modpack:'Modpacks'}[surface],exact:true}).click();
    await page.locator('.browse-row').first().waitFor();
    for(let i=0;i<4;i++)await page.getByRole('button',{name:'Load more',exact:true}).click();
  }
  await page.locator('main.content').evaluate(e=>e.scrollTop=0);
  await page.waitForTimeout(options.settleMs??700);
}
