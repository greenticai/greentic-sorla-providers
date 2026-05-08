#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sorla_provider_catalog::ProviderCatalogEntry;
use sorla_provider_core::{
    ConfigValidator, ContractCompatibility, EvidenceItem, EvidenceProvider, EvidenceQuery,
    EvidenceQueryFilter, HealthReport, HealthState, PackEmission, ProviderCapability,
    ProviderHealth, ProviderMetadata, ProviderMetadataSource, ProviderStatus,
    SORLA_PROVIDER_CONTRACT_VERSION,
};
use sorla_provider_pack::{
    ArtifactReference, ConfigSchemaRef, ProviderPackManifest, provider_artifact_file_uri,
    provider_runtime_component,
};

const PROVIDER_ID: &str = "greentic.sorla.provider.rag-mock";
const PROVIDER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RagMockConfig {
    pub seed: String,
    pub max_results: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RagSeedDocument {
    source_type: String,
    document_type: String,
    document_id: String,
    building_id: String,
    floor_id: Option<String>,
    title: String,
    section_id: Option<String>,
    page: Option<u32>,
}

pub struct RagMockProvider {
    config: RagMockConfig,
}

impl RagMockProvider {
    pub fn new(config: RagMockConfig) -> Self {
        Self { config }
    }

    pub fn for_tests() -> Self {
        Self::new(RagMockConfig {
            seed: "greentic-sorla-rag-mock".into(),
            max_results: 8,
        })
    }

    fn stable_hash(&self, query: &EvidenceQuery, doc: &RagSeedDocument) -> u64 {
        let mut hash = 1469598103934665603u64;
        for byte in self
            .config
            .seed
            .bytes()
            .chain(query.query.bytes())
            .chain(doc.document_id.bytes())
            .chain(doc.source_type.bytes())
            .chain(doc.building_id.bytes())
            .chain(doc.floor_id.as_deref().unwrap_or_default().bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(1099511628211);
        }
        hash
    }

    fn seed_documents(&self) -> Vec<RagSeedDocument> {
        vec![
            RagSeedDocument {
                source_type: "btg".into(),
                document_type: "btg".into(),
                document_id: "btg-building-kafd-01".into(),
                building_id: "building-kafd-01".into(),
                floor_id: Some("floor-07".into()),
                title: "BTG smoke control review".into(),
                section_id: Some("section-smoke-control".into()),
                page: Some(12),
            },
            RagSeedDocument {
                source_type: "rfi".into(),
                document_type: "rfi".into(),
                document_id: "rfi-building-kafd-01".into(),
                building_id: "building-kafd-01".into(),
                floor_id: Some("floor-07".into()),
                title: "RFI damper access clarification".into(),
                section_id: None,
                page: Some(3),
            },
            RagSeedDocument {
                source_type: "site-visit".into(),
                document_type: "site-visit".into(),
                document_id: "visit-building-kafd-01".into(),
                building_id: "building-kafd-01".into(),
                floor_id: Some("floor-07".into()),
                title: "Site visit field notes".into(),
                section_id: None,
                page: Some(1),
            },
            RagSeedDocument {
                source_type: "btg".into(),
                document_type: "btg".into(),
                document_id: "btg-building-kafd-02".into(),
                building_id: "building-kafd-02".into(),
                floor_id: Some("floor-03".into()),
                title: "BTG alarm zoning summary".into(),
                section_id: Some("section-alarm-zoning".into()),
                page: Some(6),
            },
        ]
    }

    fn matches_filter(doc: &RagSeedDocument, filter: &EvidenceQueryFilter) -> bool {
        if let Some(building) = &filter.building_id
            && &doc.building_id != building
        {
            return false;
        }
        if let Some(floor) = &filter.floor_id
            && doc.floor_id.as_ref() != Some(floor)
        {
            return false;
        }
        if let Some(document_type) = &filter.document_type
            && &doc.document_type != document_type
        {
            return false;
        }
        if let Some(source_type) = &filter.source_type
            && &doc.source_type != source_type
        {
            return false;
        }
        true
    }

    fn score_for(&self, query: &EvidenceQuery, doc: &RagSeedDocument) -> f32 {
        let query_lower = query.query.to_ascii_lowercase();
        let title_lower = doc.title.to_ascii_lowercase();

        let mut score = 0.45_f32;
        if query_lower.contains(&doc.source_type) {
            score += 0.15;
        }
        if query_lower.contains(&doc.building_id.to_ascii_lowercase()) {
            score += 0.15;
        }
        if let Some(floor) = &doc.floor_id
            && query_lower.contains(&floor.to_ascii_lowercase())
        {
            score += 0.1;
        }
        if title_lower
            .split_whitespace()
            .any(|token| query_lower.contains(token))
        {
            score += 0.1;
        }
        let jitter = (self.stable_hash(query, doc) % 10) as f32 / 100.0;
        (score + jitter).min(0.99)
    }

    fn build_item(&self, query: &EvidenceQuery, doc: &RagSeedDocument) -> EvidenceItem {
        let hash = self.stable_hash(query, doc);
        let evidence_id = format!("evidence-{hash:016x}");
        let chunk_id = format!("chunk-{hash:016x}");
        let source_ref = format!(
            "sharepoint://{building}/{document}",
            building = doc.building_id,
            document = doc.document_id
        );
        let snippet = format!(
            "{} for {} highlights {} on {}.",
            doc.title,
            doc.building_id,
            query.query,
            doc.floor_id
                .clone()
                .unwrap_or_else(|| "shared levels".into())
        );
        let provenance = format!(
            "source={} document={} section={} page={} seed={}",
            doc.source_type,
            doc.document_id,
            doc.section_id.as_deref().unwrap_or("n/a"),
            doc.page.unwrap_or(0),
            self.config.seed
        );
        let metadata_json = serde_json::json!({
            "building_id": doc.building_id,
            "floor_id": doc.floor_id,
            "document_type": doc.document_type,
            "title": doc.title,
        })
        .to_string();

        EvidenceItem {
            evidence_id,
            source_type: doc.source_type.clone(),
            source_ref,
            document_id: doc.document_id.clone(),
            section_id: doc.section_id.clone(),
            page: doc.page,
            chunk_id,
            snippet,
            score: self.score_for(query, doc),
            provenance,
            metadata_json,
        }
    }
}

impl ProviderMetadataSource for RagMockProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            provider_id: PROVIDER_ID.into(),
            display_name: "RAG Mock".into(),
            provider_kind: "evidence".into(),
            version: PROVIDER_VERSION.into(),
            status: ProviderStatus::Experimental,
            is_mock: true,
            capabilities: vec![
                ProviderCapability::EvidenceQuery,
                ProviderCapability::EvidenceResolve,
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
            artifact_ref: "file://generated/provider-rag-mock.gtpack".into(),
        }
    }
}

