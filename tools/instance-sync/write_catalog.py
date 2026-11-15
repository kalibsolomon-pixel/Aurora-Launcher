"""Turn inspected format evidence into the reviewed pre-implementation catalog."""
import argparse
import json
import pathlib
import re
import subprocess

from inspect_formats import ClassFile
import zipfile

VERSIONS = ("26.1.2", "26.2", "26.3")
LOCAL = {
    "soundDevice": "Device identity",
    "lastServer": "Connection history",
    "tutorialStep": "Local tutorial progress",
    "onboardAccessibility": "Local onboarding state",
    "joinedFirstServer": "Local progress",
    "skipMultiplayerWarning": "Warning acknowledgement",
    "telemetryOptInExtra": "Consent",
    "allowServerListing": "Privacy choice",
    "onlyShowSecureChat": "Trust/privacy choice",
    "hideMatchedNames": "Trust/privacy choice",
    "sharePresence": "Privacy choice",
    "hideServerAddress": "Privacy choice",
    "glDebugVerbosity": "Diagnostics",
    "debugGuiScale": "Diagnostic display",
    "useNativeTransport": "Native transport selection",
    "startedCleanly": "Startup bookkeeping",
    "incompatibleResourcePacks": "Target compatibility acknowledgement; never grant acceptance",
    "overrideWidth": "Machine window override",
    "overrideHeight": "Machine window override",
    "fullscreenResolution": "Machine display mode",
    "preferredGraphicsBackend": "Renderer capability differs; no cross-version backend adapter",
    "macFullscreenMenuVisibility": "Platform-specific behavior",
    "quitShortcuts": "Platform-specific behavior",
    "ctrlClickEmulatesRightClick": "Platform-specific behavior",
    "realmsNotifications": "Account service notification preference; keep local initially",
    "inGameNotification": "Account service notification preference; keep local initially",
}
CONTROLS = set("autoJump invertXMouse invertYMouse toggleCrouch toggleSprint toggleAttack toggleUse sprintWindow mouseSensitivity mouseWheelSensitivity discrete_mouse_scroll rawMouseInput touchscreen allowCursorChanges".split())
GRAPHICS = set("ao biomeBlendRadius chunkSectionFadeInTime cloudRange cutoutLeaves enableVsync entityDistanceScaling entityShadows exclusiveFullscreen fullscreen gamma graphicsPreset improvedTransparency inactivityFpsLimit maxAnisotropyBit maxFps mipmapLevels particles prioritizeChunkUpdates renderClouds renderDistance simulationDistance textureFiltering vignette weatherRadius syncChunkWrites menuBackgroundBlurriness".split())
PROTECTED_AURORA = {
    "playtimeTotalMs", "playtimePerWorld", "fightStatsTotalFights", "fightStatsPlaytimeSeconds",
    "migratedHitregProperties", "migratedEffectExpiryExclusions", "waypointsByWorld", "waypointNavigationByWorld",
}


def constant(op, value):
    if 2 <= op <= 8:
        return op - 3
    if op in (14, 15):
        return float(op - 14)
    if op in (16, 17):
        return int.from_bytes(bytes.fromhex(value), "big", signed=True)
    if op in (18, 19, 20) and isinstance(value, (int, float, str)):
        return value
    return None


def enum_codec(owner, support):
    entry = support.get(owner)
    if not entry:
        return None
    ins = entry["methods"].get("<clinit>", [])
    is_int = any(op == 178 and value[0] == "com/mojang/serialization/Codec" and value[1][0] == "INT" for _, op, value in ins)
    is_string = any(op == 184 and value[0] == "net/minecraft/util/StringRepresentable" and value[1][0] == "fromEnum" for _, op, value in ins)
    values, current = [], []
    for _, op, value in ins:
        if op == 187 and value == owner:
            current = []
        c = constant(op, value)
        if c is not None:
            current.append(c)
        if op == 183 and value[0] == owner and value[1][0] == "<init>" and len(current) >= 3:
            values.append(current[2])  # name, ordinal, serialized id/name
    if is_int and values and all(isinstance(v, int) for v in values):
        return "JSON integer {" + ", ".join(map(str, values)) + "}"
    if is_string and values and all(isinstance(v, str) for v in values):
        return "JSON string {" + ", ".join(values) + "}"
    return None


