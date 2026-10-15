export async function settingsModules(page, action, arg) {
 return page.evaluate(async({action,arg})=>{
  const source=await(await fetch('/Review.svelte')).text();
  const module=name=>source.match(new RegExp('from "([^"\\n]*'+name+'[^"\\n]*)"'))[1];
  const {navigation}=await import(module('navigation.svelte'));
  const {appearance}=await import(module('appearance.svelte'));
  const {homeWidgets}=await import(module('homeLayout.svelte'));
  const {discord}=await import(module('discord.svelte'));
  const {launcher}=await import(module('store.svelte'));
  const {wrapReviewInvoke}=await import(module('review-core'));
  if(action==='category'){navigation.settingsCategory=arg;return;}
  if(action==='snapshot')return {appearance:appearance.state,layout:homeWidgets.layout,discord:discord.state,nav:navigation.state};
  if(action==='discord'){discord.state=arg;return;}
  if(action==='layout'){homeWidgets.layout=arg;return;}
  if(action==='empty'){launcher.launcherState.config.selectedInstanceId=null;return;}
  if(action==='setup') {
   window.ui5Requests=[];window.ui5Failure='';window.ui5Delay=0;
   wrapReviewInvoke(previous=>async(command,args)=>{
    window.ui5Requests.push({command,request:args?.request});
    if(command===window.ui5Failure)throw {code:'fixture_save_failure',message:'Could not save. Your saved preferences are unchanged. Try again.'};
    if(window.ui5Delay&&command.startsWith('set_'))await new Promise(r=>setTimeout(r,window.ui5Delay));
    if(command==='get_appearance')return appearance.state;
    if(command==='get_discord_state')return discord.state;
    if(command==='get_home_widgets')return homeWidgets.layout;
    if(command==='reset_home_widgets')return {widgets:[{id:'instance-details',enabled:true,size:'small'},{id:'content-summary',enabled:true,size:'small'}]};
    return previous(command,args);
   });return;
  }
  if(action==='failure'){window.ui5Failure=arg;return;}
  if(action==='delay'){window.ui5Delay=arg;return;}
  if(action==='requests')return window.ui5Requests;
 },{action,arg});
}
export async function openSettings(page, category='Appearance') {
 await page.goto('http://127.0.0.1:1422/?l3&page=settings&reduced');
 await page.getByRole('heading',{name:'Settings',exact:true}).waitFor();
 await settingsModules(page,'setup');await settingsModules(page,'category',category);
 await page.waitForTimeout(100);
}
