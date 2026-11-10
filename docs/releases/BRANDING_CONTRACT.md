# Aurora Launcher naming and upgrade compatibility

The owner confirmed the permanent public name on 2026-11-10. Public application,
release, installer, shortcut and Windows resource names use Aurora Launcher.
The application executable is `aurora-launcher.exe`; installers use
`Aurora.Launcher_<version>_x64-setup.exe`. Release titles start with
`Aurora Launcher <version>`. The Cargo package and source directory names are
internal implementation details.

## Compatibility boundary

Version 2.1.1 introduces the corrected public names. Published releases through
2.1.0 are immutable. Their installer URLs and signature comments retain the
original filenames. Release verification selects exactly one installer filename
for each version, so previously signed authority remains verifiable without
allowing alternate names, arbitrary download URLs, or same-version drift.

The updater shipped in 2.1.0 checks an exact earlier filename. It cannot accept
the corrected 2.1.1 download name. Users of 2.1.0 and earlier must manually run
the 2.1.1 installer once. Later versions use the signed updater. No old-name
compatibility download alias is published for new releases, and no historical
release or tag is rewritten.

## Installation migration

The installer keeps package identity `com.aurora.launcher.native-preview`, channel
`native-preview`, fixed update authority, signing trust and application-data/
credential paths unchanged. Its existing internal installation directory is also
retained to prevent collisions with the archived WebView installation. That
directory's compatibility name is not the public product name.

For recognized earlier packages, the installer verifies the package marker,
checks that the old executable is not in use, stages the replacement, and moves
the old executable into the established recovery slot. Activation installs
`aurora-launcher.exe`. The executable rollback restores the original filename
when activation fails. Both executable names at once, unknown package ownership,
unknown earlier versions, redirected paths or interrupted transaction slots
require explicit recovery and preserve the existing files.

The Start menu entry is `Aurora Launcher/Aurora Launcher.lnk`. Shortcuts left by
the earlier package are retired only when they target the exact installation
directory. A desktop shortcut owned by another application stays untouched.
Worlds, instance data, settings and credentials are outside the installer scope.
The installer does not promise a power-loss-atomic transaction across every
metadata, notice, shortcut and executable write.

## Permanent checks

Distribution packaging rejects a different executable basename, ProductName,
FileDescription or OriginalFilename. Release tests cover corrected future names,
the version boundary, previous signed authority, release titles and retained
protection gates. Installer acceptance exercises migration, ambiguous files,
unknown ownership, in-use files, foreign shortcuts, downgrade refusal, handoff
and uninstall preservation in an isolated package identity.

Historical reports keep their original names, hashes and claims as evidence.
The owner-attended Modrinth matrix remains waived by the owner, not run or passed.
The naming change does not waive protected environment approvals or acceptance
of the exact production-signed artifact.
