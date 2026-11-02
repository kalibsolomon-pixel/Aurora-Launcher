export async function appearanceFixture(page,query='') {
  await page.goto(`http://127.0.0.1:1422/?l3&page=cosmetics&reduced&${query}`);
  await page.getByRole('heading',{name:'Appearance',exact:true}).waitFor();
  await page.waitForTimeout(180);
}
export async function appearanceModules(page,action,arg) {
  return page.evaluate(async({action,arg})=>{
    const source=await(await fetch('/Review.svelte')).text();
    const module=name=>source.match(new RegExp('from "([^"\\n]*'+name+'[^"\\n]*)"'))[1];
    const {launcher}=await import(module('store.svelte'));
    const {wrapReviewInvoke}=await import(module('review-core'));
    const cosmeticsUrl=new URL('./cosmetics.svelte.ts',new URL(module('store.svelte'),location.href)).href;
    const {cosmetics}=await import(cosmeticsUrl);
    if(action==='profile') { cosmetics.remote={...cosmetics.remote,...arg};return; }
    if(action==='requests') {
      window.ui4Requests=[];
      wrapReviewInvoke(previous=>async(command,args)=>{window.ui4Requests.push({command,request:args?.request});return previous(command,args);});return;
    }
    if(action==='account') {launcher.accountsState.selectedAccountId=arg;return;}
    if(action==='stale') {
      window.ui4Resolve=null;
      wrapReviewInvoke(previous=>async(command,args)=>command==='get_cosmetics'&&args.request.accountId===arg?new Promise(resolve=>window.ui4Resolve=resolve):previous(command,args));
      void cosmetics.load(arg,true);return;
    }
    if(action==='resolve') {window.ui4Resolve(arg);return;}
    if(action==='snapshot')return {remote:cosmetics.remote,accountId:cosmetics.accountId,avatar:launcher.accountAvatars[launcher.selectedAccount?.accountId],selected:cosmetics.selectedPresetId};
  },{action,arg});
}
