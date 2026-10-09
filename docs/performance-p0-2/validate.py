"""Read-only P0.2 evidence checks. Optional artifacts are task-created copies only."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
import re
import sys

sys.dont_write_bytecode = True
import analyze

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
EVIDENCE = HERE / "evidence"
SOURCE = "6cbabd38bc1c5268b9f6c80034f18da3a26f1e21"
CLIENT = "5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d"
FORBIDDEN_KEYS = {
    "username", "accountid", "profileid", "email", "password", "token",
    "accesstoken", "refreshtoken", "authorizationcode", "requestbody",
    "launcharguments", "arguments", "environmentvariables", "serveraddress",
    "socketaddress", "privatepath", "rawerror", "stderr", "pid", "processid",
}


def load(name):
    return json.loads((EVIDENCE / name).read_text(encoding="utf-8-sig"))


def privacy(value):
    if isinstance(value, dict):
        for key, item in value.items():
            assert key.lower() not in FORBIDDEN_KEYS, "forbidden metadata field"
            privacy(item)
    elif isinstance(value, list):
        for item in value:
            privacy(item)
    elif isinstance(value, str):
        assert not re.search(r"(?i)[a-z]:[\\/]|/Users/|/home/|\\\\", value), "absolute/private path"
        assert not re.search(r"(?i)bearer\s+|eyJ[a-zA-Z0-9_-]{20,}\.", value), "credential-like text"
        assert not re.search(r"[\w.+-]+@[\w.-]+\.[a-zA-Z]{2,}", value), "email-like text"
    elif isinstance(value, float):
        assert value == value and abs(value) != float("inf"), "non-finite numeric value"


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts-root", type=Path)
    parser.add_argument("--client-root", type=Path, default=REPO.parent / "Aurora-Client")
    args = parser.parse_args()
    for path in EVIDENCE.glob("*.json"):
        privacy(load(path.name))

    trials = load("trials.json")
    assert len(trials) == 18
    assert len({(r["cohort"], r["trial"]) for r in trials}) == 18
    assert all(r["success"] and r["closedNormally"] and r["competingLauncherOrJava"] == 0 for r in trials)
    assert sum(r["phase"] == "pilot" for r in trials) == 4
    for cohort in ("profile", "control"):
        assert {r["trial"] for r in trials if r["cohort"] == cohort and r["phase"] == "matched"} == set(range(3, 9))
    assert sum(r["cohort"] == "ipc" for r in trials) == 2
    assert not list(EVIDENCE.glob("trace-control-*.json"))
    summary = load("summary.json")
    paths = sorted(EVIDENCE.glob("trace-*.json"))
    assert len(paths) == 10
    assert {p.name: analyze.trace(p) for p in paths} == summary["traces"], "derived trace summary differs"
    assert summary["allTracedNativeCounts"]["readiness"] == 20
    assert summary["allTracedNativeCounts"]["gameIntegrity"] == 40
    assert summary["primaryNativeCounts"]["readiness"] == 12
    assert summary["primaryNativeCounts"]["gameIntegrity"] == 24
    assert summary["newPlayTrials"] == 0
    assert summary["pairedProfileMinusControlStableReady"]["n"] == 6
    assert summary["pairedProfileMinusControlStableReady"]["p95Ms"] is None
    for name in ("trace-ipc-01.json", "trace-ipc-02.json"):
        assert summary["traces"][name]["ipcRtt"]["n"] == 50

    artifact_files = 0
    for cohort in ("profile", "control", "ipc"):
        manifest = load(cohort + "-artifact.json")
        assert manifest["sourceHead"] == SOURCE and manifest["cohort"] == cohort
        assert re.fullmatch(r"[0-9a-f]{64}", manifest["executableSha256"])
        rows = manifest["frontendFiles"]
        # The Windows collector sorted Path objects (component/case-insensitive).
        assert rows == sorted(rows, key=lambda r: PureWindowsPath(r["path"]))
        assert len({r["path"] for r in rows}) == len(rows)
        for row in rows:
            relative = PurePosixPath(row["path"])
            assert not relative.is_absolute() and ".." not in relative.parts
            assert "\\" not in row["path"] and ":" not in row["path"]
            assert re.fullmatch(r"[0-9a-f]{64}", row["sha256"])
            assert type(row["bytes"]) is int and row["bytes"] > 0
        canonical = json.dumps(rows, sort_keys=True, separators=(",", ":")).encode()
        assert hashlib.sha256(canonical).hexdigest() == manifest["frontendTreeSha256"]
        if args.artifacts_root:
            root = args.artifacts_root / cohort
            executable = root / "aurora-launcher.exe"
            assert sha(executable) == manifest["executableSha256"]
            assert executable.stat().st_size == manifest["executableBytes"]
            for row in rows:
                file = root / "frontend" / row["path"]
                assert file.stat().st_size == row["bytes"] and sha(file) == row["sha256"]
                artifact_files += 1

    build = load("build-environment.json")
    assert build["sourceHead"] == SOURCE
    assert build["releaseIdentity"] == {"version": "1.4.1", "identifier": "com.aurora.launcher"}
    assert build["updaterTrust"]["trustChanged"] is False
    client = load("client-final-provenance.json")
    assert client["sourceHead"] == CLIENT and client["testsRun"] == 443 and client["testFailures"] == 0
    assert client["builtJarSha256"] == client["deployedJarSha256"]
    assert client["allEntryBytesMatch"] and not client["installedJarModified"]
    for path in EVIDENCE.glob("*-preservation.json"):
        for domain in load(path.name):
            assert domain["unexpectedCount"] == domain["removedTrackedCount"] == 0
            assert domain["changedCount"] == (18 if domain["domain"] == "launcher" else 0)

    critical = {
        REPO: ("AGENTS.md", "ARCHITECTURE.md", "README.md", "package.json", "package-lock.json",
               "vite.config.js", "svelte.config.js", "tsconfig.json", "src-tauri/Cargo.toml",
               "src-tauri/Cargo.lock", "src-tauri/tauri.conf.json", "src-tauri/build.rs"),
        args.client_root: ("AGENTS.md", "ARCHITECTURE.md", "README.md", "build.gradle",
                           "settings.gradle", "gradle.properties", "gradlew", "gradlew.bat",
                           "gradle/wrapper/gradle-wrapper.jar", "gradle/wrapper/gradle-wrapper.properties"),
    }
    for root, names in critical.items():
        assert all((root / name).is_file() for name in names), "critical repository file missing"
    links = 0
    for document in (REPO / "PERFORMANCE_RESEARCH_P0_2.md", HERE / "README.md"):
        for target in re.findall(r"\]\(([^)]+)\)", document.read_text(encoding="utf-8")):
            if target.startswith(("https://", "http://", "#")):
                continue
            assert (document.parent / target.split("#", 1)[0]).is_file(), "broken local evidence link"
            links += 1
    print(json.dumps({"numericTraceSchema": "pass", "traces": len(paths),
                      "metadataPrivacyPatterns": "pass", "artifactManifestFingerprints": "pass",
                      "retainedArtifactBytesChecked": bool(args.artifacts_root),
                      "frontendArtifactFilesChecked": artifact_files,
                      "criticalRepositoryFiles": sum(map(len, critical.values())),
                      "resolvedLocalLinks": links, "newGameLaunches": 0}, indent=2))


if __name__ == "__main__":
    main()