impl ProviderHealth for RagMockProvider {
    fn health(&self) -> Result<HealthReport, sorla_provider_core::ProviderError> {
        Ok(HealthReport {
            state: HealthState::Ready,
            message: format!(
                "RAG mock provider is ready with deterministic seed {} and max_results {}",
                self.config.seed, self.config.max_results
            ),
        })
    }
}

impl ConfigValidator for RagMockProvider {
    fn validate_config(&self, config_json: &str) -> Result<(), sorla_provider_core::ProviderError> {
        let parsed: RagMockConfig = serde_json::from_str(config_json).map_err(|err| {
            sorla_provider_core::ProviderError::Validation(format!("invalid config JSON: {err}"))
        })?;

        if parsed.seed.trim().is_empty() {
            return Err(sorla_provider_core::ProviderError::Validation(
                "seed must not be empty".into(),
            ));
        }
        if parsed.max_results == 0 {
            return Err(sorla_provider_core::ProviderError::Validation(
                "max_results must be greater than zero".into(),
            ));
        }

        Ok(())
    }
}

impl EvidenceProvider for RagMockProvider {
    fn query_evidence(
        &self,
        query: EvidenceQuery,
    ) -> Result<Vec<EvidenceItem>, sorla_provider_core::ProviderError> {
        let limit = query.limit.min(self.config.max_results).max(1);
        let mut items = self
            .seed_documents()
            .into_iter()
            .filter(|doc| Self::matches_filter(doc, &query.filter))
            .map(|doc| self.build_item(&query, &doc))
            .collect::<Vec<_>>();

        items.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.evidence_id.cmp(&right.evidence_id))
        });
        items.truncate(limit);

        Ok(items)
    }
}

