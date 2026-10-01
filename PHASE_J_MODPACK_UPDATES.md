# Phase J: Modpack update & reconciliation

Updating an installed Modrinth modpack is a reconciliation between two exact
authored snapshots and the current local filesystem — never "Update All over
the pack's mods" and never "delete and re-extract". The user's local changes
are understood and classified before Aurora mutates the instance, and the
resulting plan is what the user reviews.

## Update discovery

Discovery starts from the installed `pack-installed.json` identity alone: the
frontend supplies only the instance identifier. Rust loads the same
project's full version timeline (`pack_versions`, unfiltered by the current
instance context so an unsupported candidate is reported honestly instead of
hidden), anchors chronology on the installed exact version — which must
still exist in the same project and still publish the installed archive
SHA-512 — and offers the newest version published strictly after it.
Publication times order candidates; version strings are never compared, and
equal times are never "newer". A candidate on another loader (quilt/forge)
is reported blocked with its reason and never re-anchors the project.

`Check Modpack Update` (instance Overview) is separate from `Check for
updates` (Content tabs): pack-owned components remain blocked from ordinary
content updates with a message pointing at Update Modpack.

## Three-way reconciliation

For every path the old snapshot owns (components, overrides, recorded
divergences) or the new snapshot authors, the planner hashes the current
local file once (SHA-256 + SHA-512 in one pass) and classifies with the
strongest available identity — provider + project + version + file SHA-512 —
never by filename. Distinct files from one project stay distinct rows. A
file whose bytes the new pack authors at a new path is recognized by file
identity as a move: the old path retires and the new path adds.

Component classifications: **unchanged** (preserve, bytes and provider
records stay); **added** (acquire → verify → activate, or adopt without a
write when local bytes already equal the published SHA-512); **removed,
local unchanged** (safe retirement — proven old digest, no remaining
ownership requirement); **removed, locally modified** (conflict; keep-local
only, Aurora never deletes user bytes); **changed, local unchanged**
(transactional replace); **changed, locally modified** (conflict with
keep-local / use-new choices). Missing old files are disclosed: still
required → restored; removed by the new pack → only the record ends.

Overrides follow the same matrix: unchanged/changed/removed by the pack
crossed with unchanged/changed locally, added-with-collision unless
equivalence is proven by digest, and client-overrides keep their precedence
over common overrides.

## Divergence

A keep-local resolution (or a pre-existing local modification of a file the
pack did not change) is recorded truthfully. `pack-installed.json` moves to
schema 2 with a `divergences` list; each entry records the path, the pack's
authored expectation (`expectedSha256`/`expectedSha512`, absent when the
pack removes the path), the chosen `localSha256`, and the resolution.
Validation checks divergence paths against their chosen bytes, so drift from
the chosen state is detected while the chosen state itself validates. The
instance's pack version means "base version v2 with these recorded
exceptions"; the Overview shows "Modified (N local changes)". Schema-1
documents migrate in memory (empty divergences); a schema-1 document already
claiming divergences is malformed; unknown schemas still fail deliberately.

## Shared ownership and safe retirement

A provider component retired by the pack is deleted only when its current
bytes still match the recorded digest and no remaining managed record
requires it (dependents retiring in the same transaction do not count). If
another root still requires it, the file and its provider record survive and
only the pack's ownership ends — the row reports "preserved (still
required)". Unrelated user files never appear in a plan and are never
touched; "not in new pack" is never permission to delete.

## Plan, staleness, and the transaction

The plan (candidate identity, game transition if any, per-path rows with
old/new/local states and proposed actions, counts, optional-file carry-over)
is Rust-owned; the frontend renders it and cannot reconstruct it. Apply
re-derives everything natively and compares a fingerprint over
resolution-independent facts: candidate identity + archive digest, every
row's classification inputs including observed local digests, and the prior
pack/managed/registry state. Any drift — including a local file changed
after preview or a reclassified collision — refuses the apply. Conflict
resolutions are validated against the fresh classification.

The transaction runs: recover-any-interrupted-update → re-derive → verify
fingerprint → acquire everything through the verified stores (exact digests
deduplicated) → projected compatibility against the *target* game identity →
revalidate on-disk state → receipt + registry `installing` → (game/loader
transition through the ordinary verified pipeline if the pack changes
Minecraft or Fabric Loader) → stage, verify, retire-with-backup, activate →
commit content state, pack state, then the registry record
(ready-with-new-identity = the commit) → validate → remove receipt. Every
mutation is backed up and rolled back on any failure, including re-installing
the previous game tree when a transition had committed; an unprovable
rollback leaves the instance `installing` and reports
`pack_update_rollback_failed` rather than pretending to be ready.

Interruption (crash) recovery uses the receipt: if the new state fully
validates, the commit is finalized; if the untouched old state fully
validates, it is restored to ready (with launcher-owned staging debris
cleaned); neither proving out leaves the instance unavailable for
inspection.

## UI

The Overview page of a ready pack instance shows the installed pack
identity, a "Modified" indicator with the recorded divergences when present,
and a **Check for update** action. An available update opens a review panel:
version transition summary, added/updated/removed/preserved/conflict counts,
the game transition when present, provider changelog (plain text), an
expandable full row list, explicit per-conflict choices (Keep my file / Use
new pack version — removal conflicts offer keep-local only), and an Update
Modpack action disabled until every conflict is resolved. Progress is
phase-truthful from native events (checking, downloading, verifying,
installing game, applying, validating); completion refreshes the instance.
No polling, watchers, timers, or startup network exist anywhere in the
feature.

## Threat model additions

Reconciliation-specific protections: retirement requires byte proof plus
ownership-graph proof; shared files are never deleted for pack cleanliness;
divergent user bytes are never silently overwritten (explicit useNewPack
only, with backup); stale-plan attacks fail because every apply re-derives
authority and compares observed local digests; filename similarity is never
identity; the receipt never authorizes mutation — only validated recovery.
All Phase I parser guarantees (path allowlist, archive bounds, digest
verification, approved hosts) apply unchanged to the candidate archive.

## Known limitations

- Update discovery offers only the newest published version (skipping to an
  older intermediate version is not offered).
- A hard crash between the game-tree replacement and the document commit can
  leave an instance neither recovery case can prove; it stays unavailable for
  inspection.
- Optional-file selection carries by path; a brand-new optional file stays
  unselected through an update (opt-in happens at install).
- User-facing historical rollback (restoring pack v1 after v2) is deferred;
  the rollback machinery exists for transaction safety only.
