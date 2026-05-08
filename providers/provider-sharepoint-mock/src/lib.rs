#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sorla_provider_catalog::ProviderCatalogEntry;
use sorla_provider_core::{
    ConfigValidator, ContractCompatibility, ExternalReferencePayload, ExternalReferenceProvider,
    ExternalReferenceRequest, HealthReport, HealthState, PackEmission, ProviderCapability,
    ProviderHealth, ProviderMetadata, ProviderMetadataSource, ProviderStatus,
    SORLA_PROVIDER_CONTRACT_VERSION,
};
use sorla_provider_pack::{
    ArtifactReference, ConfigSchemaRef, ProviderPackManifest, provider_artifact_file_uri,
    provider_runtime_component,
};

const PROVIDER_ID: &str = "greentic.sorla.provider.sharepoint-mock";
const PROVIDER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharePointMockConfig {
    pub seed: String,
    pub tenant_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtgSection {
    pub section_id: String,
    pub heading: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtgDocument {
    pub document_id: String,
    pub building_id: String,
    pub floor_id: Option<String>,
    pub title: String,
    pub version: String,
    pub sections: Vec<BtgSection>,
    pub source_url: String,
    pub last_updated: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfiRecord {
    pub rfi_id: String,
    pub building_id: String,
    pub floor_id: Option<String>,
    pub question: String,
    pub answer: String,
    pub status: String,
    pub date: String,
    pub source_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteVisitRecord {
    pub visit_id: String,
    pub building_id: String,
    pub floor_id: Option<String>,
    pub summary: String,
    pub findings: Vec<String>,
    pub date: String,
    pub source_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "kebab-case")]
pub enum SharePointMockRecord {
    Btg(BtgDocument),
    Rfi(RfiRecord),
    SiteVisit(SiteVisitRecord),
}

pub struct SharePointMockProvider {
    config: SharePointMockConfig,
}

impl SharePointMockProvider {
    pub fn new(config: SharePointMockConfig) -> Self {
        Self { config }
    }

    pub fn for_tests() -> Self {
        Self::new(SharePointMockConfig {
            seed: "greentic-sorla-sharepoint-mock".into(),
            tenant_id: "ka-fd-demo".into(),
        })
    }

    fn stable_hash(&self, request: &ExternalReferenceRequest) -> u64 {
        let mut hash = 1469598103934665603u64;
        for byte in self
            .config
            .seed
            .bytes()
            .chain(self.config.tenant_id.bytes())
            .chain(request.reference_type.bytes())
            .chain(request.reference_id.bytes())
            .chain(request.building_id.as_deref().unwrap_or_default().bytes())
            .chain(request.floor_id.as_deref().unwrap_or_default().bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(1099511628211);
        }
        hash
    }

    fn building_id(&self, request: &ExternalReferenceRequest) -> String {
        request
            .building_id
            .clone()
            .unwrap_or_else(|| format!("building-{}", self.stable_hash(request) % 1000))
    }

    fn floor_id(&self, request: &ExternalReferenceRequest) -> Option<String> {
        request.floor_id.clone()
    }

    fn stable_date(&self, request: &ExternalReferenceRequest) -> String {
        let hash = self.stable_hash(request);
        let month = (hash % 12) + 1;
        let day = ((hash / 13) % 28) + 1;
        format!("2026-{month:02}-{day:02}")
    }

    fn source_url(&self, family: &str, record_id: &str, building_id: &str) -> String {
        format!(
            "https://sharepoint.mock.greentic/{tenant}/{building}/{family}/{record_id}",
            tenant = self.config.tenant_id,
            building = building_id
        )
    }

    fn btg_record(&self, request: &ExternalReferenceRequest) -> SharePointMockRecord {
        let building_id = self.building_id(request);
        let floor_id = self.floor_id(request);
        let document_id = format!("btg-{:016x}", self.stable_hash(request));
        let source_url = self.source_url("btg", &document_id, &building_id);

        SharePointMockRecord::Btg(BtgDocument {
            document_id: document_id.clone(),
            building_id: building_id.clone(),
            floor_id: floor_id.clone(),
            title: format!("BTG {} for {}", request.reference_id, building_id),
            version: format!(
                "v{}.{}",
                (self.stable_hash(request) % 3) + 1,
                (self.stable_hash(request) % 10)
            ),
            sections: vec![
                BtgSection {
                    section_id: format!("{document_id}-sec-01"),
                    heading: "Means of egress".into(),
                    summary: format!(
                        "Floor {} stair enclosure review recorded for {}.",
                        floor_id.clone().unwrap_or_else(|| "all".into()),
                        building_id
                    ),
                },
                BtgSection {
                    section_id: format!("{document_id}-sec-02"),
                    heading: "Fire alarm".into(),
                    summary: format!(
                        "Alarm zoning and annunciator placement summary captured for {}.",
                        request.reference_id
                    ),
                },
            ],
            source_url,
            last_updated: self.stable_date(request),
        })
    }

    fn rfi_record(&self, request: &ExternalReferenceRequest) -> SharePointMockRecord {
        let building_id = self.building_id(request);
        let floor_id = self.floor_id(request);
        let rfi_id = format!("rfi-{:016x}", self.stable_hash(request));
        let source_url = self.source_url("rfi", &rfi_id, &building_id);

        SharePointMockRecord::Rfi(RfiRecord {
            rfi_id,
            building_id,
            floor_id: floor_id.clone(),
            question: format!(
                "Clarify smoke damper access for {} on {}.",
                request.reference_id,
                floor_id.unwrap_or_else(|| "shared plant spaces".into())
            ),
            answer:
                "Access route confirmed via service riser and reflected in coordination markups."
                    .into(),
            status: if self.stable_hash(request).is_multiple_of(2) {
                "closed".into()
            } else {
                "answered".into()
            },
            date: self.stable_date(request),
            source_url,
        })
    }

    fn site_visit_record(&self, request: &ExternalReferenceRequest) -> SharePointMockRecord {
        let building_id = self.building_id(request);
        let floor_id = self.floor_id(request);
        let visit_id = format!("visit-{:016x}", self.stable_hash(request));
        let source_url = self.source_url("site-visit", &visit_id, &building_id);

        SharePointMockRecord::SiteVisit(SiteVisitRecord {
            visit_id,
            building_id: building_id.clone(),
            floor_id: floor_id.clone(),
            summary: format!(
                "Site walk for {} recorded observations around {}.",
                building_id,
                floor_id.unwrap_or_else(|| "shared circulation".into())
            ),
            findings: vec![
                "Fire door closer adjustment required at service corridor.".into(),
                "Ceiling access panel labelled for alarm loop inspection.".into(),
                format!("Mock note seed anchored to {}.", request.reference_id),
            ],
            date: self.stable_date(request),
            source_url,
        })
    }

    pub fn generate_record(
        &self,
        request: &ExternalReferenceRequest,
    ) -> Result<SharePointMockRecord, sorla_provider_core::ProviderError> {
        match request.reference_type.as_str() {
            "btg" => Ok(self.btg_record(request)),
            "rfi" => Ok(self.rfi_record(request)),
            "site-visit" => Ok(self.site_visit_record(request)),
            _ => Err(sorla_provider_core::ProviderError::Unsupported(
                "sharepoint-mock reference_type",
            )),
        }
    }
}

impl ProviderMetadataSource for SharePointMockProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            provider_id: PROVIDER_ID.into(),
            display_name: "SharePoint Mock".into(),
            provider_kind: "external-ref".into(),
            version: PROVIDER_VERSION.into(),
            status: ProviderStatus::Experimental,
            is_mock: true,
            capabilities: vec![
                ProviderCapability::ExternalReferenceResolve,
                ProviderCapability::HealthCheck,
                ProviderCapability::ConfigValidate,
                ProviderCapability::PackMetadataEmit,
            ],
            compatibility: ContractCompatibility::new(
                SORLA_PROVIDER_CONTRACT_VERSION,
                "0.1",
                ">=0.1, <0.2",
            ),
        }
    }

    fn pack_emission(&self) -> PackEmission {
        PackEmission {
            provider_id: self.metadata().provider_id,
            artifact_ref: "file://generated/provider-sharepoint-mock.gtpack".into(),
        }
    }
}

