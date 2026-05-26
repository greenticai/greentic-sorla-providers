use crate::types::{
    AppendEventRequest, CanonicalEntityRecord, CanonicalWriteRequest, CanonicalWriteResult,
    EntityLink, EntityLinkRequest, EntityRecord, EntityRef, EntitySearchQuery, EventRecord,
    EventStreamRequest, EvidenceItem, EvidenceQuery, ExternalReferencePayload,
    ExternalReferenceRequest, HealthReport, OntologyPath, PackEmission, PathQuery,
    PersistProjectionRequest, ProjectionCheckpoint, ProjectionRebuildRequest, ProjectionRecord,
    ProviderError, ProviderMetadata, ProviderMetricQuery, ProviderMetricResult,
    RelationshipInstance, RelationshipQuery, SorNamespace,
};

/// Exposes stable provider identity and capability metadata.
pub trait ProviderMetadataSource {
    fn metadata(&self) -> ProviderMetadata;
    fn pack_emission(&self) -> PackEmission;
}

/// Reports provider health.
pub trait ProviderHealth {
    fn health(&self) -> Result<HealthReport, ProviderError>;
}

/// Validates provider configuration.
pub trait ConfigValidator {
    fn validate_config(&self, config_json: &str) -> Result<(), ProviderError>;
}

/// Event-native provider capabilities.
pub trait EventStoreProvider {
    fn append_event(&self, request: AppendEventRequest) -> Result<EventRecord, ProviderError>;
    fn read_event_stream(
        &self,
        request: EventStreamRequest,
    ) -> Result<Vec<EventRecord>, ProviderError>;
}

/// Projection read and rebuild capabilities.
pub trait ProjectionProvider {
    fn persist_projection(
        &self,
        request: PersistProjectionRequest,
    ) -> Result<ProjectionRecord, ProviderError>;
    fn get_projection(
        &self,
        projection_name: &str,
        projection_key: &str,
    ) -> Result<Option<ProjectionRecord>, ProviderError>;
    fn rebuild_projection(
        &self,
        request: ProjectionRebuildRequest,
    ) -> Result<ProjectionCheckpoint, ProviderError>;
}

/// External source-of-record resolution capabilities.
pub trait ExternalReferenceProvider {
    fn resolve_external_reference(
        &self,
        request: ExternalReferenceRequest,
    ) -> Result<ExternalReferencePayload, ProviderError>;
}

/// Evidence query and evidence lookup capabilities.
pub trait EvidenceProvider {
    fn query_evidence(&self, query: EvidenceQuery) -> Result<Vec<EvidenceItem>, ProviderError>;
}

/// Metric query execution capabilities.
pub trait MetricProvider {
    fn query_metric(
        &self,
        query: ProviderMetricQuery,
    ) -> Result<ProviderMetricResult, ProviderError>;
}

/// Generic ontology entity storage capabilities.
pub trait EntityStoreProvider {
    fn upsert_entity(&self, entity: EntityRecord) -> Result<EntityRecord, ProviderError>;
    fn get_entity(&self, entity: EntityRef) -> Result<Option<EntityRecord>, ProviderError>;
    fn search_entities(
        &self,
        request: EntitySearchQuery,
    ) -> Result<Vec<EntityRecord>, ProviderError>;
}

/// Canonical SORX entity storage capabilities.
///
/// This is a canonical source-of-record contract rather than a replacement for
/// the generic ontology `EntityStoreProvider`. Implementations may expose both
/// when they can persist canonical revisions and also serve generic entity refs.
pub trait CanonicalEntityStoreProvider {
    fn upsert_canonical_entity(
        &self,
        record: CanonicalEntityRecord,
    ) -> Result<CanonicalEntityRecord, ProviderError>;
    fn get_canonical_entity(
        &self,
        namespace: SorNamespace,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Option<CanonicalEntityRecord>, ProviderError>;
}

/// Atomic canonical SORX write capabilities.
pub trait CanonicalWriteProvider {
    fn apply_canonical_write(
        &self,
        request: CanonicalWriteRequest,
    ) -> Result<CanonicalWriteResult, ProviderError>;
}

/// Generic ontology graph traversal capabilities.
pub trait OntologyGraphProvider {
    fn query_relationships(
        &self,
        request: RelationshipQuery,
    ) -> Result<Vec<RelationshipInstance>, ProviderError>;

    fn find_paths(&self, request: PathQuery) -> Result<Vec<OntologyPath>, ProviderError>;
}

/// Links external content and evidence to generic ontology entities.
pub trait EntityLinkProvider {
    fn link_entities(&self, request: EntityLinkRequest) -> Result<Vec<EntityLink>, ProviderError>;
}

/// Validates provider-specific external mapping documents.
pub trait ExternalMappingProvider {
    fn validate_mapping(&self, mapping_json: &str) -> Result<(), ProviderError>;
}
