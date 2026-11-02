import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer } from "node:http";
import { performance } from "node:perf_hooks";

test("isolated Git ref advancement and HTTP cache expose stale then current bytes", async () => {
  // This measures a controlled loopback cache, NOT GitHub propagation latency.
  const directory = await mkdtemp(join(tmpdir(), "aurora-authority-cache-"));
  const git = (args, input) => execFileSync("git", ["--git-dir", directory, ...args], {
    input, encoding: "utf8", env: { ...process.env, GIT_AUTHOR_NAME: "Authority test",
      GIT_AUTHOR_EMAIL: "test@example.invalid", GIT_COMMITTER_NAME: "Authority test", GIT_COMMITTER_EMAIL: "test@example.invalid" },
  }).trim();
  execFileSync("git", ["init", "--bare", directory], { stdio: "ignore" });
  const ref = "refs/heads/launcher-update-authority";
  const commit = (bytes, parent) => {
    const blob = git(["hash-object", "-w", "--stdin"], bytes);
    const tree = git(["mktree"], `100644 blob ${blob}\tlauncher-update.json\n`);
    return git(["commit-tree", tree, ...(parent ? ["-p", parent] : [])], "Diagnostic authority\n");
  };
  const oldBytes = Buffer.from('{"version":"1.4.1"}\n');
  const newBytes = Buffer.from('{"version":"1.4.2"}\n');
  const emptyTree = git(["mktree"], "");
  const root = git(["commit-tree", emptyTree], "Initialize Aurora Launcher update authority\n");
  git(["update-ref", ref, root]);
  const old = commit(oldBytes, root);
  git(["update-ref", ref, old]);
  const candidate = commit(newBytes, old);
  let staleUntil = Infinity;
  const server = createServer((_req, response) => {
    const bytes = performance.now() < staleUntil ? oldBytes : Buffer.from(git(["show", `${ref}:launcher-update.json`]) + "\n");
    response.writeHead(200, { "Cache-Control": "max-age=300", "Content-Type": "text/plain; charset=utf-8", ETag: bytes.equals(oldBytes) ? '"old"' : '"new"' });
    response.end(bytes);
  });
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  try {
    const url = `http://127.0.0.1:${server.address().port}/launcher-update.json`;
    const initial = await fetch(url);
    assert.equal(initial.status, 200); assert.equal(initial.redirected, false);
    assert.equal(initial.headers.get("cache-control"), "max-age=300");
    assert.ok(Buffer.from(await initial.arrayBuffer()).equals(oldBytes));
    git(["update-ref", ref, candidate, old]);
    const moved = performance.now();
    staleUntil = moved + 120;
    assert.equal(git(["rev-parse", ref]), candidate);
    const immediate = Buffer.from(await (await fetch(url, { headers: { "Cache-Control": "no-cache" } })).arrayBuffer());
    assert.ok(immediate.equals(oldBytes), "ref advanced while cached previous bytes remained visible");
    let observed;
    while (performance.now() - moved < 2000) {
      const response = await fetch(url, { headers: { "Cache-Control": "no-cache" } });
      if (Buffer.from(await response.arrayBuffer()).equals(newBytes)) { observed = performance.now(); break; }
      await new Promise((done) => setTimeout(done, 5));
    }
    assert.ok(observed !== undefined);
    console.log(`CONTROLLED loopback propagation after local ref advancement: ${(observed - moved).toFixed(1)} ms; GitHub latency unmeasured.`);
  } finally {
    server.closeAllConnections(); await new Promise((done) => server.close(done));
  }
  // Disposable root is retained. No cleanup of pre-existing files.
});