impl ProviderHealth for SharePointMockProvider {
    fn health(&self) -> Result<HealthReport, sorla_provider_core::ProviderError> {
        Ok(HealthReport {
            state: HealthState::Ready,
            message: format!(
                "SharePoint mock provider is ready for tenant {} with deterministic seed {}",
                self.config.tenant_id, self.config.seed
            ),
        })
    }
}

impl ConfigValidator for SharePointMockProvider {
    fn validate_config(&self, config_json: &str) -> Result<(), sorla_provider_core::ProviderError> {
        let parsed: SharePointMockConfig = serde_json::from_str(config_json).map_err(|err| {
            sorla_provider_core::ProviderError::Validation(format!("invalid config JSON: {err}"))
        })?;

        if parsed.seed.trim().is_empty() {
            return Err(sorla_provider_core::ProviderError::Validation(
                "seed must not be empty".into(),
            ));
        }
        if parsed.tenant_id.trim().is_empty() {
            return Err(sorla_provider_core::ProviderError::Validation(
                "tenant_id must not be empty".into(),
            ));
        }

        Ok(())
    }
}

impl ExternalReferenceProvider for SharePointMockProvider {
    fn resolve_external_reference(
        &self,
        request: ExternalReferenceRequest,
    ) -> Result<ExternalReferencePayload, sorla_provider_core::ProviderError> {
        let record = self.generate_record(&request)?;
        let (record_id, source_url) = match &record {
            SharePointMockRecord::Btg(item) => (item.document_id.clone(), item.source_url.clone()),
            SharePointMockRecord::Rfi(item) => (item.rfi_id.clone(), item.source_url.clone()),
            SharePointMockRecord::SiteVisit(item) => {
                (item.visit_id.clone(), item.source_url.clone())
            }
        };

        Ok(ExternalReferencePayload {
            record_id,
            source_url,
            content_json: serde_json::to_string(&record).map_err(|err| {
                sorla_provider_core::ProviderError::Validation(format!(
                    "serialization failed: {err}"
                ))
            })?,
        })
    }
}

