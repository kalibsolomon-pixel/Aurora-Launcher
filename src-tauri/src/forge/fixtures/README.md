# Official F1C fixtures

Retrieved 2026-10-12 from the official MavenForge repository. JSON is
pretty-printed; only the GUI installer's base64 `icon` is omitted. All
operational fields, ordering and published hashes are retained. These
are metadata fixtures, not executable artifacts or an independent signature.

| Exact pin | Installer bytes | Published installer SHA-1 |
| --- | --- | --- |
| 1.20.1-47.4.10 | 6,078,070 | 66bfea9963bfa60d88bab6b2750e74a958392715 |
| 1.20.4-49.2.0 | 5,535,650 | 01615b3e329bf23b1207de18b4c6f14cd897fde5 |

Source pattern: `https://maven.minecraftforge.net/net/minecraftforge/forge/<pin>/forge-<pin>-installer.jar`, with `.sha1` sidecars.
Both exact pins are recommended in the official promotions document at
retrieval. Discovery alone does not establish Aurora gameplay acceptance.

Mojang documents are unmodified bytes retrieved via the pinned piston-meta
version manifest and SHA-1-checked: 1.20.1 `760a390e336b2b6202ec5906757cee3a929a7c85`;
1.20.4 `868b02c961f819c9af215d6c628cb0a08d7591df`.
Both require `java-runtime-gamma` / Java 17.

## Assessment before parser changes

The existing spec-1, side-mapped data, quoted values, Maven extensions,
embedded patch bundle, ordered processor runner, SHA-1 store, receipts,
managed Java and shared game composition are reusable.

1.20.4 uses five client steps: MCP_DATA, DOWNLOAD_MOJMAPS (sanitized),
MERGE_MAPPING, ForgeAutoRenamingTool, binarypatcher. All five outputs
have published expectations. Its ForgeBootstrap / forge_client launch
shape matches F1B. MCP_DATA introduces an inline `[coordinate@zip]`
processor input, which must become a validated path, never a literal.

1.20.1 uses six steps: MCP_DATA, DOWNLOAD_MOJMAPS, MERGE_MAPPING,
jarsplitter (two outputs), ForgeAutoRenamingTool, binarypatcher. Its
BootstrapLauncher / forgeclient launch has explicit module-path and
FML identity arguments. `_comment_` is the older comment spelling.
Tool-only libraries include different versions of game libraries and
must stay out of shared game-library collision/launch handling. Several
generated resources are discovered by the FML loader through exact
library paths rather than declared as launch libraries. PATCHED_SHA is
published in data even though binarypatcher has no outputs map; it must
be retained as an expected digest. Unhashed mapping/rename intermediates
remain honest local observations. The Mojang download tool verifies its
official mapping download; this needs source inspection and native evidence.

The official Installer's PostProcessors resolves whole bracket arguments
against the library directory and whole brace arguments against data;
Aurora must reject unresolved fragments. No new executor, historical
native normalization, schema migration or Java override is required.
Older generations remain investigation-only.
