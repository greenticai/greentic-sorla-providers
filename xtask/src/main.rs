#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use semver::Version;
use serde::{Deserialize, Serialize};
use sorla_provider_core::{
    EntityLinkProvider, EntityLinkRequest, EntityRecord, EntityRef, EntityStoreProvider,
    EvidenceProvider, EvidenceQuery, EvidenceQueryFilter, ExternalMappingProvider,
    OntologyGraphProvider, OntologyScope, PathQuery, RelationshipDirection, RelationshipInstance,
    RelationshipQuery, RelationshipRef,
};

const PROVIDER_MAP_PATH: &str = "ci/provider-dependencies.json";

#[derive(Debug, Deserialize)]
struct ProviderMap {
    providers: BTreeMap<String, ProviderSpec>,
    shared_crates: BTreeMap<String, SharedCrateSpec>,
    rebuild_all_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderSpec {
    package: String,
    manifest_path: String,
    paths: Vec<String>,
    depends_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SharedCrateSpec {
    paths: Vec<String>,
    affects: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ProviderMatrix {
    include: Vec<ProviderMatrixEntry>,
}

#[derive(Debug, Serialize)]
struct ProviderMatrixEntry {
    provider: String,
    package: String,
    manifest_path: String,
    version: String,
    depends_on: Vec<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let command = args.first().cloned().ok_or_else(|| {
        "usage: cargo xtask <provider-version|provider-matrix|ontology-smoke> ...".to_string()
    })?;
    args.remove(0);

    match command.as_str() {
        "provider-version" => provider_version(args),
        "provider-matrix" => provider_matrix(args),
        "ontology-smoke" => ontology_smoke(),
        _ => Err(format!("unknown xtask command: {command}")),
    }
}

fn ontology_smoke() -> Result<(), String> {
    let graph_paths = smoke_foundationdb_graph()?;
    let sharepoint_links = smoke_sharepoint_linking()?;
    let evidence_items = smoke_rag_evidence()?;
    let catalog_entries = smoke_pack_catalog_metadata()?;
    let security_artifacts_checked = smoke_security_checks()?;

    let summary = serde_json::json!({
        "graph_paths": graph_paths,
        "sharepoint_links": sharepoint_links,
        "evidence_items": evidence_items,
        "catalog_entries": catalog_entries,
        "security_artifacts_checked": security_artifacts_checked,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).map_err(|err| format!("json error: {err}"))?
    );
    Ok(())
}

fn smoke_foundationdb_graph() -> Result<usize, String> {
    let provider = provider_foundationdb::FoundationDbProvider::for_tests();
    let customer = smoke_entity("Customer", "customer-001");
    let contract = smoke_entity("Contract", "contract-001");
    let asset = smoke_entity("Asset", "asset-001");
    let evidence = smoke_entity("EvidenceDocument", "doc-001");

    for entity in [&customer, &contract, &asset, &evidence] {
        provider
            .upsert_entity(EntityRecord {
                entity: entity.clone(),
                label: Some(format!("{} {}", entity.entity_type, entity.entity_id)),
                metadata_json: None,
            })
            .map_err(|err| format!("entity upsert failed: {err}"))?;
    }

    for relationship in [
        smoke_relationship("has_contract", customer.clone(), contract.clone()),
        smoke_relationship("governs", contract.clone(), asset),
        smoke_relationship("has_evidence", contract.clone(), evidence.clone()),
    ] {
        provider
            .upsert_relationship(relationship)
            .map_err(|err| format!("relationship upsert failed: {err}"))?;
    }

    let direct = provider
        .query_relationships(RelationshipQuery {
            root_entities: vec![customer.clone()],
            relationship_type: Some("has_contract".into()),
            direction: RelationshipDirection::Outgoing,
            max_depth: Some(1),
            limit: 10,
        })
        .map_err(|err| format!("relationship query failed: {err}"))?;
    if direct.len() != 1 {
        return Err(format!(
            "expected one direct relationship, got {}",
            direct.len()
        ));
    }

    let paths = provider
        .find_paths(PathQuery {
            from: customer,
            to: evidence,
            relationship_types: vec![],
            max_depth: 3,
            limit: 10,
        })
        .map_err(|err| format!("path query failed: {err}"))?;
    if paths.len() != 1 || paths[0].steps.len() != 2 {
        return Err("expected one two-step Customer to EvidenceDocument path".into());
    }

    Ok(paths.len())
}

fn smoke_sharepoint_linking() -> Result<usize, String> {
    let provider = provider_sharepoint_mock::SharePointMockProvider::for_tests();
    provider
        .validate_mapping(
            r#"{
                "schema": "greentic.sorla.external-mapping.v1",
                "provider_id": "greentic.sorla.provider.sharepoint-mock",
                "mappings": [
                    {
                        "source_type": "document",
                        "target_concept": "EvidenceDocument",
                        "id_field": "document_id",
                        "entity_fields": {
                            "title": "title",
                            "source_url": "source_url"
                        }
                    }
                ]
            }"#,
        )
        .map_err(|err| format!("mapping validation failed: {err}"))?;

    let links = provider
        .link_entities(EntityLinkRequest {
            source_ref: Some("sharepoint://tenant/ka-fd-demo/document/doc-001".into()),
            evidence_id: None,
            content_json: None,
            candidate_types: vec!["EvidenceDocument".into()],
            ontology_scope: None,
        })
        .map_err(|err| format!("entity linking failed: {err}"))?;
    if links.len() != 1 || links[0].entity.entity_id != "doc-001" {
        return Err("expected one deterministic SharePoint entity link".into());
    }

    Ok(links.len())
}

fn smoke_rag_evidence() -> Result<usize, String> {
    let provider = provider_rag_mock::RagMockProvider::for_tests();
    let items = provider
        .query_evidence(EvidenceQuery {
            query: "risk evidence for building-kafd-01".into(),
            filter: EvidenceQueryFilter {
                ontology_scope: Some(OntologyScope {
                    root_entities: vec![smoke_entity("Building", "building-kafd-01")],
                    include_related: vec![],
                    max_depth: Some(1),
                    include_evidence_links: true,
                }),
                source_types: vec![],
                document_types: vec![],
                metadata_json: None,
                time_range: None,
                sensitivity_max: Some("internal".into()),
            },
            limit: 4,
        })
        .map_err(|err| format!("evidence query failed: {err}"))?;
    if items.is_empty()
        || items
            .iter()
            .any(|item| item.linked_entities.is_empty() || item.permissions_context_json.is_none())
    {
        return Err(
            "expected ontology-scoped evidence with linked entities and permissions context".into(),
        );
    }
    Ok(items.len())
}

fn smoke_pack_catalog_metadata() -> Result<usize, String> {
    let manifests = vec![
        provider_foundationdb::pack_manifest(),
        provider_sharepoint_mock::pack_manifest(),
        provider_rag_mock::pack_manifest(),
    ];
    if manifests
        .iter()
        .any(|manifest| manifest.ontology_capabilities.is_none())
    {
        return Err("expected all smoke providers to advertise ontology metadata".into());
    }

    let catalog = sorla_provider_catalog::ProviderCatalog::from_manifests(&manifests);
    if catalog.entries.iter().any(|entry| {
        entry.ontology.as_ref().is_none_or(|ontology| {
            ontology.supported_ontology_schema.is_empty()
                || !ontology.supported_ontology_schema_range.contains("1.0.0")
        })
    }) {
        return Err("expected catalog ontology compatibility metadata".into());
    }

    Ok(catalog.entries.len())
}

fn smoke_security_checks() -> Result<usize, String> {
    let mut files = Vec::new();
    collect_json_files(&workspace_path("examples/generated-packs"), &mut files)?;
    collect_json_files(&workspace_path("examples/generated-catalog"), &mut files)?;

    let forbidden = [
        "password",
        "secret",
        "api_key",
        "apikey",
        "credential",
        "private_key",
    ];
    for path in &files {
        let content = fs::read_to_string(path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let lowered = content.to_ascii_lowercase();
        for pattern in forbidden {
            if lowered.contains(pattern) {
                return Err(format!(
                    "generated artifact {} contains credential-like text: {pattern}",
                    path.display()
                ));
            }
        }
    }

    Ok(files.len())
}

fn collect_json_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        fs::read_dir(root).map_err(|err| format!("failed to read {}: {err}", root.display()))?
    {
        let entry = entry.map_err(|err| format!("failed to read directory entry: {err}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(())
}

fn smoke_entity(entity_type: &str, entity_id: &str) -> EntityRef {
    EntityRef {
        entity_type: entity_type.into(),
        entity_id: entity_id.into(),
        namespace: Some("ontology-smoke".into()),
        version: None,
    }
}

fn smoke_relationship(
    relationship_type: &str,
    from: EntityRef,
    to: EntityRef,
) -> RelationshipInstance {
    RelationshipInstance {
        relationship: RelationshipRef {
            relationship_type: relationship_type.into(),
            from,
            to,
        },
        metadata_json: None,
        provenance: Some("ontology-smoke".into()),
    }
}

fn provider_version(args: Vec<String>) -> Result<(), String> {
    let (subcommand, rest) = args
        .split_first()
        .ok_or_else(|| "usage: cargo xtask provider-version <list|set|bump> ...".to_string())?;
    match subcommand.as_str() {
        "list" => {
            let map = read_provider_map()?;
            for (provider, spec) in map.providers {
                println!(
                    "{provider}\t{}",
                    read_manifest_version(&spec.manifest_path)?
                );
            }
            Ok(())
        }
        "set" => {
            if rest.len() != 2 {
                return Err("usage: cargo xtask provider-version set <provider> <version>".into());
            }
            let provider = &rest[0];
            let version =
                Version::parse(&rest[1]).map_err(|err| format!("invalid version: {err}"))?;
            set_provider_version(provider, version)
        }
        "bump" => {
            if rest.len() != 2 {
                return Err(
                    "usage: cargo xtask provider-version bump <provider> <patch|minor|major>"
                        .into(),
                );
            }
            let provider = &rest[0];
            let bump = &rest[1];
            let map = read_provider_map()?;
            let spec = find_provider(&map, provider)?;
            let mut version = Version::parse(&read_manifest_version(&spec.manifest_path)?)
                .map_err(|err| format!("invalid existing version: {err}"))?;
            match bump.as_str() {
                "patch" => version.patch += 1,
                "minor" => {
                    version.minor += 1;
                    version.patch = 0;
                }
                "major" => {
                    version.major += 1;
                    version.minor = 0;
                    version.patch = 0;
                }
                "none" => {}
                _ => return Err("version bump must be one of none, patch, minor, major".into()),
            }
            set_provider_version(provider, version)
        }
        _ => Err(format!("unknown provider-version subcommand: {subcommand}")),
    }
}

fn provider_matrix(args: Vec<String>) -> Result<(), String> {
    let mut provider = "all".to_string();
    let mut changed_files: Option<PathBuf> = None;
    let mut version_bump = "none".to_string();

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                provider = iter
                    .next()
                    .ok_or_else(|| "--provider requires a value".to_string())?;
            }
            "--changed-files" => {
                changed_files =
                    Some(PathBuf::from(iter.next().ok_or_else(|| {
                        "--changed-files requires a value".to_string()
                    })?));
            }
            "--version-bump" => {
                version_bump = iter
                    .next()
                    .ok_or_else(|| "--version-bump requires a value".to_string())?;
            }
            _ => return Err(format!("unknown provider-matrix argument: {arg}")),
        }
    }

    if !matches!(version_bump.as_str(), "none" | "patch" | "minor" | "major") {
        return Err("version bump must be one of none, patch, minor, major".into());
    }

    let map = read_provider_map()?;
    let selected = if provider == "all" {
        match changed_files {
            Some(path) => detect_changed_providers(&map, &read_lines(&path)?),
            None => map.providers.keys().cloned().collect(),
        }
    } else {
        let key = canonical_provider_key(&map, &provider)?;
        BTreeSet::from([key])
    };

    let mut include = Vec::new();
    for provider in selected {
        let spec = map
            .providers
            .get(&provider)
            .ok_or_else(|| format!("unknown provider selected: {provider}"))?;
        include.push(ProviderMatrixEntry {
            provider,
            package: spec.package.clone(),
            manifest_path: spec.manifest_path.clone(),
            version: read_manifest_version(&spec.manifest_path)?,
            depends_on: spec.depends_on.clone(),
        });
    }

    let matrix = ProviderMatrix { include };
    println!(
        "{}",
        serde_json::to_string(&matrix).map_err(|err| format!("json error: {err}"))?
    );
    Ok(())
}

fn set_provider_version(provider: &str, version: Version) -> Result<(), String> {
    let map = read_provider_map()?;
    let key = canonical_provider_key(&map, provider)?;
    let spec = map
        .providers
        .get(&key)
        .ok_or_else(|| format!("unknown provider: {provider}"))?;
    replace_manifest_version(&spec.manifest_path, &version.to_string())?;
    refresh_cargo_lock()?;
    println!("{key}\t{version}");
    Ok(())
}

fn read_provider_map() -> Result<ProviderMap, String> {
    let path = workspace_path(PROVIDER_MAP_PATH);
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {PROVIDER_MAP_PATH}: {err}"))?;
    serde_json::from_str(&raw).map_err(|err| format!("invalid {PROVIDER_MAP_PATH}: {err}"))
}

fn workspace_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path)
    }
}

