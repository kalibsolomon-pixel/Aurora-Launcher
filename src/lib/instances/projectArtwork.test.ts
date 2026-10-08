import assert from "node:assert/strict";
import { it } from "node:test";
import { artworkProject, ResolutionCache } from "./projectArtwork.ts";
import type { ModEntry } from "../backend.ts";
const entry = (ownership:ModEntry["ownership"],provider="modrinth") => ({ownership,provenance:{provider,projectId:"AAAABBBB"}} as ModEntry);
it("only provider-managed Modrinth provenance supplies installed artwork identity", () => {
  const item=entry("providerManaged"); const before=structuredClone(item);
  assert.equal(artworkProject(item),"AAAABBBB"); assert.deepEqual(item,before);
  for(const ownership of ["userManaged","unknown","launcherManagedRequired"] as const) assert.equal(artworkProject(entry(ownership)),null);
  assert.equal(artworkProject(entry("providerManaged","other")),null);
});

it('typed artwork retries transient failures, coalesces concurrent subscribers and reuses success', async () => {
  let now=0, calls=0;
  const cache=new ResolutionCache<{source:string|null;retryAfterMs:number|null}>(()=>now);
  const fetch=async()=>{calls++;return calls===1 ? {source:null,retryAfterMs:60_000} : {source:'data:image/png;base64,YWJjZA==',retryAfterMs:null};};
  const [first,second]=await Promise.all([cache.get('project',fetch),cache.get('project',fetch)]);
  assert.deepEqual(first,second);assert.equal(calls,1);
  now=59_999;await cache.get('project',fetch);assert.equal(calls,1);
  now=60_000;assert.ok((await cache.get('project',fetch)).source);assert.equal(calls,2);
  await cache.get('project',fetch);assert.equal(calls,2);
});
it('typed absent artwork stays distinct from temporary rejection and expires', async()=>{
  let now=0,calls=0;const cache=new ResolutionCache<{retryAfterMs:null}>(()=>now);
  const fetch=async()=>{calls++;return {retryAfterMs:null};};
  await cache.get('absent',fetch);now=60_000;await cache.get('absent',fetch);assert.equal(calls,1);
  now=600_000;await cache.get('absent',fetch);assert.equal(calls,2);
  let failed=0;const retry=async()=>{if(++failed===1)throw Error('offline');return {retryAfterMs:null};};
  await assert.rejects(cache.get('retry',retry));now+=60_000;
  await cache.get('retry',retry);assert.equal(failed,2);
});
