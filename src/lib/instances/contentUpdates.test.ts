import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { UpdateAvailability } from "../backend.ts";
import {
  BROWSE_KINDS,
  BROWSE_SORTS,
  browseKindLabel,
  browseKindNoun,
  browseSortLabel,
  summarizeUpdates,
  updateChannelDescription,
  updateChannelLabel,
  updateStatusText,
} from "../backend.ts";

function availability(status: UpdateAvailability["status"], extra: Partial<UpdateAvailability> = {}): UpdateAvailability {
  return {
    contentType: "mod",
    projectId: "AAAA0001",
    currentVersion: "1.0.0",
    status,
    block: null,
    candidate: null,
    pinned: false,
    channel: "stable",
    detail: null,
    ...extra,
  };
}

describe("update availability summaries", () => {
  it("counts each typed state separately and never collapses blocked or noted entries", () => {
    const counts = summarizeUpdates([
      availability("updateAvailable"),
      availability("updateAvailable"),
      availability("pinnedUpdateAvailable"),
      availability("blocked", { block: "disabled" }),
      availability("upToDate"),
      availability("noNewerUnderPolicy"),
      availability("providerUnavailable"),
      availability("currentVersionUnknown"),
    ]);
    assert.deepEqual(counts, {
      ready: 2,
      pinned: 1,
      blocked: 1,
      current: 1,
      noted: 3,
    });
  });

  it("names each status in user-facing language with candidate versions", () => {
    const available = availability("updateAvailable", {
      candidate: { id: "v2", name: "Release 2", versionNumber: "2.0.0", versionType: "release", datePublished: "", environment: "client_and_server", loaders: ["fabric"] },
    });
    assert.equal(updateStatusText(available), "Update available: 2.0.0");
    const pinned = availability("pinnedUpdateAvailable", {
      candidate: { id: "v2", name: "Release 2", versionNumber: "2.0.0", versionType: "release", datePublished: "", environment: "client_and_server", loaders: ["fabric"] },
    });
    assert.equal(updateStatusText(pinned), "Pinned — update available: 2.0.0");
    assert.equal(updateStatusText(availability("upToDate")), "Up to date");
    assert.equal(
      updateStatusText(availability("blocked", { block: "disabled", detail: "This mod is disabled." })),
      "This mod is disabled.",
    );
    assert.equal(
      updateStatusText(availability("noNewerUnderPolicy", { detail: "Newer versions exist but none is allowed by the Stable release-channel policy for this content." })),
      "Newer versions exist but none is allowed by the Stable release-channel policy for this content.",
    );
  });
});

describe("provider browse vocabulary", () => {
  it("labels every browsable content type and keeps modpacks distinct from installable kinds", () => {
    assert.equal(BROWSE_KINDS.length, 4);
    assert.ok(BROWSE_KINDS.includes("modpack"));
    assert.equal(browseKindLabel("mod"), "Mods");
    assert.equal(browseKindLabel("modpack"), "Modpacks");
    assert.equal(browseKindLabel("resourcePack"), "Resource Packs");
    assert.equal(browseKindLabel("shaderPack"), "Shaders");
    assert.equal(browseKindNoun("shaderPack"), "shaders");
  });

  it("exposes only provider-supported sort orders with readable labels", () => {
    assert.deepEqual([...BROWSE_SORTS], ["relevance", "downloads", "newest", "updated"]);
    assert.equal(browseSortLabel("relevance"), "Relevance");
    assert.equal(browseSortLabel("downloads"), "Most downloaded");
    assert.equal(browseSortLabel("newest"), "Newest");
    assert.equal(browseSortLabel("updated"), "Recently updated");
  });
});

describe("release channel labels", () => {
  it("labels and describes the three persisted policies", () => {
    assert.equal(updateChannelLabel("stable"), "Stable");
    assert.equal(updateChannelLabel("beta"), "Beta");
    assert.equal(updateChannelLabel("alpha"), "Alpha");
    assert.equal(updateChannelDescription("stable"), "Release versions only");
    assert.equal(updateChannelDescription("beta"), "Release and beta versions");
    assert.equal(updateChannelDescription("alpha"), "Release, beta, and alpha versions");
  });
});
