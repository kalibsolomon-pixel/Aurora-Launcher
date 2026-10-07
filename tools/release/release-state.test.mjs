import test from "node:test";
import assert from "node:assert/strict";
import { selectRelease, selectAsset, verifyBytes, verifyNotesIdentity, expectedAssetName, productionClientIdentities, productionClientVersion } from "./release-state.mjs";
import { createHash } from "node:crypto";

const sha = "a".repeat(40);
const draft = { id: 9, tag_name: "v1.1.0", target_commitish: sha, draft: true, prerelease: false };
const publicRelease = { ...draft, draft: false };
const name = "Aurora Launcher_1.1.0_x64-setup.exe";
const bytes = Buffer.from("installer fixture");
const expected = { name, sizeBytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") };
const asset = { id: 2, name: expectedAssetName(name), size: bytes.length, digest: `sha256:${expected.sha256}` };

test("no release creates a draft", () => assert.equal(selectRelease([], "v1.1.0", sha).kind, "create"));
test("matching draft is selected by exact ID", () => assert.equal(selectRelease([draft], "v1.1.0", sha, 9).kind, "resume"));
test("draft selection does not require a by-tag lookup or tag ref", () => assert.equal(selectRelease([draft], "v1.1.0", sha, 9, null).release.id, 9));
test("normalized name and exact bytes pass", () => { assert.equal(selectAsset([asset], expected).id, 2); verifyBytes(bytes, expected); });
test("wrong downloaded bytes fail", () => assert.throws(() => verifyBytes(Buffer.from("different fixture"), expected)));
test("wrong advertised digest fails", () => assert.throws(() => selectAsset([{ ...asset, digest: "sha256:" + "0".repeat(64) }], expected)));
test("duplicate candidate assets fail", () => assert.throws(() => selectAsset([asset, { ...asset, id: 3, name }], expected)));
test("conflicting draft fails", () => assert.throws(() => selectRelease([{ ...draft, target_commitish: "b".repeat(40) }], "v1.1.0", sha, 9)));
test("existing public release is reported complete", () => assert.equal(selectRelease([publicRelease], "v1.1.0", sha, null, sha).kind, "public"));
test("conflicting tag fails", () => assert.throws(() => selectRelease([draft], "v1.1.0", sha, 9, "b".repeat(40))));
test("multiple releases fail", () => assert.throws(() => selectRelease([draft, { ...draft, id: 10 }], "v1.1.0", sha, 9)));
test("notes naming the launcher and current client pass", () => {
  verifyNotesIdentity("# Aurora Launcher 1.2.0 — release notes\n\nUses Aurora Client 2.1.5.", "1.2.0", "2.1.5");
});
test("notes with a stale client version fail", () => {
  assert.throws(() => verifyNotesIdentity("# Aurora Launcher 1.2.0 — release notes\n\nUses Aurora Client 2.1.3.", "1.2.0", "2.1.5"));
});
test("notes with the wrong launcher version fail", () => {
  assert.throws(() => verifyNotesIdentity("# Aurora Launcher 1.1.0 — release notes\n\nUses Aurora Client 2.1.5.", "1.2.0", "2.1.5"));
});
test("current Client notes derive identity from the production catalog", () => {
  assert.equal(productionClientVersion, "3.0.0");
  verifyNotesIdentity("# Aurora Launcher 1.4.1 — release notes\nAurora Client 3.0.0.", "1.4.1");
  for (const identity of ["2.1.5", "3.0.0-fake", "3.0.0.1", "3.0.00", "9.9.9"]) {
    assert.throws(() => verifyNotesIdentity(`# Aurora Launcher 1.4.1\nAurora Client ${identity}`, "1.4.1"));
  }
  assert.throws(() => verifyNotesIdentity("# Aurora Launcher 1.4.10\nAurora Client 3.0.0", "1.4.1"));
});
test("production Client ordering is semantic and independent of catalog order", () => {
  const releases = ['1.9.0', '1.10.0', '2.0.0'].map((auroraVersion) => ({ auroraVersion, channel: 'stable' }));
  assert.deepEqual(productionClientIdentities({ schemaVersion: 1, releases }), ['2.0.0', '1.10.0', '1.9.0']);
  for (const catalog of [null, { schemaVersion: 2, releases }, { schemaVersion: 1, releases: [] },
    { schemaVersion: 1, releases: [releases[0], releases[0]] },
    { schemaVersion: 1, releases: [{ auroraVersion: '3.0.0-fake', channel: 'stable' }] }]) {
    assert.throws(() => productionClientIdentities(catalog));
  }
});
