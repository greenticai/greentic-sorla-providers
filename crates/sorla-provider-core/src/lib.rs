#![forbid(unsafe_code)]

mod traits;
mod types;

pub use traits::{
    CanonicalEntityStoreProvider, CanonicalWriteProvider, ConfigValidator, EntityLinkProvider,
    EntityStoreProvider, EventStoreProvider, EvidenceProvider, ExternalMappingProvider,
    ExternalReferenceProvider, MetricProvider, OntologyGraphProvider, ProjectionProvider,
    ProviderHealth, ProviderMetadataSource,
};
pub use types::{
    AppendEventRequest, CanonicalEntityRecord, CanonicalWriteRequest, CanonicalWriteResult,
    ContractCompatibility, EntityLink, EntityLinkRequest, EntityRecord, EntityRef,
    EntitySearchQuery, EventRecord, EventStreamRequest, EvidenceItem, EvidenceQuery,
    EvidenceQueryFilter, ExternalReferencePayload, ExternalReferenceRequest, HealthReport,
    HealthState, OntologyContractCompatibility, OntologyPath, OntologyPathStep, OntologyScope,
    PackEmission, PathQuery, PersistProjectionRequest, PolicyContext, PolicyContextRequest,
    ProjectionCheckpoint, ProjectionRebuildRequest, ProjectionRecord, ProjectionSupport,
    ProviderCapability, ProviderError, ProviderIndexCapabilities, ProviderMetadata,
    ProviderMetricAggregateFunction, ProviderMetricAggregation, ProviderMetricDimension,
    ProviderMetricFilter, ProviderMetricFilterOperator, ProviderMetricQuery, ProviderMetricResult,
    ProviderMetricRow, ProviderMetricSource, ProviderMetricTimeBucket, ProviderMetricTimeGrain,
    ProviderMetricValue, ProviderOntologyCapabilities, ProviderSearchCapabilities, ProviderStatus,
    RelationshipDirection, RelationshipInstance, RelationshipQuery, RelationshipRef,
    RelationshipTraversalRule, SorEventRecord, SorNamespace, TimeRange,
};

