//! Local deterministic compatibility facts. Ownership is independent of
//! capabilities; this module never downloads, executes, adopts or mutates.
use crate::fabric::versions::satisfies as fabric_satisfies;
use crate::neoforge::versions::satisfies as neoforge_satisfies;

/// Evaluates one relation requirement in the semantics of the instance's
/// loader family: Fabric predicates for Fabric instances, Maven ranges for
/// NeoForge instances. Malformed requirements are errors, never matches.
pub(crate) fn satisfies_for_family(
    version: &str,
    requirement: &str,
    loader_family: &str,
) -> Result<bool, &'static str> {
    if loader_family == "neoForge" {
        neoforge_satisfies(version, requirement)
    } else {
        fabric_satisfies(version, requirement)
    }
}
use crate::instance_mods::{ModEntry, ModInventory, ModMetadata, ModOwnership, ModRelation};
use serde::Serialize;

pub(crate) fn add_artifact(
    inventory: &mut ModInventory,
    file_name: String,
    metadata: ModMetadata,
    sha256: String,
    warnings: Vec<crate::instance_mods::ModWarning>,
) {
    inventory.entries.push(ModEntry {
        entry_id: format!("planned:{file_name}"),
        display_name: metadata.name.clone().unwrap_or_else(|| metadata.id.clone()),
        file_name,
        enabled: true,
        file_type: crate::instance_mods::ModFileType::EnabledJar,
        size_bytes: None,
        modified_unix_millis: None,
        ownership: ModOwnership::ProviderManaged,
        sha256: Some(sha256),
        provenance: None,
        metadata: Some(metadata),
        warnings,
        can_toggle: false,
        can_remove: false,
        removal_blocked_reason: None,
        action_blocked_reason: None,
    });
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityIssue {
    pub code: String,
    pub mod_id: String,
    pub file_name: String,
    pub requirement: Option<String>,
    pub installed_version: Option<String>,
    pub ownership: ModOwnership,
    pub message: String,
}

pub(crate) fn version_for<'a>(metadata: &'a ModMetadata, id: &str) -> Option<&'a str> {
    if metadata.id == id {
        metadata.version.as_deref()
    } else {
        metadata
            .nested_mod_versions
            .get(id)
            .and_then(|v| v.as_deref())
    }
}

pub(crate) fn usable(entry: &ModEntry) -> bool {
    entry.enabled
        && entry.ownership != ModOwnership::Unknown
        && entry.sha256.is_some()
        && entry.metadata.as_ref().is_some_and(|m| {
            m.version.as_deref().is_some_and(|v| !v.is_empty())
                && m.nested_mod_versions
                    .values()
                    .all(|v| v.as_deref().is_some_and(|s| !s.is_empty()))
        })
        && !entry.warnings.iter().any(|w| {
            w.code.starts_with("nested_")
                || w.code.starts_with("fabric_metadata_")
                || w.code.starts_with("neoforge_metadata_")
                || w.code == "mixin_metadata_invalid"
        })
}

pub(crate) fn relation_satisfied(
    inventory: &ModInventory,
    relation: &ModRelation,
    loader_family: &str,
) -> Result<bool, &'static str> {
    let active: Vec<_> = inventory
        .entries
        .iter()
        .filter(|e| usable(e))
        .filter_map(|e| e.metadata.as_ref())
        .filter(|m| m.environment.as_deref() != Some("server"))
        .collect();
    // A top-level mod is mandatory; nested jars are selectable candidates.
    // Every incoming hard constraint must accept the same selected version.
    let roots: Vec<_> = active.iter().filter(|m| m.id == relation.mod_id).collect();
    let versions: Vec<_> = if roots.is_empty() {
        active
            .iter()
            .filter_map(|m| version_for(m, &relation.mod_id))
            .collect()
    } else {
        roots.iter().filter_map(|m| m.version.as_deref()).collect()
    };
    let mut found = false;
    for version in versions {
        let mut matches = satisfies_for_family(version, &relation.requirement, loader_family)?;
        for metadata in &active {
            for constraint in metadata
                .depends
                .iter()
                .filter(|r| r.mod_id == relation.mod_id)
            {
                matches &= satisfies_for_family(version, &constraint.requirement, loader_family)?;
            }
            for constraint in metadata
                .breaks
                .iter()
                .filter(|r| r.mod_id == relation.mod_id)
            {
                matches &= !satisfies_for_family(version, &constraint.requirement, loader_family)?;
            }
        }
        found |= matches;
    }
    Ok(found)
}

fn issue(
    entry: &ModEntry,
    code: &str,
    requirement: Option<String>,
    installed_version: Option<String>,
    message: String,
) -> CompatibilityIssue {
    CompatibilityIssue {
        code: code.into(),
        mod_id: entry
            .metadata
            .as_ref()
            .map_or_else(|| entry.display_name.clone(), |m| m.id.clone()),
        file_name: entry.file_name.clone(),
        requirement,
        installed_version,
        ownership: entry.ownership,
        message,
    }
}

