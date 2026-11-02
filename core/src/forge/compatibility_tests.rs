//! Offline compatibility fixtures. Synthetic session/bytes are never executed.
use super::*;
use crate::forge::metadata::{ForgeVersionList, InstallProfileDocument, VersionProfileDocument};
use crate::forge::plan::{ForgeDataValue, ForgeProcessorArgument};
use crate::integrity::{ArtifactTrust, Sha1Digest};
use crate::minecraft::metadata::VersionDocument;
use crate::minecraft::rules::{Architecture, FeatureProfile, OperatingSystem};

fn documents(
    mc: &str,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    match mc {
        "1.20.1" => (
            "1.20.1-47.4.10",
            "66bfea9963bfa60d88bab6b2750e74a958392715",
            include_str!("fixtures/1.20.1-47.4.10-install_profile.json"),
            include_str!("fixtures/1.20.1-47.4.10-version.json"),
            include_str!("fixtures/1.20.1-mojang.json"),
        ),
        "1.20.4" => (
            "1.20.4-49.2.0",
            "01615b3e329bf23b1207de18b4c6f14cd897fde5",
            include_str!("fixtures/1.20.4-49.2.0-install_profile.json"),
            include_str!("fixtures/1.20.4-49.2.0-version.json"),
            include_str!("fixtures/1.20.4-mojang.json"),
        ),
        "1.21.11" => (
            "1.21.11-61.2.0",
            "ca682f0d537c65e9a112e1e7ab337095d486298f",
            include_str!("fixtures/1.21.11-61.2.0-install_profile.json"),
            include_str!("fixtures/1.21.11-61.2.0-version.json"),
            include_str!("fixtures/1.21.11-mojang.json"),
        ),
        _ => panic!("unsupported fixture"),
    }
}

fn platform() -> PlatformProfile {
    PlatformProfile::new(OperatingSystem::Windows, Architecture::X86_64)
}

