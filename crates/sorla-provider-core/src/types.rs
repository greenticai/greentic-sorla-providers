use semver::VersionReq;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    CanonicalState,
    CanonicalWrite,
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
    ExactIndex,
    CompositeIndex,
    RelationshipRead,
    RelationshipQuery,
    PathFind,
    EntityLink,
    SemanticAliasResolve,
    ExternalMappingValidate,
    OntologyScopedEvidenceQuery,
    HybridEvidenceQuery,
    PolicyContextResolve,
    TextSearchProjection,
    VectorSearchProjection,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_capabilities: Option<ProviderIndexCapabilities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_capabilities: Option<ProviderSearchCapabilities>,
}

/// Structured index support metadata for provider discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderIndexCapabilities {
    pub exact: bool,
    pub composite: bool,
}

/// Whether a projection is unavailable, optional, or required for a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectionSupport {
    Unavailable,
    Optional,
    Required,
}

/// Structured search projection support metadata for provider discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSearchCapabilities {
    pub text_projection: ProjectionSupport,
    pub vector_projection: ProjectionSupport,
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

/// Production source-of-record namespace for canonical SORX state.
///
/// Durable production records are scoped by `tenant_id + sor_id`. `environment_id`
/// may be used to isolate local, test, or staging state, but it must not replace
/// the production source-of-record boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SorNamespace {
    pub tenant_id: String,
    pub sor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_id: Option<String>,
}

impl SorNamespace {
    pub fn production_key(&self) -> String {
        format!("{}\u{1f}{}", self.tenant_id, self.sor_id)
    }

    pub fn to_entity_namespace(&self) -> String {
        match self.environment_id.as_deref() {
            Some(environment_id) => {
                format!("{}/{}/{}", self.tenant_id, self.sor_id, environment_id)
            }
            None => format!("{}/{}", self.tenant_id, self.sor_id),
        }
    }
}

/// Canonical persisted entity record for SORX state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalEntityRecord {
    pub namespace: SorNamespace,
    pub entity_type: String,
    pub entity_id: String,
    pub canonical_version: String,
    pub revision: u64,
    pub data_json: Value,
    pub created_at: String,
    pub updated_at: String,
}

impl CanonicalEntityRecord {
    pub fn entity_ref(&self) -> EntityRef {
        EntityRef {
            entity_type: self.entity_type.clone(),
            entity_id: self.entity_id.clone(),
            namespace: Some(self.namespace.to_entity_namespace()),
            version: Some(self.canonical_version.clone()),
        }
    }
}

/// Immutable event record for canonical SORX streams.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SorEventRecord {
    pub namespace: SorNamespace,
    pub event_id: String,
    pub stream_id: String,
    pub sequence: u64,
    pub event_type: String,
    pub entity_ref: EntityRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_view_version: Option<String>,
    pub canonical_version: String,
    pub payload_json: Value,
    pub timestamp: String,
}

/// Canonical write request that must be applied atomically by durable providers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalWriteRequest {
    pub event: SorEventRecord,
    pub entity: CanonicalEntityRecord,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<RelationshipInstance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_links: Vec<EntityLink>,
}

/// Result returned after an atomic canonical write.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalWriteResult {
    pub event: SorEventRecord,
    pub entity: CanonicalEntityRecord,
    pub relationships_written: usize,
    pub entity_links_written: usize,
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
