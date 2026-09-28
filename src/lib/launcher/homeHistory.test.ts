import test from "node:test";
import assert from "node:assert/strict";
import { formatPlaytime, recentLabel } from "./homeHistory.ts";
import { moveWidget, registeredWidgets, setWidget, widgetCatalog } from "./widgets.ts";

test("playtime and recency use readable bounded units", () => {
  assert.equal(formatPlaytime(0), "0m");
  assert.equal(formatPlaytime(42 * 60000), "42m");
  assert.equal(formatPlaytime((12 * 60 + 42) * 60000), "12h 42m");
  assert.equal(recentLabel(1_000_000, 1_000_000_000), "Today");
  assert.equal(recentLabel(1_000_000 - 86400, 1_000_000_000), "Yesterday");
});

test("the three stable history widgets obey show, size, ordering, and unknown-ID preservation", () => {
  const ids = ["playtime", "recent-worlds", "recent-servers"];
  for (const id of ids) assert.deepEqual(widgetCatalog.find(item => item.id === id)?.sizes, ["small", "wide", "large"]);
  const original = { widgets: [
    { id: "instance-details", enabled: true, size: "small" as const },
    { id: "future-widget", enabled: true, size: "wide" as const },
  ] };
  const withPlaytime = setWidget(original, "playtime", { enabled: true, size: "large" });
  assert.deepEqual(registeredWidgets(withPlaytime).map(item => item.id), ["instance-details", "playtime"]);
  const moved = moveWidget(withPlaytime, "playtime", -1);
  assert.deepEqual(moved.widgets.map(item => item.id), ["playtime", "future-widget", "instance-details"]);
  assert.equal(setWidget(moved, "playtime", { enabled: false }).widgets[0].enabled, false);
  assert.equal(moved.widgets[1].id, "future-widget");
});

test("E3 cosmetics entries append only when explicitly enabled in an existing E2 layout", () => {
  for (const id of ["skin-manager", "cape-selector"]) assert.deepEqual(widgetCatalog.find(item => item.id === id)?.sizes, ["small", "wide", "large"]);
  const e2 = { widgets: [
    { id: "recent-servers", enabled: false, size: "large" as const },
    { id: "playtime", enabled: true, size: "wide" as const },
    { id: "recent-worlds", enabled: true, size: "small" as const },
  ] };
  assert.deepEqual(registeredWidgets(e2).map(item => item.id), ["playtime", "recent-worlds"]);
  const added = setWidget(e2, "skin-manager", { enabled: true, size: "wide" });
  assert.deepEqual(added.widgets.slice(0, e2.widgets.length), e2.widgets);
  assert.equal(added.widgets.at(-1)?.id, "skin-manager");
  assert.equal(added.widgets.at(-1)?.size, "wide");
});