#[test]
fn installer_entries_reject_links_traversal_oversize_and_clobber() {
    use std::io::Write as _;
    use zip::write::SimpleFileOptions;
    let root = std::env::temp_dir().join(format!("aurora-f1c-archive-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let jar = root.join("installer.jar");
    let mut archive = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
    archive
        .add_symlink(
            "data/client.lzma",
            "../outside",
            SimpleFileOptions::default(),
        )
        .unwrap();
    archive
        .add_symlink(
            "install_profile.json",
            "../outside",
            SimpleFileOptions::default(),
        )
        .unwrap();
    archive
        .start_file("data/regular.lzma", SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"verified fixture entry").unwrap();
    archive.finish().unwrap();
    let output = root.join("scratch/output");
    assert!(metadata::extract_installer_entry(&jar, "/data/client.lzma", &output).is_err());
    for name in [
        "/../outside",
        "/C:/outside",
        "/data\\outside",
        "//outside",
        "/data/missing",
    ] {
        assert!(metadata::extract_installer_entry(&jar, name, &output).is_err());
    }
    assert!(!output.exists());
    assert!(metadata::read_installer_documents(&jar).is_err());
    metadata::extract_installer_entry(&jar, "/data/regular.lzma", &output).unwrap();
    std::fs::write(&output, b"preserved pre-existing scratch bytes").unwrap();
    assert!(metadata::extract_installer_entry(&jar, "/data/regular.lzma", &output).is_err());
    assert_eq!(
        std::fs::read(&output).unwrap(),
        b"preserved pre-existing scratch bytes"
    );
    let mut archive = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
    archive
        .start_file(
            "install_profile.json",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
    archive
        .write_all(&vec![b' '; metadata::MAX_INSTALLER_DOCUMENT_BYTES + 1])
        .unwrap();
    archive.finish().unwrap();
    assert!(matches!(
        metadata::read_installer_documents(&jar),
        Err(metadata::ForgeMetadataError::ResponseTooLarge { .. })
    ));
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    std::fs::remove_dir_all(root).unwrap();
}

fn forge(mc: &str) -> ForgePlan {
    let (pin, hash, profile, version, _) = documents(mc);
    ForgePlan::from_documents(
        &InstallProfileDocument::from_json(profile).unwrap(),
        &VersionProfileDocument::from_json(version).unwrap(),
        &MinecraftVersionId::new(mc).unwrap(),
        &ForgeVersionId::new(pin).unwrap(),
        &ForgeMavenEndpoints::official(),
        Sha1Digest::parse(hash).unwrap(),
    )
    .unwrap()
}

fn game(mc: &str) -> GameInstallPlan {
    let document = VersionDocument::from_json(documents(mc).4).unwrap();
    let base = crate::minecraft::plan::plan_version_document(&document, platform()).unwrap();
    compose_forge_game_plan(base, forge(mc)).unwrap()
}

// Synthetic changes to a parsed official profile; checked-in upstream fixture
// bytes and their provenance are unchanged. Exercise normalization AND compose.
fn installer_destination_case(kind: &str) {
    let mc = "1.20.4";
    let (pin, digest, profile_json, version_json, mojang_json) = documents(mc);
    let mojang = crate::minecraft::plan::plan_version_document(
        &VersionDocument::from_json(mojang_json).unwrap(),
        platform(),
    )
    .unwrap();
    let baseline = game(mc);
    let overlap = mojang
        .libraries()
        .iter()
        .find(|l| l.coordinate().as_maven_string() == "com.google.code.gson:gson:2.10.1")
        .unwrap();
    let mut profile = InstallProfileDocument::from_json(profile_json).unwrap();
    profile
        .libraries
        .retain(|l| l.name != overlap.coordinate().as_maven_string());
    let overlap_sha1 = overlap.artifact().sha1().as_hex();
    let (coordinate, sha1, retained) = match kind {
        "same-url-different" => (
            "com.google.code.gson:gson:2.10.1",
            overlap_sha1.as_str(),
            false,
        ),
        "conflict" => (
            "com.google.code.gson:gson:2.10.1",
            "0000000000000000000000000000000000000000",
            false,
        ),
        "zip-different-hash" => (
            "com.google.code.gson:gson:2.10.1@zip",
            "0000000000000000000000000000000000000000",
            true,
        ),
        "zip-same-hash" => (
            "com.google.code.gson:gson:2.10.1@zip",
            overlap_sha1.as_str(),
            true,
        ),
        "different-version" => (
            "com.google.code.gson:gson:9.9.9",
            overlap_sha1.as_str(),
            true,
        ),
        "shared-basename" => ("unrelated.group:gson:2.10.1", overlap_sha1.as_str(), true),
        _ => unreachable!(),
    };
    let path = crate::forge::plan::ForgeCoordinate::parse(coordinate)
        .unwrap()
        .repository_path();
    let url = format!("https://maven.minecraftforge.net/{path}");
    assert_ne!(url, overlap.artifact().url());
    profile.libraries.push(metadata::ProfileLibrary {
        name: coordinate.into(),
        path: path.clone(),
        url,
        sha1: sha1.into(),
        size: overlap.artifact().size_bytes(),
    });
    let forge = ForgePlan::from_documents(
        &profile,
        &VersionProfileDocument::from_json(version_json).unwrap(),
        &MinecraftVersionId::new(mc).unwrap(),
        &ForgeVersionId::new(pin).unwrap(),
        &ForgeMavenEndpoints::official(),
        Sha1Digest::parse(digest).unwrap(),
    )
    .unwrap();
    let composed = compose_forge_game_plan(mojang.clone(), forge);
    if kind == "conflict" {
        assert!(matches!(
            composed,
            Err(crate::game::plan::CompositionError::LibraryConflict { .. })
        ));
        return;
    }
    let composed = composed.unwrap();
    assert_eq!(composed.minecraft(), &mojang);
    assert_eq!(
        composed.libraries(),
        baseline.libraries(),
        "classpath order and provenance unchanged"
    );
    assert_eq!(
        composed.forge_loader().unwrap().processors(),
        baseline.forge_loader().unwrap().processors(),
        "processor inputs unchanged"
    );
    let installer = composed.forge_loader().unwrap().installer_libraries();
    assert_eq!(
        installer.iter().filter(|l| l.path() == path).count(),
        usize::from(retained)
    );
    assert_eq!(
        installer.len(),
        baseline.forge_loader().unwrap().installer_libraries().len() + usize::from(retained)
    );
    assert_eq!(
        composed
            .libraries()
            .iter()
            .filter(|l| l.repository_path() == overlap.path())
            .count(),
        1
    );
    if kind.starts_with("zip") {
        assert_ne!(path, overlap.path());
        assert_eq!(installer.last().unwrap().coordinate().extension(), "zip");
    }
}

#[test]
fn installer_same_destination_same_sha1_different_urls_deduplicates() {
    installer_destination_case("same-url-different");
}
#[test]
fn installer_same_destination_different_sha1_conflicts() {
    installer_destination_case("conflict");
}
#[test]
fn installer_different_extensions_different_sha1_retains_both() {
    installer_destination_case("zip-different-hash");
}
#[test]
fn installer_different_extensions_same_sha1_retains_both() {
    installer_destination_case("zip-same-hash");
}
#[test]
fn installer_different_versions_retains_both() {
    installer_destination_case("different-version");
}
#[test]
fn installer_shared_basename_retains_both() {
    installer_destination_case("shared-basename");
}

/// F3R-B2: the official installer profile's tool libraries overlap the
/// composed client set (1.20.4 ships the same verified gson 2.10.1 as
/// Mojang's document; 1.20.1 overlaps on jopt-simple 5.0.4). Before the
/// composition fix both sides were planned, materializing one destination
/// twice and making the installed manifest unsealable. Composition must
/// leave exactly one verified entry per managed destination, keep the
/// merged order and provenance, preserve processor inputs, and keep every
/// genuinely distinct library.
#[test]
fn installer_tool_overlaps_compose_to_one_destination_per_artifact() {
    for (mc, overlap) in [
        ("1.20.4", Some("com.google.code.gson:gson:2.10.1")),
        ("1.20.1", Some("net.sf.jopt-simple:jopt-simple:5.0.4")),
        // The modern installer's tool set no longer overlaps Mojang's:
        // composition must retain everything.
        ("1.21.11", None),
    ] {
        let forge_plan = forge(mc);
        let processor_steps = forge_plan.processors().len();
        let launch_libraries = forge_plan.libraries().len();
        let installer_entries: Vec<String> = forge_plan
            .installer_libraries()
            .iter()
            .map(|library| library.coordinate().as_maven_string())
            .collect();
        let document = VersionDocument::from_json(documents(mc).4).unwrap();
        let base = crate::minecraft::plan::plan_version_document(&document, platform()).unwrap();
        let mojang_count = base.libraries().len();
        let composed = compose_forge_game_plan(base, forge_plan).unwrap();
        let forge_half = composed.forge_loader().unwrap();

        let merged_overlap: Vec<_> = overlap
            .map(|overlap| {
                // The overlapping installer-tool entry is dropped...
                assert!(
                    !forge_half
                        .installer_libraries()
                        .iter()
                        .any(|library| library.coordinate().as_maven_string() == overlap),
                    "{mc}: the overlapping installer entry must not be planned twice"
                );
                // ...and the composed set carries the Mojang-published copy once.
                let entries: Vec<_> = composed
                    .libraries()
                    .iter()
                    .filter(|library| library.coordinate_string() == overlap)
                    .collect();
                assert_eq!(entries.len(), 1, "{mc}");
                assert_eq!(
                    entries[0].provenance(),
                    crate::game::plan::LibraryProvenance::Mojang,
                    "{mc}"
                );
                entries
            })
            .unwrap_or_default();

        // One destination per artifact across BOTH materialized sets —
        // the original defect planned the same relative path twice.
        let mut destinations: Vec<String> = composed
            .libraries()
            .iter()
            .map(|library| library.repository_path())
            .collect();
        destinations.extend(
            forge_half
                .installer_libraries()
                .iter()
                .map(|library| library.path().to_owned()),
        );
        let mut unique = destinations.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            destinations.len(),
            "{mc}: duplicate destinations"
        );

        // The merge itself follows the shared boundary's established rules
        // (exact duplicates collapse): Mojang document order first, then
        // Forge launch libraries, with only genuine additions beyond the
        // Mojang set. The installer filter never touches this list.
        assert!(
            mojang_count <= composed.libraries().len()
                && composed.libraries().len() <= mojang_count + launch_libraries,
            "{mc}"
        );
        assert_eq!(
            composed.libraries()[0].provenance(),
            crate::game::plan::LibraryProvenance::Mojang,
            "{mc}"
        );
        // Every non-overlapping installer entry survives, in order.
        let retained: Vec<String> = forge_half
            .installer_libraries()
            .iter()
            .map(|library| library.coordinate().as_maven_string())
            .collect();
        assert_eq!(
            retained,
            installer_entries
                .iter()
                .filter(|entry| Some(entry.as_str()) != overlap)
                .cloned()
                .collect::<Vec<_>>(),
            "{mc}: only the overlapping entry may disappear"
        );

        // Processor inputs survive: the steps are untouched, and the only
        // destination composition can remove is the overlap's — which the
        // composed Mojang entry still plans. (Tool jars like installertools
        // are extracted from the installer archive by the processor runner,
        // not planned here.)
        assert_eq!(forge_half.processors().len(), processor_steps, "{mc}");
        let planned: std::collections::BTreeSet<String> = composed
            .libraries()
            .iter()
            .map(|library| format!("libraries/{}", library.repository_path()))
            .chain(
                forge_half
                    .installer_libraries()
                    .iter()
                    .map(|library| format!("libraries/{}", library.path())),
            )
            .collect();
        let overlap_destination = merged_overlap
            .first()
            .map(|entry| format!("libraries/{}", entry.repository_path()));
        for step in forge_half.processors() {
            for coordinate in std::iter::once(step.jar()).chain(step.classpath()) {
                let destination = format!("libraries/{}", coordinate.repository_path());
                if Some(destination.as_str()) == overlap_destination.as_deref() {
                    assert!(
                        planned.contains(&destination),
                        "{mc}: processor input {destination} lost its planned destination"
                    );
                }
            }
        }
    }
}

#[test]
fn official_mapping_generations_preserve_order_paths_and_digest_evidence() {
    for (mc, count, main, target) in [
        (
            "1.20.1",
            6,
            "cpw.mods.bootstraplauncher.BootstrapLauncher",
            "forgeclient",
        ),
        (
            "1.20.4",
            5,
            "net.minecraftforge.bootstrap.ForgeBootstrap",
            "forge_client",
        ),
    ] {
        let plan = game(mc);
        let forge = plan.forge_loader().unwrap();
        assert_eq!(plan.main_class(), main);
        assert_eq!(plan.java().component(), "java-runtime-gamma");
        assert_eq!(plan.java().major_version(), 17);
        assert!(!plan.java().raised_by_loader());
        assert_eq!(&forge.game_arguments()[..2], &["--launchTarget", target]);
        assert_eq!(forge.processors().len(), count);
        assert_eq!(&forge.processors()[0].args()[..2], &["--task", "MCP_DATA"]);
        assert_eq!(
            &forge.processors()[1].args()[..2],
            &["--task", "DOWNLOAD_MOJMAPS"]
        );
        assert_eq!(
            &forge.processors()[2].args()[..2],
            &["--task", "MERGE_MAPPING"]
        );
        assert_eq!(
            forge.processors().last().unwrap().jar().artifact(),
            "binarypatcher"
        );
        assert!(
            matches!(&forge.processors()[0].arguments()[3], ForgeProcessorArgument::Artifact(c) if c.extension() == "zip")
        );
        assert!(matches!(
            forge.data()["MAPPINGS"],
            ForgeDataValue::Artifact { .. }
        ));
        // Every expectation the profile publishes survives; PATCHED_SHA is
        // enforced even on 1.20.1's output-map-less final step.
        let patched = forge
            .processors()
            .last()
            .unwrap()
            .outputs()
            .iter()
            .find(|r| r.token() == "PATCHED")
            .unwrap();
        let ForgeDataValue::Literal(digest) = &forge.data()["PATCHED_SHA"] else {
            panic!()
        };
        assert_eq!(patched.sha1().as_hex(), *digest);
        if mc == "1.20.1" {
            assert_eq!(forge.processors()[3].jar().artifact(), "jarsplitter");
            assert_eq!(forge.processors()[3].outputs().len(), 2);
            assert_eq!(
                forge.processors()[1].outputs()[0].sha1().as_hex(),
                "6c48521eed01fe2e8ecdadbd5ae348415f3c47da"
            );
            assert!(
                forge
                    .jvm_arguments()
                    .iter()
                    .any(|a| a.starts_with("-DignoreList=") && a.ends_with(",client.jar"))
            );
            assert!(forge.jvm_arguments().iter().any(|a| a == "-p"));
            // Tool-only Guava 25.1 never replaces/joins Mojang's game Guava.
            assert!(
                !plan
                    .libraries()
                    .iter()
                    .any(|l| l.coordinate_string().contains("25.1-jre"))
            );
        } else {
            assert_eq!(
                forge
                    .processors()
                    .iter()
                    .map(|s| s.outputs().len())
                    .collect::<Vec<_>>(),
                [1, 1, 1, 1, 1]
            );
        }
        assert_eq!(plan, game(mc));
    }
}

#[test]
fn discovery_recommendations_never_advertise_unreviewed_builds() {
    let xml = "<metadata><versions><version>1.20.1-47.4.26</version><version>1.20.1-47.4.10</version><version>1.20.4-49.2.0</version><version>1.19.2-43.5.0</version></versions></metadata>";
    let listing = ForgeVersionList::from_documents(
        xml,
        r#"{"promos":{"1.20.1-latest":"47.4.26","1.20.1-recommended":"47.4.10"}}"#,
    )
    .unwrap();
    let mc = MinecraftVersionId::new("1.20.1").unwrap();
    assert_eq!(listing.for_minecraft(&mc).len(), 2);
    let choices = listing.installable_for_minecraft(&mc);
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].version().as_str(), "1.20.1-47.4.10");
    assert!(choices[0].stable());
    let no_recommendation =
        listing.installable_for_minecraft(&MinecraftVersionId::new("1.20.4").unwrap());
    assert!(!no_recommendation[0].stable()); // automatic must refuse
    assert!(
        listing
            .installable_for_minecraft(&MinecraftVersionId::new("1.19.2").unwrap())
            .is_empty()
    );
    assert!(
        ForgeVersionId::new("1.19.2-43.5.0")
            .unwrap()
            .require_reviewed()
            .is_err()
    );
    assert!(
        ForgeVersionId::parse_for_minecraft(
            "1.20.1-47.4.10",
            &MinecraftVersionId::new("1.20.4").unwrap()
        )
        .is_err()
    );
}

