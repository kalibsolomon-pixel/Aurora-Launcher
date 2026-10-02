# Phase K NeoForge research (authoritative sources, 2026-10-02)

All findings verified live against maven.neoforged.net artifacts and a real
boot of the game. Sources: maven versions API, real installer jars
(21.1.252, 26.2.0.88, 26.1.2.112), real universal jars, official installer
run in a throwaway directory, and a launcher-style boot of MC 26.2 +
NeoForge 26.2.0.88 (probe log + screenshot in this directory).

## Version discovery
- Official source: https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge
  JSON {isSnapshot:false, versions:[...]} oldest-first (1766 entries today).
- Maven artifact version = BARE NeoForge version (no MC prefix).
- Version scheme (verified): 3 components "A.B.C" with A in {20,21} ->
  Minecraft "1.A.B"; 4 components "A.B.C.D" (26.x generation) ->
  Minecraft "A.B.C" if C != 0 else "A.B". Anything else (e.g. the
  0.25w14craftmine April-Fools line) is rejected as unparseable.
- Verified examples: 21.1.252 -> 1.21.1; 21.11.45 -> 1.21.11;
  26.2.0.88 -> 26.2; 26.1.2.112 -> 26.1.2 (install_profile.minecraft).
- Prerelease suffix "-beta"/"-alpha" ranks below stable; plain = stable.
- Promotions API is gone (404). No recommended/promoted labels exist now;
  newest stable per MC version is the selection policy.
- Current stable lines: 26.1.2.112 (MC 26.1.2), 26.2.0.88 (MC 26.2);
  MC 26.3 has only 26.3.0.40-beta so far.

## Artifact layout (per version directory)
neoforge-<v>-installer.jar (+ .sha1/.sha256/.sha512/.md5/.asc sidecars),
-universal.jar, -moddev-config.json, .module, .pom, -sources.jar, -userdev.jar.
NO client.jar anymore. Every file has official sha1/sha256 sidecars.

## Installer contents (verified by extraction)
Root: install_profile.json, version.json, url.png, big_logo.png,
lekeystore.jks, data/client.lzma (binpatches), maven/ entries.
install_profile.json: spec=1, profile="NeoForge", version="neoforge-<v>",
minecraft="<MC version>" (authoritative mapping), json="/version.json",
data{token -> {client,server} values}, processors[], libraries[]
(maven artifacts with url+sha1+size), serverJarPath.
version.json: id, inheritsFrom=<MC>, mainClass, arguments{jvm,game}
(plain strings; placeholders ${library_directory}, ${classpath_separator},
${version_name} + standard Mojang game placeholders), libraries[]
(maven with url+sha1+size+path). NO javaVersion (Mojang's is authoritative).

## Processor model (client side)
- 26.x generation: 2 processors; only PROCESS_MINECRAFT_JAR applies to the
  client (sides null): --input vanilla client.jar --output
  [net.neoforged:minecraft-client-patched:<v>] --extract-libraries-to
  <libraries> --apply-patches installer-embedded data/client.lzma.
  No network inside the processor.
- 21.x generation (1.21.x): 6 client-side processors incl. DOWNLOAD_MOJMAPS
  (network), MERGE_MAPPING, jarsplitter, AutoRenamingTool, binarypatcher.
  -> Phase K scopes discovery/install to the 26.x generation; 1.21.x-era
  NeoForge is deliberately not offered (truthful no-compatible result).
- Processor arg grammar: {TOKEN} -> data map (side-specific); [g:a:v[:c][@ext]]
  -> artifact path under libraries root; '/path' -> installer-embedded file;
  'literal' (quoted) -> literal; bare -> literal.
- Outputs are artifact-path targets validated by existence + recorded
  observed SHA-256; processors whose token-artifacts all exist with recorded
  digests are skipped (retry/reuse semantics).

## Launch model (verified by real boot, probe-log.txt + screen.png)
- classpath = Mojang platform libraries + NF version.json libraries +
  VANILLA client.jar LAST (inheritsFrom semantics).
- mainClass from version.json: 26.x = net.neoforged.fml.startup.Client.
- JVM args: Mojang's then NF's appended. Game args: Mojang's then NF's.
- FML discovers via -DlibraryDirectory (NF jvm arg):
  libraries/net/neoforged/neoforge/<v>/neoforge-<v>-universal.jar AND
  libraries/net/neoforged/minecraft-client-patched/<v>/minecraft-client-patched-<v>.jar
  -> both must be materialized in the managed libraries tree but are NOT
  classpath entries.
- Log proof: "minecraft (jar(~libraries/.../minecraft-client-patched-...))",
  "NeoForge mod loading, version 26.2.0.88, for MC 26.2",
  TRANSFORMER/minecraft@26.2 frames from the patched jar.
- The official installer writes only versions/neoforge-<v>/<id>.json (the
  profile) for legacy-launcher integration; Aurora instead composes its own
  normalized plan and never writes launcher profiles.

## Mojang side (needed for MC 26.x)
- MC versioning moved to year-based: 26.1/26.2/26.3 releases exist.
- 26.2 document: javaVersion component java-runtime-epsilon major 25.
- Natives: rule-gated classifier-in-name entries (same shape Aurora already
  handles for 1.21.11), BUT java.library.path is now
  ${natives_directory}/java (new subdirectory convention; also /jna /lwjgl
  /netty extraction targets). Aurora must derive the natives subdirectory
  from the planned -Djava.library.path template (1.21.x = flat).
- log4j stays a normal Mojang library (the installer does not download
  Mojang libraries; the launcher does - same as Aurora's model).

## Integrity model
- NF maven publishes sha1 (and sha256) sidecars for every artifact.
- version.json + install_profile libraries carry inline sha1+size (like
  Mojang) -> SHA-1 verified acquisition through the existing sha1 store.
- installer.jar and universal.jar need sidecar sha1 fetches (2 requests).
- Processor-generated artifacts (minecraft-client-patched) are honestly
  recorded as observed SHA-256 (SecureTransportObserved-class semantics:
  locally observed consistency reference, never "officially verified").

## Live boot evidence
- probe-log.txt: full FML/mod-loading log; neoforge-version.txt; screen.png
  (Minecraft title window "Minecraft NeoForge 26.2", first-launch
  accessibility dialog).
- Boot environment: Temurin 25, windows x64, vanilla client jar on
  classpath, natives extracted to <natives>/java.
