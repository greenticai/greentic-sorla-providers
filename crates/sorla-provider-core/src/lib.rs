#![forbid(unsafe_code)]

mod traits;
mod types;

pub use traits::{
    ConfigValidator, EntityLinkProvider, EntityStoreProvider, EventStoreProvider, EvidenceProvider,
    ExternalMappingProvider, ExternalReferenceProvider, OntologyGraphProvider, ProjectionProvider,
    ProviderHealth, ProviderMetadataSource,
};
pub use types::{
    AppendEventRequest, ContractCompatibility, EntityLink, EntityLinkRequest, EntityRecord,
    EntityRef, EntitySearchQuery, EventRecord, EventStreamRequest, EvidenceItem, EvidenceQuery,
    EvidenceQueryFilter, ExternalReferencePayload, ExternalReferenceRequest, HealthReport,
    HealthState, OntologyContractCompatibility, OntologyPath, OntologyPathStep, OntologyScope,
    PackEmission, PathQuery, PersistProjectionRequest, PolicyContext, PolicyContextRequest,
    ProjectionCheckpoint, ProjectionRebuildRequest, ProjectionRecord, ProviderCapability,
    ProviderError, ProviderMetadata, ProviderOntologyCapabilities, ProviderStatus,
    RelationshipDirection, RelationshipInstance, RelationshipQuery, RelationshipRef,
    RelationshipTraversalRule, TimeRange,
};

