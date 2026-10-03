import assert from "node:assert/strict";
import { test } from "node:test";

import {
  clientPhaseLabel,
  formatBytes,
  isUpdateAvailable,
  launcherPhaseLabel,
  offeredVersion,
  presentationNotes,
  summarizeAvailability,
} from "./updatePresentation.ts";

test("summarizeAvailability reports an update with versions", () => {
  const summary = summarizeAvailability(
    {
      kind: "updateAvailable",
      current: "2.1.5",
      candidate: "2.2.0",

      notes: "Improvements.",
    },
    "client",
  );
  assert.equal(summary.tone, "accent");
  assert.equal(summary.title, "Aurora Client 2.2.0 available");
  assert.equal(summary.detail, "Installed 2.1.5");
});

test("summarizeAvailability names the launcher domain distinctly", () => {
  const summary = summarizeAvailability(
    { kind: "upToDate" },
    "launcher",
  );
  assert.equal(summary.title, "Aurora Launcher is up to date");
});

test("unavailable is honest and never an instance or account failure", () => {
  const summary = summarizeAvailability(
    { kind: "unavailable", reason: "the manifest endpoint answered HTTP 404" },
    "client",
  );
  assert.equal(summary.tone, "warn");
  assert.equal(summary.title, "Could not check for updates");
});

test("notApplicable explains itself", () => {
  const summary = summarizeAvailability(
    { kind: "notApplicable", reason: "no instance is selected" },
    "client",
  );
  assert.equal(summary.title, "No update target");
  assert.equal(summary.detail, "no instance is selected");
});

test("isUpdateAvailable only surfaces real offers", () => {
  assert.equal(isUpdateAvailable({ kind: "notChecked" }), false);
  assert.equal(isUpdateAvailable({ kind: "upToDate" }), false);
  assert.equal(isUpdateAvailable({ kind: "notApplicable", reason: "x" }), false);
  assert.equal(isUpdateAvailable({ kind: "unavailable", reason: "x" }), false);
  assert.equal(
    isUpdateAvailable({
      kind: "updateAvailable",
      current: "1",
      candidate: "2",

      notes: null,
    }),
    true,
  );
  assert.equal(isUpdateAvailable(null), false);
});

test("offeredVersion extracts the candidate", () => {
  assert.equal(
    offeredVersion({
      kind: "updateAvailable",
      current: "1.3.1",
      candidate: "1.4.0",

      notes: null,
    }),
    "1.4.0",
  );
  assert.equal(offeredVersion({ kind: "upToDate" }), null);
});

test("presentationNotes normalizes and bounds untrusted note text", () => {
  assert.equal(presentationNotes({ kind: "upToDate" }, null), null);
  assert.equal(presentationNotes(null, "   \r\n  "), null);
  assert.equal(presentationNotes(null, "Line one\r\nLine two"), "Line one\nLine two");
  const bounded = presentationNotes(null, "x".repeat(5000), 100);
  assert.ok(bounded !== null);
  assert.equal(bounded.length, 100);
  assert.ok(bounded.endsWith("…"));
  // Availability notes are used when no candidate notes override them.
  assert.equal(
    presentationNotes(
      {
        kind: "updateAvailable",
        current: "1",
        candidate: "2",

        notes: "from availability",
      },
      null,
    ),
    "from availability",
  );
});

test("formatBytes formats real progress honestly", () => {
  assert.equal(formatBytes(null), null);
  assert.equal(formatBytes(undefined), null);
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(2048), "2 KiB");
  assert.equal(formatBytes(5 * 1024 * 1024), "5 MiB");
});

test("launcherPhaseLabel never fabricates a percentage", () => {
  assert.equal(launcherPhaseLabel("checking", null, null), "Checking…");
  assert.equal(launcherPhaseLabel("downloading", null, null), "Downloading…");
  assert.equal(launcherPhaseLabel("downloading", 2048, 4096), "Downloading 2 KiB of 4 KiB");
  assert.equal(launcherPhaseLabel("downloading", 2048, null), "Downloading 2 KiB");
  assert.equal(launcherPhaseLabel("readyToInstall", null, null), "Ready to install");
  assert.equal(
    launcherPhaseLabel("installing", null, null),
    "Installing — the launcher will restart",
  );
  assert.equal(launcherPhaseLabel(null, null, null), "");
});

test("clientPhaseLabel names the real transaction phases", () => {
  assert.equal(clientPhaseLabel("acquiring"), "Updating…");
  assert.equal(clientPhaseLabel("committing"), "Updating…");
  assert.equal(clientPhaseLabel("validating"), "Updating…");
  assert.equal(clientPhaseLabel(null), "");
});