/// `java_major` comes only from the resolved game plan. None skips Java checks
/// during content-only validation; Play always supplies the resolved major.
pub fn validate(
    inventory: &ModInventory,
    minecraft: &str,
    loader: &str,
    java_major: Option<u32>,
    loader_family: &str,
) -> Vec<CompatibilityIssue> {
    let mut issues = Vec::new();
    for entry in inventory.entries.iter().filter(|e| e.enabled) {
        if !usable(entry) {
            issues.push(issue(entry,"mod_metadata_unusable",None,None,format!("{} has unreadable, ambiguous, or modified mod/Mixin metadata. Inspect {} before Play.",entry.display_name,entry.file_name)));
            continue;
        }
        let metadata = entry.metadata.as_ref().expect("usable");
        if metadata.environment.as_deref() == Some("server") {
            continue;
        }
        for relation in &metadata.depends {
            let builtin = match relation.mod_id.as_str() {
                "minecraft" => Some(minecraft.to_owned()),
                "fabricloader" if loader_family != "neoForge" => Some(loader.to_owned()),
                "neoforge" | "forge" if loader_family == "neoForge" => Some(loader.to_owned()),
                "java" => {
                    if java_major.is_none() {
                        continue;
                    }
                    java_major.map(|v| v.to_string())
                }
                _ => None,
            };
            let result = if let Some(version) = &builtin {
                satisfies_for_family(version, &relation.requirement, loader_family)
            } else {
                relation_satisfied(inventory, relation, loader_family)
            };
            if result != Ok(true) {
                let providers: Vec<_> = inventory
                    .entries
                    .iter()
                    .filter_map(|candidate| {
                        candidate
                            .metadata
                            .as_ref()
                            .and_then(|m| version_for(m, &relation.mod_id))
                            .map(|v| {
                                format!(
                                    "{} ({:?}, {}{})",
                                    v,
                                    candidate.ownership,
                                    candidate.file_name,
                                    if candidate.enabled { "" } else { ", disabled" }
                                )
                            })
                    })
                    .collect();
                let installed =
                    builtin.or_else(|| (!providers.is_empty()).then(|| providers.join("; ")));
                let reason = if result.is_err() {
                    "The predicate could not be evaluated safely."
                } else {
                    "No active installed capability satisfies it."
                };
                issues.push(issue(
                    entry,
                    "mod_dependency_unsatisfied",
                    Some(relation.requirement.clone()),
                    installed.clone(),
                    format!(
                        "{} {} requires {} {}. Installed: {}. {reason}",
                        metadata.id,
                        metadata.version.as_deref().unwrap_or("unknown"),
                        relation.mod_id,
                        relation.requirement,
                        installed.as_deref().unwrap_or("absent")
                    ),
                ));
            }
        }
        // Fabric conflicts are advisory; breaks are fatal. Recommendations and
        // suggestions never become hard requirements.
        for relation in &metadata.breaks {
            let builtin = match relation.mod_id.as_str() {
                "minecraft" => Some(minecraft.to_owned()),
                "fabricloader" if loader_family != "neoForge" => Some(loader.to_owned()),
                "neoforge" | "forge" if loader_family == "neoForge" => Some(loader.to_owned()),
                "java" => {
                    if java_major.is_none() {
                        continue;
                    }
                    java_major.map(|v| v.to_string())
                }
                _ => None,
            };
            if let Some(version) = builtin {
                if satisfies_for_family(&version, &relation.requirement, loader_family) != Ok(false)
                {
                    issues.push(issue(
                        entry,
                        "mod_declared_break",
                        Some(relation.requirement.clone()),
                        Some(version.clone()),
                        format!(
                            "{} {} declares breaks {} {}. This instance uses {} {}.",
                            metadata.id,
                            metadata.version.as_deref().unwrap_or("unknown"),
                            relation.mod_id,
                            relation.requirement,
                            relation.mod_id,
                            version
                        ),
                    ));
                }
                continue;
            }

            for other in inventory.entries.iter().filter(|e| usable(e)) {
                if let Some(version) = other
                    .metadata
                    .as_ref()
                    .and_then(|m| version_for(m, &relation.mod_id))
                {
                    match satisfies_for_family(version, &relation.requirement, loader_family) {
                        Ok(false) => {}
                        result => issues.push(issue(
                            entry,
                            "mod_declared_break",
                            Some(relation.requirement.clone()),
                            Some(version.into()),
                            format!(
                                "{} {} declares breaks {} {}. Installed: {} in {} ({:?}).{}",
                                metadata.id,
                                metadata.version.as_deref().unwrap_or("unknown"),
                                relation.mod_id,
                                relation.requirement,
                                version,
                                other.file_name,
                                other.ownership,
                                if result.is_err() {
                                    " Predicate is unsupported; inspect metadata."
                                } else {
                                    ""
                                }
                            ),
                        )),
                    }
                }
            }
        }
        for other in inventory
            .entries
            .iter()
            .filter(|e| usable(e) && e.entry_id != entry.entry_id)
        {
            let other_meta = other.metadata.as_ref().unwrap();
            if metadata.id == other_meta.id {
                issues.push(issue(entry,"mod_duplicate_root",None,other_meta.version.clone(),format!("Top-level identity {} is provided by both {} and {}. No file will be adopted or replaced.",metadata.id,entry.file_name,other.file_name)));
            }
        }
        if let Some(java) = java_major {
            for requirement in &metadata.mixin_java_requirements {
                if requirement.java_major > java {
                    issues.push(issue(entry,"mod_java_incompatible",Some(format!("JAVA_{}",requirement.java_major)),Some(java.to_string()),format!("{} requires Java {} for declared Mixin config {}. This instance uses Aurora managed Java {}.",requirement.mod_id,requirement.java_major,requirement.config,java)));
                }
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance_mods::inspect_fabric_metadata;
    use std::io::{Cursor, Write};
    fn entry(document: serde_json::Value, ownership: ModOwnership, enabled: bool) -> ModEntry {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(document.to_string().as_bytes()).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let root =
            std::env::temp_dir().join(format!("aurora-compat-test-{}.jar", uuid::Uuid::new_v4()));
        std::fs::write(&root, &bytes).unwrap();
        let (metadata, warnings) = inspect_fabric_metadata(&root, bytes.len() as u64);
        std::fs::remove_file(&root).unwrap();
        let mut inventory = ModInventory {
            instance_id: "test".into(),
            entries: Vec::new(),
            missing_managed: Vec::new(),
        };
        add_artifact(
            &mut inventory,
            format!("{}.jar", document["id"].as_str().unwrap()),
            metadata.unwrap(),
            "a".repeat(64),
            warnings,
        );
        let mut entry = inventory.entries.pop().unwrap();
        entry.ownership = ownership;
        entry.enabled = enabled;
        entry
    }
    fn dependency(ownership: ModOwnership, enabled: bool, version: &str) -> ModEntry {
        entry(
            serde_json::json!({"id":"dependency","version":version}),
            ownership,
            enabled,
        )
    }
    fn parent(key: &str, requirement: &str) -> ModEntry {
        let mut doc = serde_json::json!({"id":"parent","version":"1.0"});
        doc[key] = serde_json::json!({"dependency":requirement});
        entry(doc, ModOwnership::UserManaged, true)
    }
    fn inventory(entries: Vec<ModEntry>) -> ModInventory {
        ModInventory {
            instance_id: "test".into(),
            entries,
            missing_managed: Vec::new(),
        }
    }
    #[test]
    fn absent_and_disabled_never_satisfy() {
        for entries in [
            vec![parent("depends", ">=2")],
            vec![
                parent("depends", ">=2"),
                dependency(ModOwnership::UserManaged, false, "2"),
            ],
        ] {
            assert!(
                !validate(&inventory(entries), "1.21.11", "0.19.5", Some(21), "fabric").is_empty()
            );
        }
    }
    #[test]
    fn ownership_is_independent_of_satisfaction() {
        for ownership in [
            ModOwnership::UserManaged,
            ModOwnership::ProviderManaged,
            ModOwnership::LauncherBootstrap,
        ] {
            let entries = vec![
                parent("depends", ">=2 <3"),
                dependency(ownership, true, "2.4+build"),
            ];
            assert!(
                validate(&inventory(entries), "1.21.11", "0.19.5", Some(21), "fabric").is_empty()
            );
        }
    }
    #[test]
    fn incompatible_and_ambiguous_artifacts_fail_precisely() {
        for ownership in [
            ModOwnership::UserManaged,
            ModOwnership::ProviderManaged,
            ModOwnership::Unknown,
        ] {
            let issues = validate(
                &inventory(vec![
                    parent("depends", ">=2"),
                    dependency(ownership, true, "1"),
                ]),
                "1.21.11",
                "0.19.5",
                Some(21),
                "fabric",
            );
            assert!(issues.iter().any(|i| i.code == "mod_dependency_unsatisfied"
                && i.message.contains(">=2")
                && i.message.contains("dependency.jar")));
        }
    }
    #[test]
    fn advisory_relations_do_not_block_but_breaks_do() {
        for key in ["recommends", "suggests", "conflicts"] {
            assert!(
                validate(
                    &inventory(vec![parent(key, "*")]),
                    "1.21.11",
                    "0.19.5",
                    Some(21),
                    "fabric"
                )
                .is_empty()
            );
            assert!(
                validate(
                    &inventory(vec![
                        parent(key, "*"),
                        dependency(ModOwnership::UserManaged, true, "2")
                    ]),
                    "1.21.11",
                    "0.19.5",
                    Some(21),
                    "fabric"
                )
                .is_empty()
            );
        }
        assert_eq!(
            validate(
                &inventory(vec![
                    parent("breaks", "<=2"),
                    dependency(ModOwnership::UserManaged, true, "2")
                ]),
                "1.21.11",
                "0.19.5",
                Some(21),
                "fabric"
            )[0]
            .code,
            "mod_declared_break"
        );
        assert!(
            validate(
                &inventory(vec![
                    parent("breaks", "<=2"),
                    dependency(ModOwnership::UserManaged, true, "3")
                ]),
                "1.21.11",
                "0.19.5",
                Some(21),
                "fabric"
            )
            .is_empty()
        );
    }
    #[test]
    fn one_selected_version_must_satisfy_every_parent() {
        let mut second = parent("depends", "<2");
        second.entry_id = "second".into();
        second.metadata.as_mut().unwrap().id = "second".into();
        let mut bundle = dependency(ModOwnership::UserManaged, true, "1");
        bundle.metadata.as_mut().unwrap().id = "bundle".into();
        bundle
            .metadata
            .as_mut()
            .unwrap()
            .nested_mod_ids
            .push("dependency".into());
        bundle
            .metadata
            .as_mut()
            .unwrap()
            .nested_mod_versions
            .insert("dependency".into(), Some("1".into()));
        assert!(
            !validate(
                &inventory(vec![
                    parent("depends", ">=2"),
                    second,
                    dependency(ModOwnership::UserManaged, true, "2"),
                    bundle
                ]),
                "1.21.11",
                "0.19.5",
                Some(21),
                "fabric"
            )
            .is_empty()
        );
    }
    #[test]
    fn nested_capability_satisfies_without_adoption() {
        let mut bundle = dependency(ModOwnership::UserManaged, true, "2");
        let m = bundle.metadata.as_mut().unwrap();
        m.id = "bundle".into();
        m.nested_mod_ids.push("dependency".into());
        m.nested_mod_versions
            .insert("dependency".into(), Some("2".into()));
        assert!(
            validate(
                &inventory(vec![parent("depends", ">=2"), bundle]),
                "1.21.11",
                "0.19.5",
                Some(21),
                "fabric"
            )
            .is_empty()
        );
    }
    #[test]
    fn builtin_breaks_use_the_resolved_instance_versions() {
        for (id, predicate) in [
            ("minecraft", ">=1.21.11"),
            ("fabricloader", "<0.20"),
            ("java", "<25"),
        ] {
            let value = entry(
                serde_json::json!({"id":"builtin-break","version":"1","breaks":{id:predicate}}),
                ModOwnership::UserManaged,
                true,
            );
            assert_eq!(
                validate(
                    &inventory(vec![value]),
                    "1.21.11",
                    "0.19.5",
                    Some(21),
                    "fabric"
                )[0]
                .code,
                "mod_declared_break"
            );
        }
        let value = entry(
            serde_json::json!({"id":"builtin-break","version":"1","breaks":{"java":"<21","minecraft":"<1.21"}}),
            ModOwnership::UserManaged,
            true,
        );
        assert!(
            validate(
                &inventory(vec![value]),
                "1.21.11",
                "0.19.5",
                Some(21),
                "fabric"
            )
            .is_empty()
        );
    }
    #[test]
    fn duplicate_roots_and_java_are_deterministic() {
        let mut duplicate = dependency(ModOwnership::UserManaged, true, "3");
        duplicate.entry_id = "duplicate".into();
        duplicate.file_name = "other.jar".into();
        assert!(
            validate(
                &inventory(vec![
                    dependency(ModOwnership::UserManaged, true, "2"),
                    duplicate
                ]),
                "1.21.11",
                "0.19.5",
                Some(21),
                "fabric"
            )
            .iter()
            .any(|i| i.code == "mod_duplicate_root")
        );
        let java = entry(
            serde_json::json!({"id":"java-mod","version":"1","depends":{"java":">=25"}}),
            ModOwnership::UserManaged,
            true,
        );
        assert!(
            validate(
                &inventory(vec![java.clone()]),
                "1.21.11",
                "0.19.5",
                Some(21),
                "fabric"
            )
            .iter()
            .any(|i| i.code == "mod_dependency_unsatisfied")
        );
        assert!(
            validate(
                &inventory(vec![java]),
                "1.21.11",
                "0.19.5",
                Some(25),
                "fabric"
            )
            .is_empty()
        );
    }
}
