import assert from "node:assert/strict";
import { test } from "node:test";
import { creationPlatforms, requiresLoaderVersion, creationBlocked, transitionRows, executeApprovedTransition } from "./configurationChoices.ts";
import type { AuroraTransitionPreview, PlatformCapability } from "../backend.ts";
const capability = (kind: PlatformCapability["kind"], usable: boolean, auroraSupported = false): PlatformCapability => ({kind,canCreate:usable,canInstall:usable,canValidate:usable,canLaunch:usable,auroraSupported});
test("creation choices consume backend capabilities including Vanilla and Fabric", () => {
  const choices = [capability("vanilla",true),capability("fabric",true,true),capability("forge",false),capability("neoForge",false),capability("quilt",false)];
  assert.deepEqual(creationPlatforms(choices).map(choice => choice.kind), ["vanilla","fabric"]);
  assert.deepEqual(creationPlatforms([capability("vanilla",false)]), []);
});
test("Vanilla has no loader selector and Fabric accepts optional backend Aurora compatibility", () => {
  assert.equal(requiresLoaderVersion({kind:"vanilla"}), false);
  assert.equal(requiresLoaderVersion({kind:"fabric",policy:{type:"pinned",version:"0.19.5"}}), true);
  const unavailable = {available:false,reason:"Unsupported",version:null,loaderVersion:null};
  const available = {available:true,reason:"Compatible",version:"2.1.2",loaderVersion:"0.19.5"};
  assert.equal(creationBlocked(false, unavailable), false);
  assert.equal(creationBlocked(true, unavailable), true);
  assert.equal(creationBlocked(true, available), false);
  assert.equal(creationBlocked(true, null), true);
});
const file = (relativePath: string, reason: string) => ({relativePath,reason,sha256:"a".repeat(64),sizeBytes:1});
const preview: AuroraTransitionPreview = {
  instanceId:"test",current:{minecraftVersion:"1.21.11",platform:{kind:"fabric",version:"0.19.5"},aurora:null},
  target:{minecraftVersion:"1.21.11",platform:{kind:"fabric",version:"0.19.5"},aurora:{channel:"stable",version:"2.1.2"}},
  requirementsAdded:["Aurora"],requirementsRemoved:[],install:[file("mods/new.jar","Required")],remove:[file("mods/old.jar","Retired")],retain:[file("mods/api.jar","Dependency")],warnings:[],blockers:[],fingerprint:"approved",
};
test("transition renders backend additions removals and retention reasons", () => {
  assert.deepEqual(transitionRows(preview).map(row => [row.action,row.reason]), [["Install / reconcile","Required"],["Remove","Retired"],["Retain","Dependency"]]);
});
test("approved transition sends the fingerprint and refreshes successful state", async () => {
  const calls: unknown[] = [];
  assert.equal(await executeApprovedTransition(preview, async (...args) => {calls.push(args);return "updated";},async () => {calls.push("refresh");}), "updated");
  assert.deepEqual(calls, [["test",true,"approved"],"refresh"]);
});
test("stale and failed transitions preserve errors and do not report success", async () => {
  for (const message of ["Preview is stale", "Acquisition failed"]) {
    let refreshed = false;
    await assert.rejects(executeApprovedTransition(preview,async () => {throw new Error(message);},async () => {refreshed=true;}),new RegExp(message));
    assert.equal(refreshed,false);
  }
  await assert.rejects(executeApprovedTransition({...preview,blockers:["Provider ownership conflict"]},async () => {throw new Error("must not apply");},async () => {}),/Provider ownership conflict/);
});
