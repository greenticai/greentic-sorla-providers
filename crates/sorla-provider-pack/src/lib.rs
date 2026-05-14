#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sorla_provider_core::{
    ProviderCapability, ProviderMetadata, ProviderOntologyCapabilities, ProviderStatus,
};
use thiserror::Error;

/// Reference to a generated provider artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReference {
    pub kind: String,
    pub uri: String,
}

/// Runtime component required by a provider pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeComponentRef {
    pub component_id: String,
    pub kind: String,
    pub entrypoint: String,
    pub artifact_uri: String,
}

/// Configuration schema reference shipped with a generated provider pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSchemaRef {
    pub format: String,
    pub path: String,
    pub schema_json: String,
}

/// Human-facing metadata for discovery surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayMetadata {
    pub title: String,
    pub summary: String,
}

/// Canonical local pack manifest emitted from provider metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPackManifest {
    pub pack_format_version: String,
    pub provider_id: String,
    pub provider_version: String,
    pub provider_kind: String,
    pub capabilities: Vec<ProviderCapability>,
    pub is_mock: bool,
    pub status: ProviderStatus,
    pub supported_provider_contract_version: String,
    pub supported_sorla_ir: String,
    pub supported_sorla_ir_range: String,
    pub artifact_references: Vec<ArtifactReference>,
    pub runtime_components: Vec<RuntimeComponentRef>,
    pub config_schema: ConfigSchemaRef,
    pub oci_reference: Option<String>,
    pub display: DisplayMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ontology_capabilities: Option<ProviderOntologyCapabilities>,
}

/// Serialized local gtpack artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedPackArtifact {
    pub artifact_format: String,
    pub manifest: ProviderPackManifest,
}

/// Result of writing a local provider pack layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedPackLayout {
    pub pack_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub artifact_path: PathBuf,
    pub schema_path: PathBuf,
}

/// Errors returned while generating local provider pack artifacts.
#[derive(Debug, Error)]
pub enum PackGenerationError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl ProviderPackManifest {
    pub fn from_metadata(
        metadata: &ProviderMetadata,
        artifact_references: Vec<ArtifactReference>,
        runtime_components: Vec<RuntimeComponentRef>,
        config_schema: ConfigSchemaRef,
    ) -> Self {
        let oci_reference = runtime_components
            .first()
            .map(|component| component.artifact_uri.clone());
        Self {
            pack_format_version: "sorla-provider-pack/v1".into(),
            provider_id: metadata.provider_id.clone(),
            provider_version: metadata.version.clone(),
            provider_kind: metadata.provider_kind.clone(),
            capabilities: metadata.capabilities.clone(),
            is_mock: metadata.is_mock,
            status: metadata.status,
            supported_provider_contract_version: metadata
                .compatibility
                .supported_provider_contract_version
                .clone(),
            supported_sorla_ir: metadata.compatibility.supported_sorla_ir.clone(),
            supported_sorla_ir_range: metadata.compatibility.supported_sorla_ir_range.clone(),
            artifact_references,
            runtime_components,
            config_schema,
            oci_reference,
            display: DisplayMetadata {
                title: metadata.display_name.clone(),
                summary: format!("{} provider manifest for SoRLa", metadata.display_name),
            },
            ontology_capabilities: metadata.ontology_capabilities.clone(),
        }
    }

    pub fn package_dir_name(&self) -> String {
        provider_id_to_package_dir(&self.provider_id)
    }

    pub fn artifact_file_name(&self) -> String {
        format!("{}.gtpack.json", self.package_dir_name())
    }

    pub fn provider_slug(&self) -> String {
        provider_slug(&self.provider_id)
    }
}

pub fn provider_slug(provider_id: &str) -> String {
    provider_id
        .rsplit('.')
        .next()
        .unwrap_or(provider_id)
        .trim_start_matches("provider-")
        .to_ascii_lowercase()
}

pub fn provider_artifact_file_uri(provider_id: &str) -> String {
    format!("./{}.gtpack.json", provider_id_to_package_dir(provider_id))
}

pub fn provider_runtime_oci_reference(provider_id: &str, version: &str) -> String {
    format!(
        "oci://ghcr.io/greenticai/sorla-providers/{}:{}",
        provider_slug(provider_id),
        version
    )
}

pub fn provider_runtime_component(
    provider_id: &str,
    version: &str,
    component_id: impl Into<String>,
    entrypoint: impl Into<String>,
) -> RuntimeComponentRef {
    RuntimeComponentRef {
        component_id: component_id.into(),
        kind: "service".into(),
        entrypoint: entrypoint.into(),
        artifact_uri: provider_runtime_oci_reference(provider_id, version),
    }
}

fn provider_id_to_package_dir(provider_id: &str) -> String {
    provider_id
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect()
}

impl GeneratedPackArtifact {
    pub fn from_manifest(manifest: ProviderPackManifest) -> Self {
        Self {
            artifact_format: "sorla-provider-gtpack/v1".into(),
            manifest,
        }
    }
}

