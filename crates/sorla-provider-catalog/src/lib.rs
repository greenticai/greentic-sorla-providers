#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sorla_provider_core::{ProviderCapability, ProviderStatus};
use sorla_provider_pack::ProviderPackManifest;

/// Catalog entry generated from a provider pack manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCatalogEntry {
    pub provider_id: String,
    pub provider_version: String,
    pub provider_kind: String,
    pub capabilities: Vec<ProviderCapability>,
    pub tags: Vec<String>,
    pub is_mock: bool,
    pub status: ProviderStatus,
    pub supported_provider_contract_version: String,
    pub supported_sorla_ir: String,
    pub supported_sorla_ir_range: String,
    pub config_schema_path: String,
    pub artifact_uri: Option<String>,
    pub oci_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ontology: Option<ProviderCatalogOntology>,
}

/// Ontology metadata projected from provider pack manifests for discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCatalogOntology {
    pub capabilities: Vec<ProviderCapability>,
    pub max_traversal_depth: Option<u8>,
    pub supports_generic_entity_refs: bool,
    pub supported_ontology_schema: String,
    pub supported_ontology_schema_range: String,
    pub supported_retrieval_binding_schema: Option<String>,
    pub supported_external_mapping_schema: Option<String>,
}

/// Deterministic catalog output for SoRLa wizard discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCatalog {
    pub catalog_format_version: String,
    pub generated_from: String,
    pub entries: Vec<ProviderCatalogEntry>,
}

/// Paths written for a generated catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCatalogLayout {
    pub output_dir: PathBuf,
    pub catalog_path: PathBuf,
}

fn tags_for_manifest(manifest: &ProviderPackManifest) -> Vec<String> {
    let mut tags = vec![manifest.provider_kind.clone()];
    if manifest.is_mock {
        tags.push("mock".into());
    } else {
        tags.push("real".into());
    }
    tags.sort();
    tags.dedup();
    tags
}

fn ontology_for_manifest(manifest: &ProviderPackManifest) -> Option<ProviderCatalogOntology> {
    let ontology = manifest.ontology_capabilities.as_ref()?;
    let mut capabilities = Vec::new();

    if ontology.supports_entity_read {
        capabilities.push(ProviderCapability::EntityRead);
    }
    if ontology.supports_entity_search {
        capabilities.push(ProviderCapability::EntitySearch);
    }
    if ontology.supports_relationship_query {
        capabilities.push(ProviderCapability::RelationshipQuery);
    }
    if ontology.supports_path_find {
        capabilities.push(ProviderCapability::PathFind);
    }
    if ontology.supports_entity_linking {
        capabilities.push(ProviderCapability::EntityLink);
    }
    if ontology.supports_ontology_scoped_evidence {
        capabilities.push(ProviderCapability::OntologyScopedEvidenceQuery);
    }
    if ontology.supports_policy_context {
        capabilities.push(ProviderCapability::PolicyContextResolve);
    }

    Some(ProviderCatalogOntology {
        capabilities,
        max_traversal_depth: ontology.max_traversal_depth,
        supports_generic_entity_refs: true,
        supported_ontology_schema: ontology.compatibility.supported_ontology_schema.clone(),
        supported_ontology_schema_range: ontology
            .compatibility
            .supported_ontology_schema_range
            .clone(),
        supported_retrieval_binding_schema: ontology
            .compatibility
            .supported_retrieval_binding_schema
            .clone(),
        supported_external_mapping_schema: ontology
            .compatibility
            .supported_external_mapping_schema
            .clone(),
    })
}

impl ProviderCatalog {
    pub fn from_manifests(manifests: &[ProviderPackManifest]) -> Self {
        let mut entries = manifests
            .iter()
            .map(|manifest| ProviderCatalogEntry {
                provider_id: manifest.provider_id.clone(),
                provider_version: manifest.provider_version.clone(),
                provider_kind: manifest.provider_kind.clone(),
                capabilities: manifest.capabilities.clone(),
                tags: tags_for_manifest(manifest),
                is_mock: manifest.is_mock,
                status: manifest.status,
                supported_provider_contract_version: manifest
                    .supported_provider_contract_version
                    .clone(),
                supported_sorla_ir: manifest.supported_sorla_ir.clone(),
                supported_sorla_ir_range: manifest.supported_sorla_ir_range.clone(),
                config_schema_path: manifest.config_schema.path.clone(),
                artifact_uri: manifest
                    .artifact_references
                    .first()
                    .map(|item| item.uri.clone()),
                oci_reference: manifest.oci_reference.clone(),
                ontology: ontology_for_manifest(manifest),
            })
            .collect::<Vec<_>>();

        entries.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        Self {
            catalog_format_version: "sorla-provider-catalog/v1".into(),
            generated_from: "generated-pack-manifests".into(),
            entries,
        }
    }
}

pub fn read_manifest(path: impl AsRef<Path>) -> Result<ProviderPackManifest, serde_json::Error> {
    let json = fs::read_to_string(path).map_err(serde_json::Error::io)?;
    serde_json::from_str(&json)
}

