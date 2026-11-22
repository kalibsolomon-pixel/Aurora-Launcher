# Aurora Launcher 2.6.0

**A cleaner, redesigned interface.** Every surface across Home, Instances,
Discover, Pulsar, Skins, Sync, and Settings follows one restrained visual
system with full-pill active states, a single violet accent, status-only green
indicators, floating bottom-right download toasts, and a compact Home hero.

**Managed Ephemera Engine bundling.** Fabric instances with Aurora Client can
now opt into the Ephemera rendering engine during creation or manage it from
Instance Options with automatic renderer conflict checks (Sodium, Iris,
VulkanMod, Nvidium), in-place updates, and clean removal.

**Worlds, Mod Bisect, Storage Manager & Command Palette.** Manage singleplayer
worlds, backups, datapacks, and Quick Play directly from each instance's Worlds
tab; isolate crashing mods with binary-search Mod Bisect; inspect and safely
prune unreferenced caches and unused managed Java runtimes in Settings →
Storage; jump anywhere with `Ctrl+K`; duplicate or export `.mrpack` archives;
and launch Offline Play when signed out.

**Pulsar PvP combat breakdown & annotations.** Synced with Aurora Client 3.7.0
analytics with structured Key takeaways, Defense health/absorption timelines,
Flow exchange tables, and per-fight notes and tags.

**More reliable updates across the stack.** Titlebar chips now surface both
Launcher and Aurora Client updates directly to Settings → Updates, in-instance
Content → Updates shows readable mod and pack names while preserving
post-update notices and keeping review buttons visible, and launcher update
checks guard against concurrent instance operations.

**Faster downloads and smaller builds.** Core metadata and artifact transfers
use HTTP/2, buffered verified streaming, and parallel library acquisition,
while compressed bundled icon tables and tuned release optimization shrink the
native binary and reduce memory use.

Update trust, application identity and managed-data locations remain unchanged.
Windows publisher signing remains unconfigured; signed update artifacts provide
release authentication.
