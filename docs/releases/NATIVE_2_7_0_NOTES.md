# Aurora Launcher 2.7.0

**Warn-then-comply conflict resolution.** Installing Ephemera Engine when Sodium,
Iris, VulkanMod, or Nvidium is present no longer refuses outright: Instance
Options informs you of the conflicting renderer mod(s) and lets you either
safely disable them to `.disabled` (reversible anytime in Mods) and install
Ephemera, or install Ephemera while keeping existing mods enabled. Re-enabling
or updating a previously disabled Ephemera copy also transitions cleanly back to
active status. Discover content reviews apply the same principle: user/provider
mod conflicts (including bidirectional `breaks` and optional Ephemera) list the
conflicting files and offer an explicit opt-in toggle to disable conflicting
mods and continue, while keeping Aurora Client and required bootstrap artifacts
protected.

**Redesigned Home hero & roomier desktop proportions.** The Home hero pairs a
prominent `112px` instance icon with a unified split **Play** button that builds
the instance switcher directly into its right side and displays the selected
instance underneath, without Aurora Client clutter. Controls, sidebar items,
Discover browse cards, installed Content rows (with on/off toggles placed beside
Delete), and Home Singleplayer World / Server widgets are taller and better
proportioned, with primary actions consistently on the left and minimal status
indicators on the right.

**Faster, flicker-free verification & navigation.** Readiness checks parallelize
file verification across up to 16 workers, overlap mod compatibility scanning
with Java runtime validation, and cache per-instance readiness in the UI so
switching instances or refreshing never flashes transient loading states.
Browser-style Back/Forward navigation and bundled SF Pro Text typography make
moving between views smoother.

**Autonomous Client & Fabric Loader updates.** Compatible Aurora Client updates
can automatically upgrade Fabric Loader from official Fabric Meta and
transactionally replace existing Aurora Client JARs with rollback safety.

**Reliability fixes.** Fixed a lock-ordering deadlock between instance
duplication / Ephemera saves and concurrent instance operations, cleared stale
readiness cache entries on account switches and deletions, and eliminated false
unsaved-changes banners in Sync Groups.

Update trust, application identity and managed-data locations remain unchanged.
Windows publisher signing remains unconfigured; signed update artifacts provide
release authentication.