pub fn pack_manifest() -> ProviderPackManifest {
    let provider = SharePointMockProvider::for_tests();
    ProviderPackManifest::from_metadata(
        &provider.metadata(),
        vec![ArtifactReference {
            kind: "gtpack-json".into(),
            uri: provider_artifact_file_uri(PROVIDER_ID),
        }],
        vec![provider_runtime_component(
            PROVIDER_ID,
            PROVIDER_VERSION,
            "sharepoint-mock-runtime",
            "provider-sharepoint-mock",
        )],
        ConfigSchemaRef {
            format: "json-schema".into(),
            path: "schemas/provider-config.schema.json".into(),
            schema_json: r#"{"type":"object","required":["seed","tenant_id"],"properties":{"seed":{"type":"string"},"tenant_id":{"type":"string"}},"additionalProperties":false}"#.into(),
        },
    )
}

pub fn catalog_entry() -> ProviderCatalogEntry {
    let manifest = pack_manifest();
    ProviderCatalogEntry {
        provider_id: manifest.provider_id,
        provider_version: manifest.provider_version,
        provider_kind: manifest.provider_kind,
        capabilities: manifest.capabilities,
        tags: vec!["external-ref".into(), "mock".into()],
        is_mock: manifest.is_mock,
        status: manifest.status,
        supported_provider_contract_version: manifest.supported_provider_contract_version,
        supported_sorla_ir: manifest.supported_sorla_ir,
        supported_sorla_ir_range: manifest.supported_sorla_ir_range,
        config_schema_path: manifest.config_schema.path,
        artifact_uri: manifest
            .artifact_references
            .first()
            .map(|item| item.uri.clone()),
        oci_reference: manifest.oci_reference,
    }
}