fn refresh_cargo_lock() -> Result<(), String> {
    let status = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .stdout(Stdio::null())
        .status()
        .map_err(|err| format!("failed to refresh Cargo.lock: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to refresh Cargo.lock: {status}"))
    }
}

fn find_provider<'a>(map: &'a ProviderMap, provider: &str) -> Result<&'a ProviderSpec, String> {
    let key = canonical_provider_key(map, provider)?;
    map.providers
        .get(&key)
        .ok_or_else(|| format!("unknown provider: {provider}"))
}

fn canonical_provider_key(map: &ProviderMap, provider: &str) -> Result<String, String> {
    if map.providers.contains_key(provider) {
        return Ok(provider.to_string());
    }
    let without_prefix = provider.strip_prefix("provider-").unwrap_or(provider);
    if map.providers.contains_key(without_prefix) {
        return Ok(without_prefix.to_string());
    }
    if let Some((key, _)) = map
        .providers
        .iter()
        .find(|(_, spec)| spec.package == provider || spec.manifest_path.contains(provider))
    {
        return Ok(key.clone());
    }
    Err(format!("unknown provider: {provider}"))
}

fn read_manifest_version(path: impl AsRef<Path>) -> Result<String, String> {
    let path = workspace_path(path);
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    parse_manifest_version(&text)
}