/// Canonical contract version for SoRLa provider implementations.
pub const SORLA_PROVIDER_CONTRACT_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::{
        ContractCompatibility, EntityLink, EntityLinkRequest, EntityRef, EvidenceQueryFilter,
        ExternalReferenceRequest, OntologyContractCompatibility, OntologyScope, PathQuery,
        ProviderCapability, ProviderMetadata, ProviderStatus, RelationshipDirection,
        RelationshipTraversalRule, SORLA_PROVIDER_CONTRACT_VERSION,
    };

    fn sample_metadata() -> ProviderMetadata {
        ProviderMetadata {
            provider_id: "greentic.sorla.provider.foundationdb".into(),
            display_name: "FoundationDB".into(),
            provider_kind: "event-store".into(),
            version: "0.1.0".into(),
            status: ProviderStatus::Experimental,
            is_mock: false,
            capabilities: vec![
                ProviderCapability::EventAppend,
                ProviderCapability::EventStreamRead,
                ProviderCapability::ProjectionGet,
                ProviderCapability::ProjectionPut,
            ],
            compatibility: ContractCompatibility::new(
                SORLA_PROVIDER_CONTRACT_VERSION,
                "0.1",
                "0.1",
            ),
            ontology_capabilities: None,
        }
    }

    #[test]
    fn metadata_reports_capability_presence() {
        let metadata = sample_metadata();
        assert!(metadata.supports(ProviderCapability::ProjectionGet));
        assert!(!metadata.supports(ProviderCapability::EvidenceQuery));
    }

    #[test]
    fn compatibility_tracks_contract_version() {
        let metadata = sample_metadata();
        assert_eq!(
            metadata.compatibility.supported_provider_contract_version,
            SORLA_PROVIDER_CONTRACT_VERSION
        );
    }

    #[test]
    fn entity_ref_round_trips_with_generic_fields() {
        let entity = EntityRef {
            entity_type: "Contract".into(),
            entity_id: "contract-001".into(),
            namespace: Some("demo".into()),
            version: Some("v1".into()),
        };

        let json = serde_json::to_string(&entity).expect("entity should serialize");
        let parsed: EntityRef = serde_json::from_str(&json).expect("entity should deserialize");

        assert_eq!(parsed, entity);
    }

    #[test]
    fn ontology_scope_round_trips_with_traversal_rules() {
        let scope = OntologyScope {
            root_entities: vec![EntityRef {
                entity_type: "Customer".into(),
                entity_id: "customer-001".into(),
                namespace: None,
                version: None,
            }],
            include_related: vec![RelationshipTraversalRule {
                relationship_type: Some("has_contract".into()),
                direction: RelationshipDirection::Outgoing,
                max_depth: Some(2),
            }],
            max_depth: Some(2),
            include_evidence_links: true,
        };

        let json = serde_json::to_string(&scope).expect("scope should serialize");
        let parsed: OntologyScope = serde_json::from_str(&json).expect("scope should deserialize");

        assert_eq!(parsed, scope);
        assert!(json.contains("outgoing"));
    }

    #[test]
    fn generic_evidence_query_filter_round_trips() {
        let filter = EvidenceQueryFilter {
            ontology_scope: None,
            source_types: vec!["sharepoint".into()],
            document_types: vec!["EvidenceDocument".into()],
            metadata_json: Some(r#"{"sensitivity":"internal"}"#.into()),
            time_range: None,
            sensitivity_max: Some("internal".into()),
        };

        let json = serde_json::to_string(&filter).expect("filter should serialize");
        let parsed: EvidenceQueryFilter =
            serde_json::from_str(&json).expect("filter should deserialize");

        assert_eq!(parsed, filter);
        assert!(!json.contains("building_id"));
        assert!(!json.contains("floor_id"));
    }

    #[test]
    fn generic_external_reference_request_round_trips() {
        let request = ExternalReferenceRequest {
            reference_type: "document".into(),
            reference_id: "doc-001".into(),
            source_ref: Some("sharepoint://tenant/demo/document/doc-001".into()),
            metadata_json: Some(r#"{"source_system":"sharepoint"}"#.into()),
            ontology_scope: None,
        };

        let json = serde_json::to_string(&request).expect("request should serialize");
        let parsed: ExternalReferenceRequest =
            serde_json::from_str(&json).expect("request should deserialize");

        assert_eq!(parsed, request);
        assert!(!json.contains("building_id"));
        assert!(!json.contains("floor_id"));
    }

    #[test]
    fn ontology_capabilities_have_kebab_case_names() {
        let serialized = serde_json::to_string(&ProviderCapability::OntologyScopedEvidenceQuery)
            .expect("capability should serialize");

        assert_eq!(serialized, "\"ontology-scoped-evidence-query\"");
    }

    #[test]
    fn path_query_and_entity_link_round_trip() {
        let from = EntityRef {
            entity_type: "Customer".into(),
            entity_id: "customer-001".into(),
            namespace: None,
            version: None,
        };
        let to = EntityRef {
            entity_type: "EvidenceDocument".into(),
            entity_id: "doc-001".into(),
            namespace: None,
            version: None,
        };
        let query = PathQuery {
            from,
            to: to.clone(),
            relationship_types: vec!["supports".into()],
            max_depth: 4,
            limit: 8,
        };
        let link = EntityLink {
            entity: to,
            source_ref: "sharepoint://tenant/demo/document/doc-001".into(),
            evidence_id: Some("evidence-001".into()),
            confidence: 1.0,
            match_kind: "external-id".into(),
            provenance: "test".into(),
            metadata_json: None,
        };
        let request = EntityLinkRequest {
            source_ref: Some(link.source_ref.clone()),
            evidence_id: link.evidence_id.clone(),
            content_json: None,
            candidate_types: vec!["EvidenceDocument".into()],
            ontology_scope: None,
        };

        let query_json = serde_json::to_string(&query).expect("path query should serialize");
        let link_json = serde_json::to_string(&link).expect("link should serialize");
        let request_json = serde_json::to_string(&request).expect("link request should serialize");

        assert_eq!(
            serde_json::from_str::<PathQuery>(&query_json).expect("path query should deserialize"),
            query
        );
        assert_eq!(
            serde_json::from_str::<EntityLink>(&link_json).expect("link should deserialize"),
            link
        );
        assert_eq!(
            serde_json::from_str::<EntityLinkRequest>(&request_json)
                .expect("link request should deserialize"),
            request
        );
    }

    #[test]
    fn ontology_compatibility_validates_schema_ranges() {
        let valid = OntologyContractCompatibility {
            supported_ontology_schema: "greentic.sorla.ontology.v1".into(),
            supported_ontology_schema_range: ">=1.0.0, <2.0.0".into(),
            supported_retrieval_binding_schema: Some("greentic.sorla.retrieval-bindings.v1".into()),
            supported_external_mapping_schema: None,
        };
        let invalid = OntologyContractCompatibility {
            supported_ontology_schema: "greentic.sorla.ontology.v1".into(),
            supported_ontology_schema_range: "not a range".into(),
            supported_retrieval_binding_schema: None,
            supported_external_mapping_schema: None,
        };

        assert!(valid.parses_schema_range());
        assert!(!invalid.parses_schema_range());
    }
}
