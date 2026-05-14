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
    OntologyModelRead,
    EntityRead,
    EntitySearch,
    RelationshipRead,
    RelationshipQuery,
    PathFind,
    EntityLink,
    SemanticAliasResolve,
    ExternalMappingValidate,
    OntologyScopedEvidenceQuery,
    HybridEvidenceQuery,
    PolicyContextResolve,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ontology_capabilities: Option<ProviderOntologyCapabilities>,
}

impl ProviderMetadata {
    pub fn supports(&self, capability: ProviderCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

/// Optional ontology capability metadata for providers and generated packs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderOntologyCapabilities {
    pub schema: String,
    pub compatibility: OntologyContractCompatibility,
    pub supports_entity_read: bool,
    pub supports_entity_search: bool,
    pub supports_relationship_query: bool,
    pub supports_path_find: bool,
    pub supports_entity_linking: bool,
    pub supports_ontology_scoped_evidence: bool,
    pub supported_concept_types: Vec<String>,
    pub supported_relationship_types: Vec<String>,
    pub max_traversal_depth: Option<u8>,
    pub supports_policy_context: bool,
}

/// Compatibility gates for ontology-aware provider metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OntologyContractCompatibility {
    pub supported_ontology_schema: String,
    pub supported_ontology_schema_range: String,
    pub supported_retrieval_binding_schema: Option<String>,
    pub supported_external_mapping_schema: Option<String>,
}

impl OntologyContractCompatibility {
    pub fn parses_schema_range(&self) -> bool {
        VersionReq::parse(&self.supported_ontology_schema_range).is_ok()
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

/// Generic reference to an ontology entity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityRef {
    pub entity_type: String,
    pub entity_id: String,
    pub namespace: Option<String>,
    pub version: Option<String>,
}

/// Persisted or returned ontology entity payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRecord {
    pub entity: EntityRef,
    pub label: Option<String>,
    pub metadata_json: Option<String>,
}

/// Search request for generic ontology entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitySearchQuery {
    pub entity_types: Vec<String>,
    pub query: Option<String>,
    pub namespace: Option<String>,
    pub metadata_json: Option<String>,
    pub limit: usize,
}

/// Generic reference to an ontology relationship.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationshipRef {
    pub relationship_type: String,
    pub from: EntityRef,
    pub to: EntityRef,
}

/// Direction for relationship traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelationshipDirection {
    Incoming,
    Outgoing,
    Both,
}

/// Rule used when expanding an ontology scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipTraversalRule {
    pub relationship_type: Option<String>,
    pub direction: RelationshipDirection,
    pub max_depth: Option<u8>,
}

/// Scope used to bind evidence, graph traversal, and policy context to ontology entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OntologyScope {
    pub root_entities: Vec<EntityRef>,
    pub include_related: Vec<RelationshipTraversalRule>,
    pub max_depth: Option<u8>,
    pub include_evidence_links: bool,
}

/// Query for relationship instances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipQuery {
    pub root_entities: Vec<EntityRef>,
    pub relationship_type: Option<String>,
    pub direction: RelationshipDirection,
    pub max_depth: Option<u8>,
    pub limit: usize,
}

/// Generic relationship instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipInstance {
    pub relationship: RelationshipRef,
    pub metadata_json: Option<String>,
    pub provenance: Option<String>,
}

/// Query for deterministic bounded path finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathQuery {
    pub from: EntityRef,
    pub to: EntityRef,
    pub relationship_types: Vec<String>,
    pub max_depth: u8,
    pub limit: usize,
}

/// One step in an ontology path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OntologyPathStep {
    pub relationship: RelationshipRef,
    pub direction: RelationshipDirection,
}

/// Deterministic ontology path result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OntologyPath {
    pub start: EntityRef,
    pub end: EntityRef,
    pub steps: Vec<OntologyPathStep>,
}

/// Inclusive time range filter using provider-neutral timestamp strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: Option<String>,
    pub end: Option<String>,
}

/// Request to resolve an external source-of-record reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalReferenceRequest {
    pub reference_type: String,
    pub reference_id: String,
    pub source_ref: Option<String>,
    pub metadata_json: Option<String>,
    pub ontology_scope: Option<OntologyScope>,
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
    pub ontology_scope: Option<OntologyScope>,
    pub source_types: Vec<String>,
    pub document_types: Vec<String>,
    pub metadata_json: Option<String>,
    pub time_range: Option<TimeRange>,
    pub sensitivity_max: Option<String>,
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
    pub linked_entities: Vec<EntityLink>,
    pub relationship_context: Vec<RelationshipRef>,
    pub permissions_context_json: Option<String>,
}

/// Link from provider content or source refs to a generic ontology entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityLink {
    pub entity: EntityRef,
    pub source_ref: String,
    pub evidence_id: Option<String>,
    pub confidence: f32,
    pub match_kind: String,
    pub provenance: String,
    pub metadata_json: Option<String>,
}

/// Request to link provider content to ontology entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityLinkRequest {
    pub source_ref: Option<String>,
    pub evidence_id: Option<String>,
    pub content_json: Option<String>,
    pub candidate_types: Vec<String>,
    pub ontology_scope: Option<OntologyScope>,
}

/// Provider-neutral policy context lookup request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyContextRequest {
    pub ontology_scope: Option<OntologyScope>,
    pub subject_ref: Option<EntityRef>,
    pub action: String,
    pub metadata_json: Option<String>,
}

/// Provider-neutral policy context result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyContext {
    pub context_json: String,
    pub provenance: String,
}

/// Minimal pack emission hook input shared by provider implementations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackEmission {
    pub provider_id: String,
    pub artifact_ref: String,
}
