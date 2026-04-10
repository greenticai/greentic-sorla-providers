#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sorla_provider_core::{ProviderCapability, ProviderMetadata, ProviderStatus};
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
            oci_reference: None,
            display: DisplayMetadata {
                title: metadata.display_name.clone(),
                summary: format!("{} provider manifest for SoRLa", metadata.display_name),
            },
        }
    }

    pub fn package_dir_name(&self) -> String {
        self.provider_id
            .chars()
            .map(|ch| match ch {
                'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
                _ => '-',
            })
            .collect()
    }

    pub fn artifact_file_name(&self) -> String {
        format!("{}.gtpack.json", self.package_dir_name())
    }
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
        ContractCompatibility, ProviderCapability, ProviderMetadata, ProviderStatus,
        SORLA_PROVIDER_CONTRACT_VERSION,
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
}