fn parse_manifest_version(text: &str) -> Result<String, String> {
    let mut in_package = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package || !trimmed.starts_with("version") {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim() == "version" {
            return Ok(value.trim().trim_matches('"').to_string());
        }
    }
    Err("provider package version must be an explicit [package] version".into())
}

fn replace_manifest_version(path: impl AsRef<Path>, version: &str) -> Result<(), String> {
    let path = workspace_path(path);
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let mut in_package = false;
    let mut replaced = false;
    let lines = text
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_package = trimmed == "[package]";
            }
            if in_package
                && trimmed.starts_with("version")
                && let Some((key, _)) = trimmed.split_once('=')
                && key.trim() == "version"
            {
                replaced = true;
                return format!("version = \"{version}\"");
            }
            line.to_string()
        })
        .collect::<Vec<_>>();
    if !replaced {
        return Err(format!(
            "{} does not contain an explicit [package] version",
            path.display()
        ));
    }
    fs::write(&path, format!("{}\n", lines.join("\n")))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn detect_changed_providers(map: &ProviderMap, changed_files: &[String]) -> BTreeSet<String> {
    if changed_files.is_empty()
        || changed_files
            .iter()
            .any(|path| matches_any(path, &map.rebuild_all_paths))
    {
        return map.providers.keys().cloned().collect();
    }

    let mut selected = BTreeSet::new();
    for path in changed_files {
        for (provider, spec) in &map.providers {
            if matches_any(path, &spec.paths) {
                selected.insert(provider.clone());
            }
        }
        for shared in map.shared_crates.values() {
            if matches_any(path, &shared.paths) {
                selected.extend(shared.affects.iter().cloned());
            }
        }
    }

    selected
}