#[test]
fn official_profiles_reject_unknown_mandatory_fields_sides_and_expressions() {
    for mc in ["1.20.1", "1.20.4"] {
        let (pin, digest, profile, version, _) = documents(mc);
        for (pointer, field) in [
            ("", "unknownRequired"),
            ("/processors/0", "unknownArgumentMode"),
            ("/libraries/0/downloads/artifact", "unknownIntegrity"),
            ("/data/MAPPINGS", "unknownSide"),
        ] {
            let mut raw: serde_json::Value = serde_json::from_str(profile).unwrap();
            raw.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), true.into());
            assert!(
                InstallProfileDocument::from_json(&raw.to_string()).is_err(),
                "{pointer}/{field}"
            );
        }
        let mut raw: serde_json::Value = serde_json::from_str(profile).unwrap();
        raw["processors"][0]["sides"] = serde_json::json!(["other"]);
        assert!(InstallProfileDocument::from_json(&raw.to_string()).is_err());
        let mut doc = InstallProfileDocument::from_json(profile).unwrap();
        doc.processors
            .iter_mut()
            .find(|p| p.applies_to_client())
            .unwrap()
            .args
            .push("{ROOT}/escape".into());
        assert!(
            ForgePlan::from_documents(
                &doc,
                &VersionProfileDocument::from_json(version).unwrap(),
                &MinecraftVersionId::new(mc).unwrap(),
                &ForgeVersionId::new(pin).unwrap(),
                &ForgeMavenEndpoints::official(),
                Sha1Digest::parse(digest).unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn unsanitized_mappings_without_mojang_digest_fail_closed() {
    let mut doc = VersionDocument::from_json(documents("1.20.1").4).unwrap();
    doc.downloads.client_mappings = None;
    let base = crate::minecraft::plan::plan_version_document(&doc, platform()).unwrap();
    assert!(compose_forge_game_plan(base, forge("1.20.1")).is_err());
}

#[test]
fn both_official_generations_assemble_structured_redacting_launch_specs() {
    use crate::install::state::{
        InstalledFile, InstalledFileRole, InstalledGameManifest, NativesRecord,
    };
    use crate::launch::resolve::{LaunchOptions, LaunchPlan, resolve_launch_spec};
    for mc in ["1.20.1", "1.20.4"] {
        let plan = game(mc);
        let root = std::env::temp_dir().join(format!("aurora-f1c-launch-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let id = crate::instances::InstanceId::new("fixture").unwrap();
        let paths = managed.instance_paths(&id);
        let mut files = Vec::new();
        let mut add = |path: String, role| {
            let target = paths.game().join(&path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, b"fixture").unwrap();
            files.push(InstalledFile::new(
                role,
                path,
                ArtifactTrust::verified_sha1(
                    &Sha1Digest::parse("0000000000000000000000000000000000000000").unwrap(),
                ),
                7,
            ));
        };
        for lib in plan.libraries() {
            add(
                format!("libraries/{}", lib.repository_path()),
                InstalledFileRole::Library,
            );
        }
        add(
            format!("versions/{mc}/client.jar"),
            InstalledFileRole::Client,
        );
        add(
            format!(
                "assets/indexes/{}.json",
                plan.minecraft().asset_index().id()
            ),
            InstalledFileRole::AssetIndex,
        );
        if let Some(logging) = plan.minecraft().logging() {
            add(
                format!("versions/{mc}/{}", logging.file_name()),
                InstalledFileRole::LoggingConfig,
            );
        }
        std::fs::create_dir_all(paths.game().join("natives")).unwrap();
        std::fs::write(paths.game().join("natives/fixture.dll"), b"fixture").unwrap();
        let java = managed
            .runtimes_dir()
            .join("java-runtime-gamma/fixture/bin/javaw.exe");
        std::fs::create_dir_all(java.parent().unwrap()).unwrap();
        std::fs::write(&java, b"fixture").unwrap();
        let manifest = InstalledGameManifest::for_platform(
            mc,
            plan.platform(),
            "fixture",
            1,
            files,
            NativesRecord::new("natives".into()),
        );
        let session = crate::auth::MinecraftSession::new(
            "986dec87b7ec47ff89ff033fdb95c4b5",
            crate::auth::MinecraftProfile::new("986dec87b7ec47ff89ff033fdb95c4b5", "FixtureUser"),
            crate::auth::credentials::SecretString::new("F1C-SECRET-FIXTURE"),
            std::time::Duration::from_secs(3600),
        );
        let spec = resolve_launch_spec(
            &managed,
            &id,
            &LaunchPlan::from_game_plan(&plan, platform()),
            &manifest,
            &java,
            &session,
            &FeatureProfile::none(),
            &LaunchOptions::default(),
        )
        .unwrap();
        assert_eq!(spec.main_class(), plan.main_class());
        assert_eq!(spec.java_executable(), java);
        assert_eq!(spec.working_directory(), paths.root());
        assert_eq!(
            spec.classpath().last().unwrap(),
            &paths.game().join(format!("versions/{mc}/client.jar"))
        );
        assert!(
            spec.game_arguments()
                .iter()
                .any(|a| a.expose() == "FixtureUser")
        );
        assert!(
            spec.game_arguments()
                .iter()
                .any(|a| a.is_sensitive() && a.expose() == "F1C-SECRET-FIXTURE")
        );
        assert!(!format!("{spec:?}").contains("F1C-SECRET-FIXTURE"));
        assert!(
            spec.jvm_arguments()
                .iter()
                .chain(spec.game_arguments())
                .all(|a| !a.expose().contains("${"))
        );
        if mc == "1.20.1" {
            let args: Vec<_> = spec.jvm_arguments().iter().map(|a| a.expose()).collect();
            let index = args.iter().position(|a| *a == "-p").unwrap();
            assert!(args[index + 1].contains("bootstraplauncher-1.1.2.jar;"));
            assert!(
                args[index + 1]
                    .contains(&paths.game().join("libraries").to_string_lossy().to_string())
            );
        }
        // Exact disposable root created above; never owner data.
        assert!(root.parent() == Some(std::env::temp_dir().as_path()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