pub fn write_catalog(
    root: impl AsRef<Path>,
    catalog: &ProviderCatalog,
) -> Result<GeneratedCatalogLayout, serde_json::Error> {
    let root = root.as_ref();
    fs::create_dir_all(root).map_err(serde_json::Error::io)?;
    let catalog_path = root.join("provider-catalog.json");
    let json = serde_json::to_string_pretty(catalog)?;
    fs::write(&catalog_path, format!("{json}\n")).map_err(serde_json::Error::io)?;

    Ok(GeneratedCatalogLayout {
        output_dir: root.to_path_buf(),
        catalog_path,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ProviderCatalog, read_manifest, write_catalog};
    use sorla_provider_core::{
        ContractCompatibility, OntologyContractCompatibility, ProviderCapability, ProviderMetadata,
        ProviderOntologyCapabilities, ProviderStatus, SORLA_PROVIDER_CONTRACT_VERSION,
    };
    use sorla_provider_pack::{
        ArtifactReference, ConfigSchemaRef, ProviderPackManifest, RuntimeComponentRef,
        write_generated_pack,
    };

    fn manifest(provider_id: &str, provider_kind: &str, is_mock: bool) -> ProviderPackManifest {
        ProviderPackManifest::from_metadata(
            &ProviderMetadata {
                provider_id: provider_id.into(),
                display_name: provider_id.into(),
                provider_kind: provider_kind.into(),
                version: "0.1.0".into(),
                status: ProviderStatus::Experimental,
                is_mock,
                capabilities: vec![ProviderCapability::PackMetadataEmit],
                compatibility: ContractCompatibility::new(
                    SORLA_PROVIDER_CONTRACT_VERSION,
                    "0.1",
                    ">=0.1, <0.2",
                ),
                ontology_capabilities: None,
            },
            vec![ArtifactReference {
                kind: "gtpack-json".into(),
                uri: format!("./{provider_id}.gtpack.json"),
            }],
            vec![RuntimeComponentRef {
                component_id: format!("{provider_id}-runtime"),
                kind: "service".into(),
                entrypoint: provider_id.into(),
                artifact_uri: format!("oci://ghcr.io/greenticai/{provider_id}:v0.1.0"),
            }],
            ConfigSchemaRef {
                format: "json-schema".into(),
                path: "schemas/provider-config.schema.json".into(),
                schema_json: r#"{"type":"object","additionalProperties":false}"#.into(),
            },
        )
    }

    fn ontology_manifest(provider_id: &str) -> ProviderPackManifest {
        let mut manifest = manifest(provider_id, "evidence", true);
        manifest
            .capabilities
            .push(ProviderCapability::OntologyScopedEvidenceQuery);
        manifest.ontology_capabilities = Some(ProviderOntologyCapabilities {
            schema: "greentic.sorla.provider.ontology-capabilities.v1".into(),
            compatibility: OntologyContractCompatibility {
                supported_ontology_schema: "greentic.sorla.ontology.v1".into(),
                supported_ontology_schema_range: ">=1.0.0, <2.0.0".into(),
                supported_retrieval_binding_schema: Some(
                    "greentic.sorla.retrieval-bindings.v1".into(),
                ),
                supported_external_mapping_schema: None,
            },
            supports_entity_read: false,
            supports_entity_search: false,
            supports_relationship_query: false,
            supports_path_find: false,
            supports_entity_linking: false,
            supports_ontology_scoped_evidence: true,
            supported_concept_types: vec!["EvidenceDocument".into()],
            supported_relationship_types: vec![],
            max_traversal_depth: Some(1),
            supports_policy_context: false,
        });
        manifest
    }

    #[test]
    fn catalog_generation_is_sorted_and_stable() {
        let catalog = ProviderCatalog::from_manifests(&[
            manifest("provider-b", "evidence", true),
            manifest("provider-a", "event-store", false),
        ]);

        assert_eq!(catalog.entries[0].provider_id, "provider-a");
        assert_eq!(catalog.entries[0].tags, vec!["event-store", "real"]);
        assert_eq!(
            catalog.entries[1].artifact_uri.as_deref(),
            Some("./provider-b.gtpack.json")
        );
    }

    #[test]
    fn catalog_can_round_trip_from_generated_pack_manifest() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("sorla-catalog-test-{unique}"));
        let pack_root = root.join("packs");
        let catalog_root = root.join("catalog");

        let manifest = manifest("provider-c", "external-ref", true);
        let layout = write_generated_pack(&pack_root, &manifest).expect("pack write should work");
        let parsed = read_manifest(&layout.manifest_path).expect("manifest should parse");
        let catalog = ProviderCatalog::from_manifests(&[parsed]);
        let written = write_catalog(&catalog_root, &catalog).expect("catalog write should work");

        assert!(written.catalog_path.exists());
    }

    #[test]
    fn catalog_serializes_ontology_metadata_from_manifest() {
        let catalog = ProviderCatalog::from_manifests(&[ontology_manifest("provider-ontology")]);

        let entry = &catalog.entries[0];
        let ontology = entry.ontology.as_ref().expect("ontology metadata");
        assert_eq!(
            ontology.capabilities,
            vec![ProviderCapability::OntologyScopedEvidenceQuery]
        );
        assert_eq!(ontology.max_traversal_depth, Some(1));
        assert!(ontology.supports_generic_entity_refs);
        assert_eq!(
            ontology.supported_ontology_schema,
            "greentic.sorla.ontology.v1"
        );
        assert_eq!(
            ontology.supported_retrieval_binding_schema.as_deref(),
            Some("greentic.sorla.retrieval-bindings.v1")
        );
    }
}
