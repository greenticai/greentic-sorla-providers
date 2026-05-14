use crate::types::{
    AppendEventRequest, EntityLink, EntityLinkRequest, EntityRecord, EntityRef, EntitySearchQuery,
    EventRecord, EventStreamRequest, EvidenceItem, EvidenceQuery, ExternalReferencePayload,
    ExternalReferenceRequest, HealthReport, OntologyPath, PackEmission, PathQuery,
    PersistProjectionRequest, ProjectionCheckpoint, ProjectionRebuildRequest, ProjectionRecord,
    ProviderError, ProviderMetadata, RelationshipInstance, RelationshipQuery,
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

/// Generic ontology entity storage capabilities.
pub trait EntityStoreProvider {
    fn upsert_entity(&self, entity: EntityRecord) -> Result<EntityRecord, ProviderError>;
    fn get_entity(&self, entity: EntityRef) -> Result<Option<EntityRecord>, ProviderError>;
    fn search_entities(
        &self,
        request: EntitySearchQuery,
    ) -> Result<Vec<EntityRecord>, ProviderError>;
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
