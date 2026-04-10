use semver::VersionReq;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Shared error model for provider contract implementations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProviderError {
    #[error("validation error: {0}")]
    Validation(String),
    #[error("provider capability not supported: {0}")]
    Unsupported(&'static str),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

/// Stable SoRLa provider capability list for lock-phase contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderCapability {
    EventAppend,
    EventStreamRead,
    ProjectionGet,
    ProjectionPut,
    ProjectionRebuild,
    ProjectionCheckpoint,
    ExternalReferenceResolve,
    EvidenceQuery,
    EvidenceResolve,
    HealthCheck,
    ConfigValidate,
    PackMetadataEmit,
}

/// Lifecycle status for a provider implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderStatus {
    Experimental,
    Stable,
    Deprecated,
}

/// Cross-repo compatibility markers shared across contracts, packs, and catalog entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractCompatibility {
    pub supported_provider_contract_version: String,
    pub supported_sorla_ir: String,
    pub supported_sorla_ir_range: String,
}

impl ContractCompatibility {
    pub fn new(
        supported_provider_contract_version: impl Into<String>,
        supported_sorla_ir: impl Into<String>,
        supported_sorla_ir_range: impl Into<String>,
    ) -> Self {
        Self {
            supported_provider_contract_version: supported_provider_contract_version.into(),
            supported_sorla_ir: supported_sorla_ir.into(),
            supported_sorla_ir_range: supported_sorla_ir_range.into(),
        }
    }

    pub fn parses_semver_range(&self) -> bool {
        VersionReq::parse(&self.supported_sorla_ir_range).is_ok()
    }
}

/// Canonical shared provider metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderMetadata {
    pub provider_id: String,
    pub display_name: String,
    pub provider_kind: String,
    pub version: String,
    pub status: ProviderStatus,
    pub is_mock: bool,
    pub capabilities: Vec<ProviderCapability>,
    pub compatibility: ContractCompatibility,
}

impl ProviderMetadata {
    pub fn supports(&self, capability: ProviderCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

/// Health state for provider checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthState {
    Ready,
    Degraded,
    Unavailable,
}

/// Provider health payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthReport {
    pub state: HealthState,
    pub message: String,
}

/// Request to append a single immutable event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppendEventRequest {
    pub stream_id: String,
    pub event_type: String,
    pub payload: String,
    pub expected_revision: Option<u64>,
}

/// Immutable event record returned by event-native providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    pub stream_id: String,
    pub revision: u64,
    pub event_type: String,
    pub payload: String,
}

/// Request for stream reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventStreamRequest {
    pub stream_id: String,
    pub from_revision: u64,
    pub limit: usize,
}

/// Projection snapshot record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionRecord {
    pub projection_name: String,
    pub projection_key: String,
    pub state_json: String,
    pub last_applied_revision: u64,
}

/// Request to persist a projection snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistProjectionRequest {
    pub projection_name: String,
    pub projection_key: String,
    pub state_json: String,
    pub last_applied_revision: u64,
}

/// Projection checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionCheckpoint {
    pub projection_name: String,
    pub checkpoint_token: String,
}

/// Request to rebuild a projection from a checkpoint or full replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionRebuildRequest {
    pub projection_name: String,
    pub from_checkpoint: Option<String>,
}

/// Request to resolve an external source-of-record reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalReferenceRequest {
    pub reference_type: String,
    pub reference_id: String,
    pub building_id: Option<String>,
    pub floor_id: Option<String>,
}

/// Resolved external record payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalReferencePayload {
    pub record_id: String,
    pub source_url: String,
    pub content_json: String,
}

/// Query filters for evidence retrieval.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceQueryFilter {
    pub building_id: Option<String>,
    pub floor_id: Option<String>,
    pub document_type: Option<String>,
    pub source_type: Option<String>,
}

/// Query for evidence lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceQuery {
    pub query: String,
    pub filter: EvidenceQueryFilter,
    pub limit: usize,
}

/// Evidence/citation-like result structure locked for consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub evidence_id: String,
    pub source_type: String,
    pub source_ref: String,
    pub document_id: String,
    pub section_id: Option<String>,
    pub page: Option<u32>,
    pub chunk_id: String,
    pub snippet: String,
    pub score: f32,
    pub provenance: String,
    pub metadata_json: String,
}

/// Minimal pack emission hook input shared by provider implementations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackEmission {
    pub provider_id: String,
    pub artifact_ref: String,
}