pub fn pack_manifest() -> ProviderPackManifest {
    let provider = RagMockProvider::for_tests();
    ProviderPackManifest::from_metadata(
        &provider.metadata(),
        vec![ArtifactReference {
            kind: "gtpack-json".into(),
            uri: provider_artifact_file_uri(PROVIDER_ID),
        }],
        vec![provider_runtime_component(
            PROVIDER_ID,
            PROVIDER_VERSION,
            "rag-mock-runtime",
            "provider-rag-mock",
        )],
        ConfigSchemaRef {
            format: "json-schema".into(),
            path: "schemas/provider-config.schema.json".into(),
            schema_json: r#"{"type":"object","required":["seed","max_results"],"properties":{"seed":{"type":"string"},"max_results":{"type":"integer","minimum":1}},"additionalProperties":false}"#.into(),
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
        tags: vec!["evidence".into(), "mock".into()],
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
    use super::{RagMockProvider, catalog_entry, pack_manifest};
    use sorla_provider_core::{
        ConfigValidator, EvidenceProvider, EvidenceQuery, EvidenceQueryFilter, ProviderCapability,
        ProviderHealth, ProviderMetadataSource,
    };

    fn base_query() -> EvidenceQuery {
        EvidenceQuery {
            query: "Show BTG and RFI evidence for building-kafd-01 floor-07 smoke control".into(),
            filter: EvidenceQueryFilter {
                building_id: Some("building-kafd-01".into()),
                floor_id: Some("floor-07".into()),
                document_type: None,
                source_type: None,
            },
            limit: 5,
        }
    }

    #[test]
    fn rag_mock_advertises_evidence_capabilities() {
        let provider = RagMockProvider::for_tests();
        let metadata = provider.metadata();
        assert!(metadata.supports(ProviderCapability::EvidenceQuery));
        assert!(metadata.supports(ProviderCapability::EvidenceResolve));
    }

    #[test]
    fn rag_mock_reports_health_and_pack_metadata() {
        let provider = RagMockProvider::for_tests();
        assert!(provider.health().is_ok());
        assert!(
            provider
                .validate_config(r#"{"seed":"demo-seed","max_results":4}"#)
                .is_ok()
        );
        assert!(provider.validate_config("{}").is_err());

        let manifest = pack_manifest();
        let entry = catalog_entry();

        assert_eq!(manifest.provider_id, entry.provider_id);
        assert_eq!(manifest.provider_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            manifest.oci_reference.as_deref(),
            Some("oci://ghcr.io/greenticai/sorla-providers/rag-mock:0.1.4")
        );
        assert_eq!(
            provider.pack_emission().artifact_ref,
            "file://generated/provider-rag-mock.gtpack"
        );
        assert_eq!(manifest.runtime_components.len(), 1);
    }

    #[test]
    fn rag_mock_returns_deterministic_results() {
        let provider = RagMockProvider::for_tests();
        let first = provider
            .query_evidence(base_query())
            .expect("query should succeed");
        let second = provider
            .query_evidence(base_query())
            .expect("query should succeed");

        assert_eq!(first, second);
        assert!(!first.is_empty());
        assert!(first[0].provenance.contains("source="));
    }

    #[test]
    fn rag_mock_applies_source_and_document_filters() {
        let provider = RagMockProvider::for_tests();
        let mut query = base_query();
        query.filter.document_type = Some("btg".into());
        query.filter.source_type = Some("btg".into());

        let items = provider
            .query_evidence(query)
            .expect("filtered query should succeed");

        assert!(!items.is_empty());
        assert!(items.iter().all(|item| item.source_type == "btg"));
    }

    #[test]
    fn rag_mock_respects_result_limits() {
        let provider = RagMockProvider::for_tests();
        let mut query = base_query();
        query.limit = 1;

        let items = provider
            .query_evidence(query)
            .expect("limited query should succeed");

        assert_eq!(items.len(), 1);
    }
}
