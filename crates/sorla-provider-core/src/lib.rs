#![forbid(unsafe_code)]

mod traits;
mod types;

pub use traits::{
    ConfigValidator, EventStoreProvider, EvidenceProvider, ExternalReferenceProvider,
    ProjectionProvider, ProviderHealth, ProviderMetadataSource,
};
pub use types::{
    AppendEventRequest, ContractCompatibility, EventRecord, EventStreamRequest, EvidenceItem,
    EvidenceQuery, EvidenceQueryFilter, ExternalReferencePayload, ExternalReferenceRequest,
    HealthReport, HealthState, PackEmission, PersistProjectionRequest, ProjectionCheckpoint,
    ProjectionRebuildRequest, ProjectionRecord, ProviderCapability, ProviderError,
    ProviderMetadata, ProviderStatus,
};

/// Canonical contract version for SoRLa provider implementations.
pub const SORLA_PROVIDER_CONTRACT_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::{
        ContractCompatibility, ProviderCapability, ProviderMetadata, ProviderStatus,
        SORLA_PROVIDER_CONTRACT_VERSION,
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
}
