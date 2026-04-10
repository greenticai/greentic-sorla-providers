use crate::types::{
    AppendEventRequest, EventRecord, EventStreamRequest, EvidenceItem, EvidenceQuery,
    ExternalReferencePayload, ExternalReferenceRequest, HealthReport, PackEmission,
    PersistProjectionRequest, ProjectionCheckpoint, ProjectionRebuildRequest, ProjectionRecord,
    ProviderError, ProviderMetadata,
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
