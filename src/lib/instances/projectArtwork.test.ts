import assert from "node:assert/strict";
import { it } from "node:test";
import { artworkProject, ArtworkCache } from "./projectArtwork.ts";
import type { ModEntry } from "../backend.ts";
const entry = (ownership:ModEntry["ownership"],provider="modrinth") => ({ownership,provenance:{provider,projectId:"AAAABBBB"}} as ModEntry);
it("only provider-managed Modrinth provenance supplies installed artwork identity", () => {
  const item=entry("providerManaged"); const before=structuredClone(item);
  assert.equal(artworkProject(item),"AAAABBBB"); assert.deepEqual(item,before);
  for(const ownership of ["userManaged","unknown","launcherManagedRequired"] as const) assert.equal(artworkProject(entry(ownership)),null);
  assert.equal(artworkProject(entry("providerManaged","other")),null);
});
it("installed project artwork deduplicates requests and expires cosmetic reuse", async () => {
  let now=0,calls=0; const cache=new ArtworkCache(()=>now);
  const fetch=async()=>{calls++;return "https://cdn.modrinth.com/data/AAAABBBB/icon.png";};
  const result=await Promise.all([cache.get("AAAABBBB",fetch),cache.get("AAAABBBB",fetch)]);
  assert.ok(result.every(url=>url?.includes("icon.png"))); assert.equal(calls,1);
  now=600001;await cache.get("AAAABBBB",fetch);assert.equal(calls,2);
});
it("artwork network failures become fallback and retry after a short negative cache", async () => {
  let now=0,calls=0;const cache=new ArtworkCache(()=>now);
  const fetch=async()=>{calls++;throw new Error("offline");};
  assert.equal(await cache.get("AAAABBBB",fetch),null);await cache.get("AAAABBBB",fetch);assert.equal(calls,1);
  now=60001;await cache.get("AAAABBBB",fetch);assert.equal(calls,2);
});