def codec(key, item, support):
    desc, init = item["descriptor"], item["initializer"]
    encoded = str(init)
    if desc == "Z":
        return "boolean true/false (raw)"
    if desc == "I":
        return "signed int32 (raw)"
    if desc == "Ljava/lang/String;":
        return "UTF-8 string (raw)"
    if key in ("resourcePacks", "incompatibleResourcePacks") and "readListOfStrings" not in encoded:
        return "JSON string array (FieldAccess reader/writer; activation planner only)"
    if "java/lang/Boolean" in str(item["fieldSignature"]):
        if "createBoolean" in encoded or "BOOLEAN_VALUES" in encoded:
            return "JSON boolean true/false"
        return None
    if key == "fov" and "DOUBLE" in encoded:
        return "JSON double [-1,1]; degrees=int(saved*40+70), saved=(degrees-70)/40"
    if key in ("guiScale", "debugGuiScale") and "ClampingLazyMaxIntRange" in encoded:
        return "JSON integer [" + ("-1" if key == "debugGuiScale" else "0") + ",2147483646]; target clamps to window maximum, 0=auto"
    if key in ("renderDistance", "simulationDistance") and "OptionInstance$IntRange" in encoded:
        return "JSON integer [" + ("2" if key == "renderDistance" else "5") + ",16 or 32]; heap/debug-dependent target limit, out-of-range skip"
    for i, (op, value) in enumerate(init):
        if op == 184 and value[0] == "com/mojang/serialization/Codec" and value[1][0] in ("intRange", "doubleRange"):
            if key == "mouseWheelSensitivity":
                return "JSON double [0.01,10]; logMouse(n)=10^(n/100)"
            a, b = (constant(*x) for x in init[i - 2:i])
            if isinstance(a, (float, int)) and isinstance(b, (float, int)):
                return f"JSON {'integer' if value[1][0] == 'intRange' else 'double'} [{a:g},{b:g}]"
    if "OptionInstance$UnitDouble" in encoded:
        suffix = "; saved=sqrt(effective), effective=saved^2" if key == "darknessEffectScale" else ""
        return "JSON double [0,1]; legacy boolean accepted as 0/1" + suffix
    for i, (op, value) in enumerate(init):
        if op == 183 and value[0] == "net/minecraft/client/OptionInstance$IntRange" and value[1][0] == "<init>" and "xmap" not in encoded:
            count = 3 if value[1][1] == "(IIZ)V" else 2
            values = [constant(*x) for x in init[i - count:i]]
            if all(isinstance(x, int) for x in values):
                return f"JSON integer [{values[0]},{values[1]}]; validate effective range, not codec's extra endpoint"
    for op, value in init:
        if op == 178 and isinstance(value, list) and value[1][0] in ("CODEC", "LEGACY_CODEC"):
            result = enum_codec(value[0], support)
            if result:
                return result + ("; legacy boolean false=off,true=fancy" if key == "renderClouds" else "")
    return None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=pathlib.Path, required=True)
    parser.add_argument("--client", type=pathlib.Path, required=True)
    parser.add_argument("--loom", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    evidence = json.loads(args.evidence.read_text())
    data = evidence["minecraft"]
    lines = ["# Instance sync format catalog", "", "Verified 2026-11-15, before phase 1. DECISIONS.md governs classification.", "",
             "This is a format/portability decision record, not permission for the legacy opaque config writer to copy these files. Runtime catalogs must be explicit and release-bound. Unlisted keys, values outside a verified codec, unknown versions, unknown Client releases and unknown nested members remain per-instance with a visible reason. No Minecraft or launcher was run to collect this evidence; no owner instance/data was opened.", "",
             "## Evidence and method", "",
             "Minecraft client JARs came from the read-only Gradle cache `C:/Users/kalib/.gradle/caches/fabric-loom/<version>/minecraft-client.jar`. Each SHA-1 matches its official Mojang version document. The version documents were fetched over HTTPS from their manifest URLs, checked against manifest SHA-1 and compared semantically with cached metadata. These are Mojang SHA-1 identities, not invented SHA-256 publisher checks.", "",
             "| Version | Official client SHA-1 | Options data version |", "|---|---|---|"]
    for version in VERSIONS:
        d = data[version]
        lines.append(f"| {version} | `{d['clientSha1']}` | {d['version']['world_version']} |")
    lines += ["", "`tools/instance-sync/inspect_formats.py` reads JVM class files in Python without starting Java. It records serialized field names and FieldAccess overloads directly from `Options.processOptions` / `processDumpedOptions`, initializer instructions, generic signatures and codec/enum/input support classes. Decompiled 26.2/26.3 source JARs in the Client Loom cache were inspected as a second representation. 26.1.2 decisions use its actual class bytecode, not an assumed copy of 26.2. Evidence and repeatable tooling are checked in under `docs/instance-sync/` and `tools/instance-sync/`.", "",
              "Aurora source: `C:/Dev/Aurora-Client`, read-only. Published tag `v3.5.0` resolves to `" + evidence['aurora']['v3.5.0']['commit'] + "`; development HEAD is `" + evidence['aurora']['HEAD']['commit'] + "`. `gradle/minecraft-versions.json` supports all three requested lines. A release tag verifies the source format; installed artifact identity must still match the release adapter before writes. The development-only Pulsar additions are not evidence that the published release supports them.", "",
              "## Vanilla per-key decisions", "", "Each version cell gives its independently inspected codec; `absent` means do not inject an alias. Effective range checks must be strict; the game's broader lenient reader is not permission to silently clamp or wrap an unsupported preference. No graphics opt-in: verified graphics/tuning fields sync by default under the owner decision.", "",
              "| Serialized key | 26.1.2 | 26.2 | 26.3 | Decision / reason |", "|---|---|---|---|---|"]
    keys = sorted(set().union(*(d["keys"] for d in data.values())))
    for key in keys:
        cells = []
        for version in VERSIONS:
            item = data[version]["keys"].get(key)
            cells.append((codec(key, item, data[version]["support"]) or "unverified codec; local") if item else "absent")
        if key in LOCAL:
            decision = "Local: " + LOCAL[key]
        elif any("unverified" in c for c in cells):
            decision = "Local on unverified version(s); verified codecs only elsewhere"
        elif key == "resourcePacks":
            decision = "Sync enablement/order by content identity; one coordinated options output"
        else:
            decision = "Sync " + ("controls" if key in CONTROLS else "graphics/performance" if key in GRAPHICS else "preferences") + "; skip absent versions"
        lines.append(f"| `{key}` | " + " | ".join(cells) + " | " + decision + " |")
    lines += ["| `version` | raw integer 4790 | raw integer 4903 | raw integer 5023 | Always local; preserve target data version. For missing options omit until target format is established. |",
              "| `fullscreenResolution` | raw display-mode string | raw display-mode string | raw display-mode string | Always local machine display mode. |", "",
              "`graphicsMode`, `subtitles` and arbitrary old aliases are not emitted under those names in these versions. Do not translate them speculatively. `touchscreen` exists only in 26.1.2; `rawMouseInput` only in 26.1.2/26.2. Never equate an absent key with false. FPS cap 260 represents the game's unlimited cutoff, not an arbitrary unbounded integer. Render/simulation distances must be validated against the target heap/capabilities. Fullscreen and VSync are preferences; physical resolution overrides remain local.", "",
              "## Enumerated audio, model parts and vanilla controls", "",
              "Dynamic prefixes are not open-ended allowlists. Emit only the exact names verified below. `soundCategory_*` uses the Options sound slider UnitDouble codec (finite [0,1], legacy bool decode); model parts use raw booleans. Unknown mod sound channels remain local.", "",
              "| Family | 26.1.2 | 26.2 | 26.3 | Decision |", "|---|---|---|---|---|"]
    dynamic = {}
    for owner, prefix, decision in (("net/minecraft/sounds/SoundSource", "soundCategory_", "Sync audio"),
                                    ("net/minecraft/world/entity/player/PlayerModelPart", "modelPart_", "Sync appearance")):
        dynamic[owner] = {}
        for v in VERSIONS:
            ins = data[v]["support"][owner]["methods"]["<clinit>"]
            # Explicit lowercase serialized names, excluding enum identifiers.
            names = sorted({val for _, op, val in ins if op in (18, 19) and isinstance(val, str) and re.fullmatch(r"[a-z][a-z_]*", val)})
            dynamic[owner][v] = names
        for name in sorted(set().union(*(set(n) for n in dynamic[owner].values()))):
            lines.append(f"| `{prefix}{name}` | " + " | ".join("present" if name in dynamic[owner][v] else "absent" for v in VERSIONS) + f" | {decision} |")
    lines += ["", "Vanilla binding values are raw named strings resolved by InputConstants.getKey and saved by KeyMapping.saveString. GLFW 26.1.2/26.2 and SDL 26.3 use different numeric codes. Sync only named keyboard/mouse values present in both verified input tables, including the explicit unknown/unbound name. Numeric fallback names, arbitrary scancodes and missing named keys stay local. Mouse left/right/middle map to different numeric IDs in SDL; preserve their verified names. An action name alone does not establish a supported mod or binding codec.", "",
              "| Serialized binding | 26.1.2 | 26.2 | 26.3 | Decision |", "|---|---|---|---|---|"]
    for action in sorted(set().union(*(set(d["vanillaActions"]) for d in data.values()))):
        decision = "Local diagnostic binding" if action.startswith("key.debug.") else "Sync controls with verified named-key codec"
        lines.append(f"| `key_{action}` | " + " | ".join("present" if action in data[v]["vanillaActions"] else "absent" for v in VERSIONS) + f" | {decision} |")
    lines += ["", "## Aurora JSON projection", "",
              "`config/aurora.json` is a pretty-printed Gson object of public mutable AuroraConfig fields, no top-level schema version. `config/profiles/<sanitized name>.json` is a Gson Profile object. AuroraConfig.load runs migration pre-passes; ProfileManager.load subsequently applies activeProfile and can override config values. Capture and apply must validate the coherent config/profile/controls set, with profile-scoped active values taking precedence. Match exact release adapters, preserve unknown members, and reject malformed/ambiguous JSON rather than round-tripping it through a lossy DTO.", "",
              "Gson codecs: boolean is JSON boolean; int/long are exact bounded JSON integers; float/double are finite JSON numbers; String is a JSON string; enums use exact Java constant names; arrays/lists/sets are JSON arrays; maps keyed by strings are JSON objects whose values use their declared codec. ThemeDefinition, ModuleLayout, Waypoint and custom nested types require their release's field schema, not arbitrary JSON blob copying. Any unverified nested member stays local. Empty and absent/null values must not be conflated.", "",
              "The following table enumerates every top-level field of both reviewed AuroraConfig revisions. `Profile` means included by the Client's actual ProfileFieldSet, not a separate portability policy. All sync classifications apply both to config and matching profile fields. Counters, world data and guards are protected even if a Client profile includes them. perf fields sync in config although Client intentionally excludes them from profiles; do not invent profile entries the Client ignores.", "",
              "| Field | Gson Java value codec | v3.5.0 / dev HEAD line | Client scope | Projection decision |", "|---|---|---|---|---|"]
    aurora = {r: {x["field"]: x for x in d["fields"]} for r, d in evidence["aurora"].items()}
    excluded = {"activeProfile", "moduleGridView", "playtimeTotalMs", "playtimePerWorld", "fightStatsTotalFights", "fightStatsPlaytimeSeconds", "migratedHitregProperties", "migratedEffectExpiryExclusions", "profilesOwnControls", "vanillaKeyModifiers", "searchInstant", "searchTypoTolerance"}
    for name in sorted(set(aurora["HEAD"]) | set(aurora["v3.5.0"])):
        item = aurora["HEAD"].get(name) or aurora["v3.5.0"][name]
        datatype = item["javaType"].strip()
        if name in PROTECTED_AURORA:
            decision = "Local: world data" if "ByWorld" in name else "Local: one-shot migration guard" if name.startswith("migrated") else "Local: lifetime counters"
        elif name == "keybindVersion":
            decision = "Target codec marker; never copy as a preference"
        elif name not in aurora["v3.5.0"]:
            decision = "Local: development-only; no published release adapter verified"
        elif name == "activeProfile":
            decision = "Sync selection with coherent imported definitions"
        elif name == "vanillaKeyModifiers":
            decision = "Sync controls: verified action name -> modifier mask 0..7"
        elif name.endswith("Key") or name == "keystrokesExtraKeys":
            decision = "Sync controls: release-specific SDL/GLFW + KeyCombo codec"
        else:
            decision = "Sync tuning" if name.startswith("perf") or name.startswith(("frame", "worldMapCapture", "worldMapCache")) else "Sync preferences; validate nested members by release"
        location = " / ".join(str(aurora[r][name]["line"]) if name in aurora[r] else "absent" for r in ("v3.5.0", "HEAD"))
        scope = "Global" if name in excluded or name.startswith("perf") else "Profile"
        lines.append(f"| `{name}` | `{datatype}` | {location} | {scope} | {decision} |")
    lines += ["", "## Profile envelope, controls and protected fields", "",
              "| JSON member | Verified codec | Projection |", "|---|---|---|",
              "| `name` | String, sanitized filename identity | Sync named definition; collision requires preview and provenance. No overwrite of unrelated target-only profile. |",
              "| `createdAtMs` | long integer | Preserve definition provenance; not edit time or conflict authority. |",
              "| `fields` | LinkedHashMap<String,JsonElement> of config field names | Project only release-verified fields above. Retain target protected/world fields in every matching profile. |",
              "| `icon` | String | Sync metadata with definition. |",
              "| `hotkey` | int KeyCombo; -1 unbound | Sync only when controls enabled and format verified. |",
              "| `controls` | nullable Map<String,String> action -> vanilla saved key name | Sync only verified actions/key names when controls enabled. Preserve target-only/unsupported mappings. |", "",
              "KeyCombo uses low 16 bits for SDL scancode, bits 16..18 for Ctrl=1, Shift=2, Alt=4; -1 is unbound. KeybindMigrator.VERSION=1 and `keybindVersion` mark migrated SDL fields, and config/profile markers are independent. Unmarked legacy GLFW values require the exact mapping table in KeybindMigrator; unsupported keys (e.g. F25) stay local with a reason instead of silently becoming unbound. Do not copy a marker without converting its values. `vanillaKeyModifiers` uses action-name keys and low modifier mask 0..7. Aurora mapping registration must establish the mapping-name/config-field pair for the installed release; never synchronize all `key_key.aurora.*` solely by prefix.", "",
              "Controls off means retain target Aurora key fields, profile hotkeys/controls and vanilla modifiers together. Profiles off still requires a coherent target active profile projection so the next Client startup cannot undo changed settings. Preserve local-only definitions and never delete target-only profiles automatically. Active profile change/removal and name collisions must appear in preview; deleted or unknown-profile names are not permission to load factory defaults.", "",
              "Always excluded: `saves/`, world directories, level.dat/regions, `config/aurora-worldmap/`, both world maps in config and profiles, `phenomena/` fight/session data, standalone phenomena.json/kwamster.json imports, Hitreg import history, credentials, logs, backups and temporary files. No advanced selection can widen this authority.", "",
              "## Uncertainty and implementation gates", "",
              "- Unverified enum/number codec cells remain local on that version. Do not infer support from matching field names or a source initializer hash alone.",
              "- No shader-loader/version adapter has been verified. Pool shader files only; selection remains local with an explanation until a real loader format is reviewed.",
              "- Saved servers remain optional/off and additions-only; do not treat any NBT edit as scalar settings propagation.",
              "- Unknown Client releases and the development-only Pulsar fields remain local. The published v3.5.0 source is the initial candidate adapter, requiring artifact identity checks during implementation.",
              "- Scalar conflict clocks are capture/fully-terminated-exit times, never file mtimes. Newest capture wins; exact ties prefer main, then a stable instance-ID ordering. Join seeding is main-authoritative before edit-anywhere capture begins.",
              "- No runtime sync authority is enabled by this catalog. Durable stopped ownership, journaled backups/atomic commit/recovery and Restore are prerequisites for enabling the new writer.", ""]
    args.output.write_text("\n".join(lines), encoding="utf-8")
    print(f"catalog: {len(keys)} explicit option keys; {len(set(aurora['HEAD']) | set(aurora['v3.5.0']))} Aurora fields")


if __name__ == "__main__":
    main()
