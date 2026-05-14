#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sorla_provider_catalog::{ProviderCatalogEntry, ProviderCatalogOntology};
use sorla_provider_core::{
    ConfigValidator, ContractCompatibility, EntityLink, EntityLinkProvider, EntityLinkRequest,
    EntityRef, ExternalMappingProvider, ExternalReferencePayload, ExternalReferenceProvider,
    ExternalReferenceRequest, HealthReport, HealthState, OntologyContractCompatibility,
    PackEmission, ProviderCapability, ProviderHealth, ProviderMetadata, ProviderMetadataSource,
    ProviderOntologyCapabilities, ProviderStatus, SORLA_PROVIDER_CONTRACT_VERSION,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ExternalReferenceMetadata {
    building_id: Option<String>,
    floor_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExternalMappingDocument {
    schema: String,
    provider_id: String,
    mappings: Vec<ExternalMappingRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExternalMappingRule {
    source_type: String,
    target_concept: String,
    id_field: String,
    entity_fields: BTreeMap<String, String>,
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
            .chain(request.source_ref.as_deref().unwrap_or_default().bytes())
            .chain(request.metadata_json.as_deref().unwrap_or_default().bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(1099511628211);
        }
        hash
    }

    fn request_metadata(
        request: &ExternalReferenceRequest,
    ) -> Result<ExternalReferenceMetadata, sorla_provider_core::ProviderError> {
        request
            .metadata_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|err| {
                sorla_provider_core::ProviderError::Validation(format!(
                    "invalid external reference metadata JSON: {err}"
                ))
            })
            .map(|metadata| metadata.unwrap_or_default())
    }

    fn building_id(&self, request: &ExternalReferenceRequest) -> String {
        Self::request_metadata(request)
            .ok()
            .and_then(|metadata| metadata.building_id)
            .unwrap_or_else(|| format!("building-{}", self.stable_hash(request) % 1000))
    }

    fn floor_id(&self, request: &ExternalReferenceRequest) -> Option<String> {
        Self::request_metadata(request)
            .ok()
            .and_then(|metadata| metadata.floor_id)
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

    fn source_ref_for(&self, family: &str, record_id: &str) -> String {
        match family {
            "btg" => format!(
                "sharepoint://tenant/{tenant}/document/{record_id}",
                tenant = self.config.tenant_id
            ),
            "rfi" | "site-visit" => format!(
                "sharepoint://tenant/{tenant}/list/{family}/item/{record_id}",
                tenant = self.config.tenant_id
            ),
            _ => format!(
                "sharepoint://tenant/{tenant}/document/{record_id}",
                tenant = self.config.tenant_id
            ),
        }
    }

    fn link_from_parts(
        &self,
        entity_id: String,
        source_ref: String,
        metadata_json: Option<String>,
    ) -> EntityLink {
        EntityLink {
            entity: EntityRef {
                entity_type: "EvidenceDocument".into(),
                entity_id,
                namespace: Some(self.config.tenant_id.clone()),
                version: None,
            },
            source_ref,
            evidence_id: None,
            confidence: 1.0,
            match_kind: "external-id".into(),
            provenance: "sharepoint-mock deterministic mapping".into(),
            metadata_json,
        }
    }

    fn link_from_source_ref(
        &self,
        source_ref: &str,
    ) -> Result<EntityLink, sorla_provider_core::ProviderError> {
        let expected_prefix = format!("sharepoint://tenant/{}/", self.config.tenant_id);
        if !source_ref.starts_with(&expected_prefix) {
            return Err(sorla_provider_core::ProviderError::Validation(format!(
                "source_ref must start with {expected_prefix}"
            )));
        }

        let parts = source_ref[expected_prefix.len()..]
            .split('/')
            .collect::<Vec<_>>();
        let entity_id = match parts.as_slice() {
            ["document", document_id] => *document_id,
            ["list", _list_id, "item", item_id] => *item_id,
            _ => {
                return Err(sorla_provider_core::ProviderError::Validation(
                    "unsupported SharePoint mock source_ref shape".into(),
                ));
            }
        };

        Ok(self.link_from_parts(entity_id.into(), source_ref.into(), None))
    }

    fn record_identity(&self, record: &SharePointMockRecord) -> (String, String, String) {
        match record {
            SharePointMockRecord::Btg(item) => (
                item.document_id.clone(),
                self.source_ref_for("btg", &item.document_id),
                serde_json::json!({
                    "family": "btg",
                    "building_id": item.building_id,
                    "floor_id": item.floor_id,
                    "title": item.title,
                    "source_url": item.source_url,
                })
                .to_string(),
            ),
            SharePointMockRecord::Rfi(item) => (
                item.rfi_id.clone(),
                self.source_ref_for("rfi", &item.rfi_id),
                serde_json::json!({
                    "family": "rfi",
                    "building_id": item.building_id,
                    "floor_id": item.floor_id,
                    "status": item.status,
                    "source_url": item.source_url,
                })
                .to_string(),
            ),
            SharePointMockRecord::SiteVisit(item) => (
                item.visit_id.clone(),
                self.source_ref_for("site-visit", &item.visit_id),
                serde_json::json!({
                    "family": "site-visit",
                    "building_id": item.building_id,
                    "floor_id": item.floor_id,
                    "source_url": item.source_url,
                })
                .to_string(),
            ),
        }
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
        Self::request_metadata(request)?;
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
                ProviderCapability::ExternalMappingValidate,
                ProviderCapability::EntityLink,
                ProviderCapability::HealthCheck,
                ProviderCapability::ConfigValidate,
                ProviderCapability::PackMetadataEmit,
            ],
            compatibility: ContractCompatibility::new(
                SORLA_PROVIDER_CONTRACT_VERSION,
                "0.1",
                ">=0.1, <0.2",
            ),
            ontology_capabilities: Some(ProviderOntologyCapabilities {
                schema: "greentic.sorla.provider.ontology-capabilities.v1".into(),
                compatibility: OntologyContractCompatibility {
                    supported_ontology_schema: "greentic.sorla.ontology.v1".into(),
                    supported_ontology_schema_range: ">=1.0.0, <2.0.0".into(),
                    supported_retrieval_binding_schema: None,
                    supported_external_mapping_schema: Some(
                        "greentic.sorla.external-mapping.v1".into(),
                    ),
                },
                supports_entity_read: false,
                supports_entity_search: false,
                supports_relationship_query: false,
                supports_path_find: false,
                supports_entity_linking: true,
                supports_ontology_scoped_evidence: false,
                supported_concept_types: vec!["EvidenceDocument".into()],
                supported_relationship_types: vec![],
                max_traversal_depth: None,
                supports_policy_context: false,
            }),
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

impl ExternalMappingProvider for SharePointMockProvider {
    fn validate_mapping(
        &self,
        mapping_json: &str,
    ) -> Result<(), sorla_provider_core::ProviderError> {
        let mapping: ExternalMappingDocument =
            serde_json::from_str(mapping_json).map_err(|err| {
                sorla_provider_core::ProviderError::Validation(format!(
                    "invalid external mapping JSON: {err}"
                ))
            })?;

        if mapping.schema != "greentic.sorla.external-mapping.v1" {
            return Err(sorla_provider_core::ProviderError::Validation(
                "unsupported external mapping schema".into(),
            ));
        }
        if mapping.provider_id != PROVIDER_ID {
            return Err(sorla_provider_core::ProviderError::Validation(
                "external mapping provider_id does not match sharepoint mock".into(),
            ));
        }
        if mapping.mappings.is_empty() {
            return Err(sorla_provider_core::ProviderError::Validation(
                "external mapping must include at least one mapping".into(),
            ));
        }

        for rule in mapping.mappings {
            if rule.source_type.trim().is_empty()
                || rule.target_concept.trim().is_empty()
                || rule.id_field.trim().is_empty()
                || rule.entity_fields.is_empty()
            {
                return Err(sorla_provider_core::ProviderError::Validation(
                    "external mapping rules must include source_type, target_concept, id_field, and entity_fields".into(),
                ));
            }
        }

        Ok(())
    }
}

impl EntityLinkProvider for SharePointMockProvider {
    fn link_entities(
        &self,
        request: EntityLinkRequest,
    ) -> Result<Vec<EntityLink>, sorla_provider_core::ProviderError> {
        let mut links = Vec::new();

        if let Some(source_ref) = request.source_ref.as_deref() {
            links.push(self.link_from_source_ref(source_ref)?);
        }

        if let Some(content_json) = request.content_json.as_deref() {
            let record: SharePointMockRecord =
                serde_json::from_str(content_json).map_err(|err| {
                    sorla_provider_core::ProviderError::Validation(format!(
                        "invalid SharePoint mock content JSON: {err}"
                    ))
                })?;
            let (entity_id, source_ref, metadata_json) = self.record_identity(&record);
            links.push(self.link_from_parts(entity_id, source_ref, Some(metadata_json)));
        }

        links.sort_by(|left, right| {
            left.entity
                .entity_id
                .cmp(&right.entity.entity_id)
                .then_with(|| left.source_ref.cmp(&right.source_ref))
        });
        links.dedup_by(|left, right| {
            left.entity == right.entity && left.source_ref == right.source_ref
        });

        if !request.candidate_types.is_empty() {
            links.retain(|link| {
                request
                    .candidate_types
                    .iter()
                    .any(|candidate_type| candidate_type == &link.entity.entity_type)
            });
        }

        Ok(links)
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
        ontology: manifest.ontology_capabilities.as_ref().map(|capabilities| {
            ProviderCatalogOntology {
                capabilities: vec![ProviderCapability::EntityLink],
                max_traversal_depth: None,
                supports_generic_entity_refs: true,
                supported_ontology_schema: capabilities
                    .compatibility
                    .supported_ontology_schema
                    .clone(),
                supported_ontology_schema_range: capabilities
                    .compatibility
                    .supported_ontology_schema_range
                    .clone(),
                supported_retrieval_binding_schema: capabilities
                    .compatibility
                    .supported_retrieval_binding_schema
                    .clone(),
                supported_external_mapping_schema: capabilities
                    .compatibility
                    .supported_external_mapping_schema
                    .clone(),
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{SharePointMockProvider, SharePointMockRecord, catalog_entry, pack_manifest};
    use sorla_provider_core::{
        ConfigValidator, EntityLinkProvider, EntityLinkRequest, ExternalMappingProvider,
        ExternalReferenceProvider, ExternalReferenceRequest, ProviderCapability, ProviderHealth,
        ProviderMetadataSource,
    };

    fn request(reference_type: &str) -> ExternalReferenceRequest {
        ExternalReferenceRequest {
            reference_type: reference_type.into(),
            reference_id: "asset-42".into(),
            source_ref: Some("sharepoint://tenant/ka-fd-demo/document/asset-42".into()),
            metadata_json: Some(
                r#"{"building_id":"building-kafd-01","floor_id":"floor-07"}"#.into(),
            ),
            ontology_scope: None,
        }
    }

    #[test]
    fn sharepoint_mock_advertises_external_reference_capability() {
        let provider = SharePointMockProvider::for_tests();
        let metadata = provider.metadata();
        assert!(metadata.supports(ProviderCapability::ExternalReferenceResolve));
        assert!(metadata.supports(ProviderCapability::ExternalMappingValidate));
        assert!(metadata.supports(ProviderCapability::EntityLink));
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
        assert!(manifest.ontology_capabilities.is_some());
        assert!(entry.ontology.is_some());
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

    #[test]
    fn sharepoint_mock_validates_generic_external_mapping() {
        let provider = SharePointMockProvider::for_tests();
        let valid = r#"{
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
        }"#;
        let invalid = r#"{
            "schema": "greentic.sorla.external-mapping.v1",
            "provider_id": "other",
            "mappings": []
        }"#;

        assert!(provider.validate_mapping(valid).is_ok());
        assert!(provider.validate_mapping(invalid).is_err());
    }

    #[test]
    fn sharepoint_mock_links_generic_entities_from_source_ref() {
        let provider = SharePointMockProvider::for_tests();
        let links = provider
            .link_entities(EntityLinkRequest {
                source_ref: Some("sharepoint://tenant/ka-fd-demo/document/doc-123".into()),
                evidence_id: None,
                content_json: None,
                candidate_types: vec!["EvidenceDocument".into()],
                ontology_scope: None,
            })
            .expect("linking should succeed");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].entity.entity_type, "EvidenceDocument");
        assert_eq!(links[0].entity.entity_id, "doc-123");
        assert_eq!(links[0].match_kind, "external-id");
        assert_eq!(
            links[0].source_ref,
            "sharepoint://tenant/ka-fd-demo/document/doc-123"
        );
    }

    #[test]
    fn sharepoint_mock_links_generic_entities_from_content_metadata() {
        let provider = SharePointMockProvider::for_tests();
        let payload = provider
            .resolve_external_reference(request("btg"))
            .expect("resolution should succeed");
        let links = provider
            .link_entities(EntityLinkRequest {
                source_ref: None,
                evidence_id: None,
                content_json: Some(payload.content_json),
                candidate_types: vec![],
                ontology_scope: None,
            })
            .expect("linking should succeed");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].entity.entity_type, "EvidenceDocument");
        assert!(
            links[0]
                .metadata_json
                .as_deref()
                .is_some_and(|metadata| metadata.contains("building_id"))
        );
    }
}