fn matches_any(path: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| matches_pattern(path, pattern))
}

fn matches_pattern(path: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        path == prefix || path.starts_with(&format!("{prefix}/"))
    } else {
        path == pattern
    }
}

fn read_lines(path: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read changed file list {}: {err}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn test_map() -> ProviderMap {
        serde_json::from_str(
            r#"{
              "providers": {
                "hubspot": {
                  "package": "provider-hubspot",
                  "manifest_path": "providers/provider-hubspot/Cargo.toml",
                  "paths": ["providers/provider-hubspot/**"],
                  "depends_on": ["sorla-provider-common"]
                },
                "salesforce": {
                  "package": "provider-salesforce",
                  "manifest_path": "providers/provider-salesforce/Cargo.toml",
                  "paths": ["providers/provider-salesforce/**"],
                  "depends_on": ["sorla-provider-common"]
                }
              },
              "shared_crates": {
                "sorla-provider-common": {
                  "paths": ["crates/sorla-provider-common/**"],
                  "affects": ["hubspot", "salesforce"]
                }
              },
              "rebuild_all_paths": ["Cargo.lock", ".github/workflows/**"]
            }"#,
        )
        .expect("map should parse")
    }

    #[test]
    fn provider_version_parsing_reads_explicit_package_version() {
        let parsed = parse_manifest_version(
            r#"[package]
name = "provider-hubspot"
version = "0.1.3"

[dependencies]
"#,
        )
        .expect("version should parse");

        assert_eq!(parsed, "0.1.3");
    }

    #[test]
    fn provider_version_parsing_rejects_workspace_version() {
        let err = parse_manifest_version(
            r#"[package]
name = "provider-hubspot"
version.workspace = true
"#,
        )
        .unwrap_err();

        assert!(err.contains("explicit [package] version"));
    }

    #[test]
    fn provider_version_parsing_ignores_dependency_versions() {
        let err = parse_manifest_version(
            r#"[package]
name = "provider-hubspot"

[dependencies]
serde = "1"
"#,
        )
        .unwrap_err();

        assert!(err.contains("explicit [package] version"));
    }

    #[test]
    fn provider_version_bumping_updates_only_manifest_version() {
        let dir = unique_temp_dir("provider-version-test");
        let manifest = dir.join("Cargo.toml");
        std::fs::write(
            &manifest,
            "[package]\nname = \"provider-hubspot\"\nversion = \"0.1.3\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap();

        replace_manifest_version(&manifest, "0.1.4").unwrap();

        let updated = std::fs::read_to_string(&manifest).unwrap();
        assert!(updated.contains("version = \"0.1.4\""));
        assert!(updated.contains("serde = \"1\""));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn provider_version_bumping_reports_missing_package_version() {
        let dir = unique_temp_dir("provider-version-missing-test");
        let manifest = dir.join("Cargo.toml");
        std::fs::write(&manifest, "[package]\nname = \"provider-hubspot\"\n").unwrap();

        let err = replace_manifest_version(&manifest, "0.1.4").unwrap_err();

        assert!(err.contains("does not contain an explicit [package] version"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn read_lines_trims_empty_entries() {
        let dir = unique_temp_dir("provider-lines-test");
        let changed = dir.join("changed.txt");
        std::fs::write(
            &changed,
            "\n providers/provider-hubspot/src/lib.rs \n\nCargo.lock\n",
        )
        .unwrap();

        let lines = read_lines(&changed).unwrap();

        assert_eq!(
            lines,
            vec!["providers/provider-hubspot/src/lib.rs", "Cargo.lock"]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn provider_change_selects_only_that_provider() {
        let selected = detect_changed_providers(
            &test_map(),
            &["providers/provider-hubspot/src/lib.rs".into()],
        );

        assert_eq!(selected, BTreeSet::from(["hubspot".into()]));
    }

    #[test]
    fn shared_crate_change_selects_all_dependents() {
        let selected = detect_changed_providers(
            &test_map(),
            &["crates/sorla-provider-common/src/lib.rs".into()],
        );

        assert_eq!(
            selected,
            BTreeSet::from(["hubspot".into(), "salesforce".into()])
        );
    }

    #[test]
    fn root_lockfile_change_selects_all_providers() {
        let selected = detect_changed_providers(&test_map(), &["Cargo.lock".into()]);

        assert_eq!(
            selected,
            BTreeSet::from(["hubspot".into(), "salesforce".into()])
        );
    }

    #[test]
    fn empty_change_list_selects_all_providers() {
        let selected = detect_changed_providers(&test_map(), &[]);

        assert_eq!(
            selected,
            BTreeSet::from(["hubspot".into(), "salesforce".into()])
        );
    }

    #[test]
    fn unrelated_change_selects_no_providers() {
        let selected = detect_changed_providers(&test_map(), &["docs/readme.md".into()]);

        assert!(selected.is_empty());
    }

    #[test]
    fn path_matching_supports_exact_and_recursive_patterns() {
        assert!(matches_pattern("Cargo.lock", "Cargo.lock"));
        assert!(matches_pattern(
            ".github/workflows/ci.yml",
            ".github/workflows/**"
        ));
        assert!(matches_pattern(".github/workflows", ".github/workflows/**"));
        assert!(!matches_pattern(
            ".github/actions/ci.yml",
            ".github/workflows/**"
        ));
    }

    #[test]
    fn canonical_provider_accepts_package_name() {
        assert_eq!(
            canonical_provider_key(&test_map(), "provider-salesforce").unwrap(),
            "salesforce"
        );
    }

    #[test]
    fn canonical_provider_rejects_unknown_provider() {
        let err = canonical_provider_key(&test_map(), "unknown").unwrap_err();

        assert!(err.contains("unknown provider"));
    }

    #[test]
    fn workflow_matrix_generation_can_select_one_or_all() {
        let map = test_map();
        let one = BTreeSet::from([canonical_provider_key(&map, "hubspot").unwrap()]);
        let all = map.providers.keys().cloned().collect::<BTreeSet<_>>();

        assert_eq!(one, BTreeSet::from(["hubspot".into()]));
        assert_eq!(all, BTreeSet::from(["hubspot".into(), "salesforce".into()]));
    }

    #[test]
    fn provider_version_command_validates_arguments() {
        assert!(provider_version(vec![]).unwrap_err().contains("usage"));
        assert!(
            provider_version(vec!["set".into(), "foundationdb".into()])
                .unwrap_err()
                .contains("usage")
        );
        assert!(
            provider_version(vec![
                "set".into(),
                "foundationdb".into(),
                "not-semver".into()
            ])
            .unwrap_err()
            .contains("invalid version")
        );
        assert!(
            provider_version(vec!["bump".into(), "foundationdb".into()])
                .unwrap_err()
                .contains("usage")
        );
        assert!(
            provider_version(vec!["nope".into()])
                .unwrap_err()
                .contains("unknown provider-version")
        );
    }

    #[test]
    fn provider_matrix_command_validates_arguments() {
        assert!(
            provider_matrix(vec!["--provider".into()])
                .unwrap_err()
                .contains("requires a value")
        );
        assert!(
            provider_matrix(vec!["--changed-files".into()])
                .unwrap_err()
                .contains("requires a value")
        );
        assert!(
            provider_matrix(vec!["--version-bump".into(), "weird".into()])
                .unwrap_err()
                .contains("version bump")
        );
        assert!(
            provider_matrix(vec!["--bogus".into()])
                .unwrap_err()
                .contains("unknown provider-matrix")
        );
    }

    #[test]
    fn provider_matrix_command_accepts_single_provider() {
        provider_matrix(vec!["--provider".into(), "sharepoint-mock".into()]).unwrap();
        provider_matrix(vec![
            "--provider".into(),
            "provider-sharepoint-mock".into(),
            "--version-bump".into(),
            "patch".into(),
        ])
        .unwrap();
    }

    #[test]
    fn provider_matrix_command_uses_changed_file_list() {
        let dir = unique_temp_dir("provider-matrix-test");
        let changed = dir.join("changed.txt");
        std::fs::write(&changed, "providers/provider-rag-mock/src/lib.rs\n").unwrap();

        provider_matrix(vec![
            "--provider".into(),
            "all".into(),
            "--changed-files".into(),
            changed.to_string_lossy().into_owned(),
        ])
        .unwrap();

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ontology_smoke_command_runs_locally() {
        ontology_smoke().unwrap();
    }
}