pub fn write_generated_pack(
    root: impl AsRef<Path>,
    manifest: &ProviderPackManifest,
) -> Result<GeneratedPackLayout, PackGenerationError> {
    let root = root.as_ref();
    let pack_dir = root.join(manifest.package_dir_name());
    let manifest_path = pack_dir.join("manifest.json");
    let artifact_path = pack_dir.join(manifest.artifact_file_name());
    let schema_path = pack_dir.join(&manifest.config_schema.path);

    fs::create_dir_all(&pack_dir)?;
    if let Some(parent) = schema_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let manifest_json = serde_json::to_string_pretty(manifest)?;
    fs::write(&manifest_path, format!("{manifest_json}\n"))?;

    let artifact = GeneratedPackArtifact::from_manifest(manifest.clone());
    let artifact_json = serde_json::to_string_pretty(&artifact)?;
    fs::write(&artifact_path, format!("{artifact_json}\n"))?;

    fs::write(
        &schema_path,
        format!("{}\n", manifest.config_schema.schema_json),
    )?;

    Ok(GeneratedPackLayout {
        pack_dir,
        manifest_path,
        artifact_path,
        schema_path,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        ArtifactReference, ConfigSchemaRef, ProviderPackManifest, RuntimeComponentRef,
        write_generated_pack,
    };
    use sorla_provider_core::{
        ContractCompatibility, OntologyContractCompatibility, ProviderCapability, ProviderMetadata,
        ProviderOntologyCapabilities, ProviderStatus, SORLA_PROVIDER_CONTRACT_VERSION,
    };

    fn sample_metadata() -> ProviderMetadata {
        ProviderMetadata {
            provider_id: "greentic.sorla.provider.sharepoint-mock".into(),
            display_name: "SharePoint Mock".into(),
            provider_kind: "external-ref".into(),
            version: "0.1.0".into(),
            status: ProviderStatus::Experimental,
            is_mock: true,
            capabilities: vec![
                ProviderCapability::ExternalReferenceResolve,
                ProviderCapability::PackMetadataEmit,
            ],
            compatibility: ContractCompatibility::new(
                SORLA_PROVIDER_CONTRACT_VERSION,
                "0.1",
                ">=0.1, <0.2",
            ),
            ontology_capabilities: None,
        }
    }

    fn sample_manifest() -> ProviderPackManifest {
        ProviderPackManifest::from_metadata(
            &sample_metadata(),
            vec![ArtifactReference {
                kind: "gtpack-json".into(),
                uri: "./greentic-sorla-provider-sharepoint-mock.gtpack.json".into(),
            }],
            vec![RuntimeComponentRef {
                component_id: "sharepoint-mock-runtime".into(),
                kind: "service".into(),
                entrypoint: "provider-sharepoint-mock".into(),
                artifact_uri: "oci://ghcr.io/greenticai/greentic-sorla-providers/provider-sharepoint-mock:v0.1.0".into(),
            }],
            ConfigSchemaRef {
                format: "json-schema".into(),
                path: "schemas/provider-config.schema.json".into(),
                schema_json: r#"{"type":"object","additionalProperties":false}"#.into(),
            },
        )
    }

    fn sample_ontology_capabilities() -> ProviderOntologyCapabilities {
        ProviderOntologyCapabilities {
            schema: "greentic.sorla.provider.ontology-capabilities.v1".into(),
            compatibility: OntologyContractCompatibility {
                supported_ontology_schema: "greentic.sorla.ontology.v1".into(),
                supported_ontology_schema_range: ">=1.0.0, <2.0.0".into(),
                supported_retrieval_binding_schema: Some(
                    "greentic.sorla.retrieval-bindings.v1".into(),
                ),
                supported_external_mapping_schema: None,
            },
            supports_entity_read: true,
            supports_entity_search: true,
            supports_relationship_query: false,
            supports_path_find: false,
            supports_entity_linking: false,
            supports_ontology_scoped_evidence: true,
            supported_concept_types: vec!["EvidenceDocument".into()],
            supported_relationship_types: vec![],
            max_traversal_depth: Some(1),
            supports_policy_context: false,
        }
    }

    #[test]
    fn pack_manifest_preserves_provider_identity() {
        let manifest = sample_manifest();

        assert_eq!(
            manifest.provider_id,
            "greentic.sorla.provider.sharepoint-mock"
        );
        assert_eq!(manifest.artifact_references.len(), 1);
        assert_eq!(manifest.runtime_components.len(), 1);
        assert!(manifest.is_mock);
    }

    #[test]
    fn pack_generation_writes_expected_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("sorla-pack-test-{unique}"));
        let manifest = sample_manifest();
        let layout = write_generated_pack(&root, &manifest).expect("pack generation should work");

        assert!(layout.manifest_path.exists());
        assert!(layout.artifact_path.exists());
        assert!(layout.schema_path.exists());

        let manifest_json =
            std::fs::read_to_string(&layout.manifest_path).expect("manifest should be readable");
        assert!(
            manifest_json.contains("\"provider_id\": \"greentic.sorla.provider.sharepoint-mock\"")
        );
    }

    #[test]
    fn pack_manifest_serializes_ontology_metadata_when_present() {
        let mut metadata = sample_metadata();
        metadata.ontology_capabilities = Some(sample_ontology_capabilities());
        let manifest = ProviderPackManifest::from_metadata(
            &metadata,
            vec![],
            vec![],
            ConfigSchemaRef {
                format: "json-schema".into(),
                path: "schemas/provider-config.schema.json".into(),
                schema_json: r#"{"type":"object"}"#.into(),
            },
        );

        let json = serde_json::to_string(&manifest).expect("manifest should serialize");

        assert!(json.contains("ontology_capabilities"));
        assert!(json.contains("ontology-capabilities.v1"));
        assert!(json.contains("supported_ontology_schema_range"));
    }
}