#[cfg(test)]
mod tests {
    use super::{SharePointMockProvider, SharePointMockRecord, catalog_entry, pack_manifest};
    use sorla_provider_core::{
        ConfigValidator, ExternalReferenceProvider, ExternalReferenceRequest, ProviderCapability,
        ProviderHealth, ProviderMetadataSource,
    };

    fn request(reference_type: &str) -> ExternalReferenceRequest {
        ExternalReferenceRequest {
            reference_type: reference_type.into(),
            reference_id: "asset-42".into(),
            building_id: Some("building-kafd-01".into()),
            floor_id: Some("floor-07".into()),
        }
    }

    #[test]
    fn sharepoint_mock_advertises_external_reference_capability() {
        let provider = SharePointMockProvider::for_tests();
        let metadata = provider.metadata();
        assert!(metadata.supports(ProviderCapability::ExternalReferenceResolve));
        assert!(metadata.is_mock);
    }

    #[test]
    fn sharepoint_mock_reports_health_and_pack_metadata() {
        let provider = SharePointMockProvider::for_tests();
        assert!(provider.health().is_ok());
        assert!(
            provider
                .validate_config(r#"{"seed":"demo-seed","tenant_id":"tenant-a"}"#)
                .is_ok()
        );
        assert!(provider.validate_config("{}").is_err());

        let manifest = pack_manifest();
        let entry = catalog_entry();

        assert_eq!(manifest.provider_id, entry.provider_id);
        assert_eq!(manifest.provider_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            manifest.oci_reference.as_deref(),
            Some("oci://ghcr.io/greenticai/sorla-providers/sharepoint-mock:0.1.4")
        );
        assert_eq!(
            provider.pack_emission().artifact_ref,
            "file://generated/provider-sharepoint-mock.gtpack"
        );
        assert_eq!(manifest.runtime_components.len(), 1);
    }

    #[test]
    fn sharepoint_mock_returns_deterministic_btg_payloads() {
        let provider = SharePointMockProvider::for_tests();
        let first = provider
            .resolve_external_reference(request("btg"))
            .expect("resolution should succeed");
        let second = provider
            .resolve_external_reference(request("btg"))
            .expect("resolution should succeed");

        assert_eq!(first, second);

        let payload: SharePointMockRecord =
            serde_json::from_str(&first.content_json).expect("payload should parse");
        match payload {
            SharePointMockRecord::Btg(item) => {
                assert_eq!(item.building_id, "building-kafd-01");
                assert_eq!(item.floor_id.as_deref(), Some("floor-07"));
                assert_eq!(item.sections.len(), 2);
                assert!(item.source_url.contains("/btg/"));
            }
            other => panic!("expected BTG payload, got {other:?}"),
        }
    }

    #[test]
    fn sharepoint_mock_supports_rfi_and_site_visit_families() {
        let provider = SharePointMockProvider::for_tests();

        let rfi = provider
            .resolve_external_reference(request("rfi"))
            .expect("rfi should resolve");
        let site_visit = provider
            .resolve_external_reference(request("site-visit"))
            .expect("site visit should resolve");

        let rfi_payload: SharePointMockRecord =
            serde_json::from_str(&rfi.content_json).expect("rfi payload should parse");
        let visit_payload: SharePointMockRecord =
            serde_json::from_str(&site_visit.content_json).expect("visit payload should parse");

        match rfi_payload {
            SharePointMockRecord::Rfi(item) => {
                assert_eq!(item.building_id, "building-kafd-01");
                assert!(item.source_url.contains("/rfi/"));
            }
            other => panic!("expected RFI payload, got {other:?}"),
        }

        match visit_payload {
            SharePointMockRecord::SiteVisit(item) => {
                assert_eq!(item.building_id, "building-kafd-01");
                assert_eq!(item.findings.len(), 3);
                assert!(item.source_url.contains("/site-visit/"));
            }
            other => panic!("expected site visit payload, got {other:?}"),
        }
    }

    #[test]
    fn sharepoint_mock_rejects_unknown_reference_types() {
        let provider = SharePointMockProvider::for_tests();
        let result = provider.resolve_external_reference(request("drawing-set"));
        assert!(result.is_err());
    }
}