/// Canonical contract version for SoRLa provider implementations.
pub const SORLA_PROVIDER_CONTRACT_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::{
        CanonicalEntityRecord, ContractCompatibility, EntityLink, EntityLinkRequest, EntityRef,
        EvidenceQueryFilter, ExternalReferenceRequest, OntologyContractCompatibility,
        OntologyScope, PathQuery, ProjectionSupport, ProviderCapability, ProviderIndexCapabilities,
        ProviderMetadata, ProviderMetricAggregateFunction, ProviderMetricAggregation,
        ProviderMetricDimension, ProviderMetricFilter, ProviderMetricFilterOperator,
        ProviderMetricQuery, ProviderMetricResult, ProviderMetricRow, ProviderMetricSource,
        ProviderMetricTimeBucket, ProviderMetricTimeGrain, ProviderMetricValue,
        ProviderSearchCapabilities, ProviderStatus, RelationshipDirection,
        RelationshipTraversalRule, SORLA_PROVIDER_CONTRACT_VERSION, SorEventRecord, SorNamespace,
    };
    use std::collections::BTreeMap;

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
    fn sor_namespace_maps_to_existing_entity_namespace() {
        let namespace = SorNamespace {
            tenant_id: "tenant-a".into(),
            sor_id: "contracts".into(),
            environment_id: None,
        };
        let dev_namespace = SorNamespace {
            tenant_id: "tenant-a".into(),
            sor_id: "contracts".into(),
            environment_id: Some("dev".into()),
        };

        assert_eq!(namespace.production_key(), "tenant-a\u{1f}contracts");
        assert_eq!(namespace.to_entity_namespace(), "tenant-a/contracts");
        assert_eq!(dev_namespace.production_key(), namespace.production_key());
        assert_eq!(
            dev_namespace.to_entity_namespace(),
            "tenant-a/contracts/dev"
        );
    }

    #[test]
    fn canonical_entity_record_has_stable_json_shape() {
        let record = CanonicalEntityRecord {
            namespace: SorNamespace {
                tenant_id: "tenant-a".into(),
                sor_id: "contracts".into(),
                environment_id: None,
            },
            entity_type: "Contract".into(),
            entity_id: "contract-001".into(),
            canonical_version: "2026-05-22".into(),
            revision: 7,
            data_json: serde_json::json!({
                "status": "active",
                "amount": 1250
            }),
            created_at: "2026-05-22T10:00:00Z".into(),
            updated_at: "2026-05-22T11:00:00Z".into(),
        };

        let json = serde_json::to_string(&record).expect("record should serialize");
        let parsed: CanonicalEntityRecord =
            serde_json::from_str(&json).expect("record should deserialize");

        assert_eq!(
            json,
            r#"{"namespace":{"tenant_id":"tenant-a","sor_id":"contracts"},"entity_type":"Contract","entity_id":"contract-001","canonical_version":"2026-05-22","revision":7,"data_json":{"amount":1250,"status":"active"},"created_at":"2026-05-22T10:00:00Z","updated_at":"2026-05-22T11:00:00Z"}"#
        );
        assert_eq!(parsed, record);
        assert_eq!(
            record.entity_ref(),
            EntityRef {
                entity_type: "Contract".into(),
                entity_id: "contract-001".into(),
                namespace: Some("tenant-a/contracts".into()),
                version: Some("2026-05-22".into()),
            }
        );
    }

    #[test]
    fn sor_event_record_has_stable_json_shape() {
        let record = SorEventRecord {
            namespace: SorNamespace {
                tenant_id: "tenant-a".into(),
                sor_id: "contracts".into(),
                environment_id: Some("dev".into()),
            },
            event_id: "evt-001".into(),
            stream_id: "Contract/contract-001".into(),
            sequence: 3,
            event_type: "contract.updated".into(),
            entity_ref: EntityRef {
                entity_type: "Contract".into(),
                entity_id: "contract-001".into(),
                namespace: Some("tenant-a/contracts/dev".into()),
                version: Some("2026-05-22".into()),
            },
            command_id: Some("cmd-001".into()),
            idempotency_key: Some("idem-001".into()),
            actor: Some("user:123".into()),
            source_view_version: None,
            canonical_version: "2026-05-22".into(),
            payload_json: serde_json::json!({"field": "status", "value": "active"}),
            timestamp: "2026-05-22T11:00:00Z".into(),
        };

        let json = serde_json::to_string(&record).expect("record should serialize");
        let parsed: SorEventRecord =
            serde_json::from_str(&json).expect("record should deserialize");

        assert_eq!(
            json,
            r#"{"namespace":{"tenant_id":"tenant-a","sor_id":"contracts","environment_id":"dev"},"event_id":"evt-001","stream_id":"Contract/contract-001","sequence":3,"event_type":"contract.updated","entity_ref":{"entity_type":"Contract","entity_id":"contract-001","namespace":"tenant-a/contracts/dev","version":"2026-05-22"},"command_id":"cmd-001","idempotency_key":"idem-001","actor":"user:123","canonical_version":"2026-05-22","payload_json":{"field":"status","value":"active"},"timestamp":"2026-05-22T11:00:00Z"}"#
        );
        assert_eq!(parsed, record);
        assert!(!json.contains("source_view_version"));
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
    fn new_capabilities_have_kebab_case_names() {
        assert_eq!(
            serde_json::to_string(&ProviderCapability::CanonicalState)
                .expect("capability should serialize"),
            "\"canonical-state\""
        );
        assert_eq!(
            serde_json::to_string(&ProviderCapability::TextSearchProjection)
                .expect("capability should serialize"),
            "\"text-search-projection\""
        );
    }

    #[test]
    fn metric_capabilities_have_kebab_case_names() {
        assert_eq!(
            serde_json::to_string(&ProviderCapability::MetricAggregateDistinctCount)
                .expect("capability should serialize"),
            "\"metric-aggregate-distinct-count\""
        );
        assert_eq!(
            serde_json::to_string(&ProviderCapability::MetricDimensionGroupBy)
                .expect("capability should serialize"),
            "\"metric-dimension-group-by\""
        );
        assert_eq!(
            serde_json::to_string(&ProviderCapability::MetricTimeBucketMonth)
                .expect("capability should serialize"),
            "\"metric-time-bucket-month\""
        );
    }

    #[test]
    fn metric_query_round_trips_with_generic_source_and_filters() {
        let query = ProviderMetricQuery {
            source: ProviderMetricSource::CanonicalEntities {
                namespace: SorNamespace {
                    tenant_id: "tenant-a".into(),
                    sor_id: "commerce".into(),
                    environment_id: Some("dev".into()),
                },
                entity_type: "Order".into(),
            },
            aggregations: vec![
                ProviderMetricAggregation {
                    alias: "order_count".into(),
                    function: ProviderMetricAggregateFunction::Count,
                    field: None,
                },
                ProviderMetricAggregation {
                    alias: "revenue".into(),
                    function: ProviderMetricAggregateFunction::Sum,
                    field: Some("amount".into()),
                },
            ],
            filters: vec![ProviderMetricFilter {
                field: "status".into(),
                operator: ProviderMetricFilterOperator::Equals,
                value: Some(serde_json::json!("paid")),
                values: vec![],
            }],
            dimensions: vec![ProviderMetricDimension {
                field: "campaign_id".into(),
                alias: Some("campaign".into()),
            }],
            time_bucket: Some(ProviderMetricTimeBucket {
                field: "created_at".into(),
                grain: ProviderMetricTimeGrain::Month,
                alias: Some("month".into()),
            }),
            limit: Some(100),
        };

        let json = serde_json::to_string(&query).expect("metric query should serialize");
        let parsed: ProviderMetricQuery =
            serde_json::from_str(&json).expect("metric query should deserialize");

        assert_eq!(parsed, query);
        assert!(json.contains("\"kind\":\"canonical-entities\""));
        assert!(json.contains("\"function\":\"sum\""));
        assert!(json.contains("\"operator\":\"equals\""));
        assert!(!json.contains("daily_clicks"));
    }

    #[test]
    fn metric_result_has_stable_row_shape() {
        let mut dimensions = BTreeMap::new();
        dimensions.insert(
            "month".into(),
            ProviderMetricValue::String("2026-05".into()),
        );
        dimensions.insert(
            "campaign".into(),
            ProviderMetricValue::String("campaign-a".into()),
        );

        let mut metrics = BTreeMap::new();
        metrics.insert("order_count".into(), ProviderMetricValue::Number(3.0));
        metrics.insert("revenue".into(), ProviderMetricValue::Number(120.5));

        let result = ProviderMetricResult {
            source: ProviderMetricSource::Fixture {
                name: "commerce".into(),
            },
            rows: vec![ProviderMetricRow {
                dimensions,
                metrics,
            }],
        };

        let json = serde_json::to_string(&result).expect("metric result should serialize");
        let parsed: ProviderMetricResult =
            serde_json::from_str(&json).expect("metric result should deserialize");

        assert_eq!(parsed, result);
        assert_eq!(
            json,
            r#"{"source":{"kind":"fixture","name":"commerce"},"rows":[{"dimensions":{"campaign":{"type":"string","value":"campaign-a"},"month":{"type":"string","value":"2026-05"}},"metrics":{"order_count":{"type":"number","value":3.0},"revenue":{"type":"number","value":120.5}}}]}"#
        );
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

    #[test]
    fn ontology_capabilities_deserialize_legacy_v1_without_new_metadata() {
        let json = r#"{
            "schema":"greentic.sorla.provider.ontology-capabilities.v1",
            "compatibility":{
                "supported_ontology_schema":"greentic.sorla.ontology.v1",
                "supported_ontology_schema_range":">=1.0.0, <2.0.0",
                "supported_retrieval_binding_schema":null,
                "supported_external_mapping_schema":null
            },
            "supports_entity_read":true,
            "supports_entity_search":true,
            "supports_relationship_query":false,
            "supports_path_find":false,
            "supports_entity_linking":false,
            "supports_ontology_scoped_evidence":false,
            "supported_concept_types":["*"],
            "supported_relationship_types":[],
            "max_traversal_depth":null,
            "supports_policy_context":false
        }"#;

        let capabilities: super::ProviderOntologyCapabilities =
            serde_json::from_str(json).expect("legacy capabilities should deserialize");

        assert!(capabilities.index_capabilities.is_none());
        assert!(capabilities.search_capabilities.is_none());
    }

    #[test]
    fn structured_index_and_search_capabilities_round_trip() {
        let index = ProviderIndexCapabilities {
            exact: true,
            composite: false,
        };
        let search = ProviderSearchCapabilities {
            text_projection: ProjectionSupport::Optional,
            vector_projection: ProjectionSupport::Unavailable,
        };

        let json = serde_json::to_string(&(index.clone(), search.clone()))
            .expect("capabilities should serialize");
        let parsed: (ProviderIndexCapabilities, ProviderSearchCapabilities) =
            serde_json::from_str(&json).expect("capabilities should deserialize");

        assert_eq!(parsed, (index, search));
        assert!(json.contains("text_projection"));
        assert!(json.contains("optional"));
    }
}
