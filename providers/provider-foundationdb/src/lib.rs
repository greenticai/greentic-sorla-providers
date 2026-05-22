#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sorla_provider_catalog::{ProviderCatalogEntry, ProviderCatalogOntology};
use sorla_provider_core::{
    AppendEventRequest, CanonicalEntityRecord, CanonicalEntityStoreProvider,
    CanonicalWriteProvider, CanonicalWriteRequest, CanonicalWriteResult, ConfigValidator,
    ContractCompatibility, EntityLink, EntityLinkProvider, EntityLinkRequest, EntityRecord,
    EntityRef, EntitySearchQuery, EntityStoreProvider, EventRecord, EventStoreProvider,
    EventStreamRequest, HealthReport, HealthState, OntologyContractCompatibility, OntologyPath,
    OntologyPathStep, PackEmission, PathQuery, PersistProjectionRequest, ProjectionCheckpoint,
    ProjectionProvider, ProjectionRebuildRequest, ProjectionRecord, ProjectionSupport,
    ProviderCapability, ProviderError, ProviderHealth, ProviderIndexCapabilities, ProviderMetadata,
    ProviderMetadataSource, ProviderOntologyCapabilities, ProviderSearchCapabilities,
    ProviderStatus, RelationshipDirection, RelationshipInstance, RelationshipQuery,
    SORLA_PROVIDER_CONTRACT_VERSION, SorEventRecord, SorNamespace,
};
use sorla_provider_pack::{
    ArtifactReference, ConfigSchemaRef, ProviderPackManifest, provider_artifact_file_uri,
    provider_runtime_component, provider_sdk_binding,
};

const PROVIDER_ID: &str = "greentic.sorla.provider.foundationdb";
const PROVIDER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn encode_key_segment(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbConfig {
    pub cluster_file: String,
    pub tenant_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyspaceLayout {
    pub events_prefix: String,
    pub projections_prefix: String,
    pub indexes_prefix: String,
    pub metadata_prefix: String,
    pub checkpoints_prefix: String,
    pub compatibility_prefix: String,
    pub ontology_model_prefix: String,
    pub entities_prefix: String,
    pub relationships_prefix: String,
    pub relationship_from_index_prefix: String,
    pub relationship_to_index_prefix: String,
    pub evidence_links_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalKeyspaceLayout {
    pub current_schema_key: String,
    pub canonical_version_key: String,
    pub active_views_prefix: String,
    pub entities_prefix: String,
    pub entity_versions_prefix: String,
    pub events_prefix: String,
    pub events_by_entity_prefix: String,
    pub events_by_time_prefix: String,
    pub edges_out_prefix: String,
    pub edges_in_prefix: String,
    pub indexes_prefix: String,
    pub composite_indexes_prefix: String,
    pub external_refs_prefix: String,
    pub external_refs_by_entity_prefix: String,
    pub evidence_prefix: String,
    pub evidence_by_entity_prefix: String,
    pub migrations_prefix: String,
    pub deployments_prefix: String,
}

#[derive(Debug, Clone)]
struct ProjectionState {
    record: ProjectionRecord,
    checkpoint: ProjectionCheckpoint,
}

#[derive(Debug, Default)]
struct InMemoryFoundationDb {
    streams: HashMap<String, Vec<EventRecord>>,
    canonical_streams: HashMap<String, Vec<SorEventRecord>>,
    canonical_entities: HashMap<String, CanonicalEntityRecord>,
    idempotency_keys: HashMap<String, String>,
    projections: HashMap<(String, String), ProjectionState>,
    entities: HashMap<String, EntityRecord>,
    relationships: Vec<RelationshipInstance>,
    evidence_links: HashMap<String, Vec<EntityLink>>,
}

impl InMemoryFoundationDb {
    fn last_stream_revision(&self, stream_id: &str) -> u64 {
        self.streams
            .get(stream_id)
            .and_then(|events| events.last())
            .map(|event| event.revision)
            .unwrap_or(0)
    }

    fn highest_revision(&self) -> u64 {
        self.streams
            .values()
            .flat_map(|items| items.iter().map(|event| event.revision))
            .max()
            .unwrap_or(0)
    }
}

/// Local/dev FoundationDB provider implementation with transactional in-memory backing.
///
/// This keeps the SoRLa event/projection semantics real and testable while avoiding a hard
/// external FoundationDB runtime dependency in the current repo phase.
pub struct FoundationDbProvider {
    config: FoundationDbConfig,
    state: Arc<RwLock<InMemoryFoundationDb>>,
}

impl FoundationDbProvider {
    pub fn new(config: FoundationDbConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(InMemoryFoundationDb::default())),
        }
    }

    pub fn for_tests() -> Self {
        Self::new(FoundationDbConfig {
            cluster_file: "/tmp/fdb.cluster".into(),
            tenant_prefix: "tenant/test".into(),
        })
    }

    pub fn keyspace_layout(&self) -> KeyspaceLayout {
        let prefix = self.config.tenant_prefix.trim_end_matches('/');
        KeyspaceLayout {
            events_prefix: format!("{prefix}/events"),
            projections_prefix: format!("{prefix}/projections"),
            indexes_prefix: format!("{prefix}/indexes"),
            metadata_prefix: format!("{prefix}/metadata"),
            checkpoints_prefix: format!("{prefix}/checkpoints"),
            compatibility_prefix: format!("{prefix}/compatibility"),
            ontology_model_prefix: format!("{prefix}/ontology/model"),
            entities_prefix: format!("{prefix}/entities"),
            relationships_prefix: format!("{prefix}/relationships"),
            relationship_from_index_prefix: format!("{prefix}/relationship-index/from"),
            relationship_to_index_prefix: format!("{prefix}/relationship-index/to"),
            evidence_links_prefix: format!("{prefix}/evidence-links"),
        }
    }

    pub fn canonical_keyspace_layout(namespace: &SorNamespace) -> CanonicalKeyspaceLayout {
        let prefix = format!(
            "/sorx/{}/{}",
            encode_key_segment(&namespace.tenant_id),
            encode_key_segment(&namespace.sor_id)
        );
        CanonicalKeyspaceLayout {
            current_schema_key: format!("{prefix}/meta/current_schema"),
            canonical_version_key: format!("{prefix}/meta/canonical_version"),
            active_views_prefix: format!("{prefix}/meta/active_views"),
            entities_prefix: format!("{prefix}/entities"),
            entity_versions_prefix: format!("{prefix}/entity_versions"),
            events_prefix: format!("{prefix}/events"),
            events_by_entity_prefix: format!("{prefix}/events_by_entity"),
            events_by_time_prefix: format!("{prefix}/events_by_time"),
            edges_out_prefix: format!("{prefix}/edges/out"),
            edges_in_prefix: format!("{prefix}/edges/in"),
            indexes_prefix: format!("{prefix}/indexes"),
            composite_indexes_prefix: format!("{prefix}/composite_indexes"),
            external_refs_prefix: format!("{prefix}/external_refs"),
            external_refs_by_entity_prefix: format!("{prefix}/external_refs_by_entity"),
            evidence_prefix: format!("{prefix}/evidence"),
            evidence_by_entity_prefix: format!("{prefix}/evidence_by_entity"),
            migrations_prefix: format!("{prefix}/migrations"),
            deployments_prefix: format!("{prefix}/deployments"),
        }
    }

    fn entity_key(entity: &EntityRef) -> String {
        format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            entity.namespace.as_deref().unwrap_or_default(),
            entity.entity_type,
            entity.entity_id,
            entity.version.as_deref().unwrap_or_default()
        )
    }

    fn canonical_entity_key(
        namespace: &SorNamespace,
        entity_type: &str,
        entity_id: &str,
    ) -> String {
        format!(
            "{}\u{1f}{}\u{1f}{}",
            namespace.production_key(),
            entity_type,
            entity_id
        )
    }

    fn idempotency_key(namespace: &SorNamespace, idempotency_key: &str) -> String {
        format!("{}\u{1f}{}", namespace.production_key(), idempotency_key)
    }

    fn relationship_key(relationship: &RelationshipInstance) -> String {
        let rel = &relationship.relationship;
        format!(
            "{}\u{1f}{}\u{1f}{}",
            rel.relationship_type,
            Self::entity_key(&rel.from),
            Self::entity_key(&rel.to)
        )
    }

    fn sorted_relationships(
        relationships: impl IntoIterator<Item = RelationshipInstance>,
    ) -> Vec<RelationshipInstance> {
        let mut relationships = relationships.into_iter().collect::<Vec<_>>();
        relationships.sort_by_key(Self::relationship_key);
        relationships
    }

    fn upsert_relationship_in_state(
        state: &mut InMemoryFoundationDb,
        relationship: RelationshipInstance,
    ) {
        let key = Self::relationship_key(&relationship);
        if let Some(existing) = state
            .relationships
            .iter_mut()
            .find(|stored| Self::relationship_key(stored) == key)
        {
            *existing = relationship;
        } else {
            state.relationships.push(relationship);
        }
    }

    fn upsert_evidence_link_in_state(state: &mut InMemoryFoundationDb, link: EntityLink) {
        let entity_key = Self::entity_key(&link.entity);
        let links = state.evidence_links.entry(entity_key).or_default();
        let link_key = (
            link.source_ref.clone(),
            link.evidence_id.clone(),
            link.match_kind.clone(),
        );
        if let Some(existing) = links.iter_mut().find(|stored| {
            (
                stored.source_ref.clone(),
                stored.evidence_id.clone(),
                stored.match_kind.clone(),
            ) == link_key
        }) {
            *existing = link;
        } else {
            links.push(link);
        }
    }

    pub fn upsert_relationship(
        &self,
        relationship: RelationshipInstance,
    ) -> Result<RelationshipInstance, ProviderError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        Self::upsert_relationship_in_state(&mut state, relationship.clone());

        Ok(relationship)
    }

    pub fn upsert_evidence_link(&self, link: EntityLink) -> Result<EntityLink, ProviderError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        Self::upsert_evidence_link_in_state(&mut state, link.clone());

        Ok(link)
    }

    pub fn evidence_links_for_entity(
        &self,
        entity: &EntityRef,
    ) -> Result<Vec<EntityLink>, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;
        let mut links = state
            .evidence_links
            .get(&Self::entity_key(entity))
            .cloned()
            .unwrap_or_default();
        links.sort_by(|left, right| {
            left.source_ref
                .cmp(&right.source_ref)
                .then_with(|| left.evidence_id.cmp(&right.evidence_id))
                .then_with(|| left.match_kind.cmp(&right.match_kind))
        });
        Ok(links)
    }

    pub fn projection_checkpoint(
        &self,
        projection_name: &str,
        projection_key: &str,
    ) -> Result<Option<ProjectionCheckpoint>, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;
        Ok(state
            .projections
            .get(&(projection_name.into(), projection_key.into()))
            .map(|stored| stored.checkpoint.clone()))
    }

    fn checkpoint_token(projection_name: &str, revision: u64) -> String {
        format!("{projection_name}@{revision}")
    }
}

impl ProviderMetadataSource for FoundationDbProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            provider_id: PROVIDER_ID.into(),
            display_name: "FoundationDB".into(),
            provider_kind: "event-store".into(),
            version: PROVIDER_VERSION.into(),
            status: ProviderStatus::Experimental,
            is_mock: false,
            capabilities: vec![
                ProviderCapability::EventAppend,
                ProviderCapability::EventStreamRead,
                ProviderCapability::CanonicalState,
                ProviderCapability::CanonicalWrite,
                ProviderCapability::ProjectionGet,
                ProviderCapability::ProjectionPut,
                ProviderCapability::ProjectionRebuild,
                ProviderCapability::ProjectionCheckpoint,
                ProviderCapability::EntityRead,
                ProviderCapability::EntitySearch,
                ProviderCapability::RelationshipRead,
                ProviderCapability::RelationshipQuery,
                ProviderCapability::PathFind,
                ProviderCapability::EntityLink,
                ProviderCapability::HealthCheck,
                ProviderCapability::ConfigValidate,
                ProviderCapability::PackMetadataEmit,
            ],
            compatibility: ContractCompatibility::new(
                SORLA_PROVIDER_CONTRACT_VERSION,
                "0.1",
                ">=0.1, <0.2",
            ),
            ontology_capabilities: Some(ProviderOntologyCapabilities {
                schema: "greentic.sorla.provider.ontology-capabilities.v1".into(),
                compatibility: OntologyContractCompatibility {
                    supported_ontology_schema: "greentic.sorla.ontology.graph.v1".into(),
                    supported_ontology_schema_range: ">=1.0.0, <2.0.0".into(),
                    supported_retrieval_binding_schema: None,
                    supported_external_mapping_schema: None,
                },
                supports_entity_read: true,
                supports_entity_search: true,
                supports_relationship_query: true,
                supports_path_find: true,
                supports_entity_linking: true,
                supports_ontology_scoped_evidence: false,
                supported_concept_types: vec!["*".into()],
                supported_relationship_types: vec!["*".into()],
                max_traversal_depth: Some(8),
                supports_policy_context: false,
                index_capabilities: Some(ProviderIndexCapabilities {
                    exact: false,
                    composite: false,
                }),
                search_capabilities: Some(ProviderSearchCapabilities {
                    text_projection: ProjectionSupport::Unavailable,
                    vector_projection: ProjectionSupport::Unavailable,
                }),
            }),
        }
    }

    fn pack_emission(&self) -> PackEmission {
        PackEmission {
            provider_id: self.metadata().provider_id,
            artifact_ref: "file://generated/provider-foundationdb.gtpack".into(),
        }
    }
}

impl ProviderHealth for FoundationDbProvider {
    fn health(&self) -> Result<HealthReport, ProviderError> {
        Ok(HealthReport {
            state: HealthState::Ready,
            message: format!(
                "FoundationDB local/dev provider is ready for tenant prefix {}",
                self.config.tenant_prefix
            ),
        })
    }
}

impl ConfigValidator for FoundationDbProvider {
    fn validate_config(&self, config_json: &str) -> Result<(), ProviderError> {
        let parsed: FoundationDbConfig = serde_json::from_str(config_json)
            .map_err(|err| ProviderError::Validation(format!("invalid config JSON: {err}")))?;

        if parsed.cluster_file.trim().is_empty() {
            return Err(ProviderError::Validation(
                "cluster_file must not be empty".into(),
            ));
        }
        if parsed.tenant_prefix.trim().is_empty() {
            return Err(ProviderError::Validation(
                "tenant_prefix must not be empty".into(),
            ));
        }

        Ok(())
    }
}

impl EventStoreProvider for FoundationDbProvider {
    fn append_event(&self, request: AppendEventRequest) -> Result<EventRecord, ProviderError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        let last_revision = state.last_stream_revision(&request.stream_id);
        if let Some(expected) = request.expected_revision
            && expected != last_revision
        {
            return Err(ProviderError::Validation(format!(
                "expected revision {expected} did not match stream revision {last_revision}"
            )));
        }

        let record = EventRecord {
            stream_id: request.stream_id.clone(),
            revision: last_revision + 1,
            event_type: request.event_type,
            payload: request.payload,
        };

        state
            .streams
            .entry(request.stream_id)
            .or_default()
            .push(record.clone());

        Ok(record)
    }

    fn read_event_stream(
        &self,
        request: EventStreamRequest,
    ) -> Result<Vec<EventRecord>, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        Ok(state
            .streams
            .get(&request.stream_id)
            .into_iter()
            .flat_map(|events| events.iter())
            .filter(|event| event.revision >= request.from_revision)
            .take(request.limit)
            .cloned()
            .collect())
    }
}

impl ProjectionProvider for FoundationDbProvider {
    fn persist_projection(
        &self,
        request: PersistProjectionRequest,
    ) -> Result<ProjectionRecord, ProviderError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        let record = ProjectionRecord {
            projection_name: request.projection_name.clone(),
            projection_key: request.projection_key.clone(),
            state_json: request.state_json,
            last_applied_revision: request.last_applied_revision,
        };
        let checkpoint = ProjectionCheckpoint {
            projection_name: request.projection_name.clone(),
            checkpoint_token: Self::checkpoint_token(
                &request.projection_name,
                request.last_applied_revision,
            ),
        };

        state.projections.insert(
            (request.projection_name, request.projection_key),
            ProjectionState {
                record: record.clone(),
                checkpoint,
            },
        );

        Ok(record)
    }

    fn get_projection(
        &self,
        projection_name: &str,
        projection_key: &str,
    ) -> Result<Option<ProjectionRecord>, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        Ok(state
            .projections
            .get(&(projection_name.into(), projection_key.into()))
            .map(|projection| projection.record.clone()))
    }

    fn rebuild_projection(
        &self,
        request: ProjectionRebuildRequest,
    ) -> Result<ProjectionCheckpoint, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        let target_revision = match request.from_checkpoint.as_deref() {
            Some(token) => token
                .rsplit_once('@')
                .and_then(|(_, revision)| revision.parse::<u64>().ok())
                .unwrap_or_else(|| state.highest_revision()),
            None => state.highest_revision(),
        };

        Ok(ProjectionCheckpoint {
            projection_name: request.projection_name.clone(),
            checkpoint_token: Self::checkpoint_token(&request.projection_name, target_revision),
        })
    }
}

impl EntityStoreProvider for FoundationDbProvider {
    fn upsert_entity(&self, entity: EntityRecord) -> Result<EntityRecord, ProviderError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        state
            .entities
            .insert(Self::entity_key(&entity.entity), entity.clone());
        Ok(entity)
    }

    fn get_entity(&self, entity: EntityRef) -> Result<Option<EntityRecord>, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        Ok(state.entities.get(&Self::entity_key(&entity)).cloned())
    }

    fn search_entities(
        &self,
        request: EntitySearchQuery,
    ) -> Result<Vec<EntityRecord>, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        let mut entities = state
            .entities
            .values()
            .filter(|record| {
                request.entity_types.is_empty()
                    || request
                        .entity_types
                        .iter()
                        .any(|entity_type| entity_type == &record.entity.entity_type)
            })
            .filter(|record| {
                request.namespace.is_none()
                    || record.entity.namespace.as_ref() == request.namespace.as_ref()
            })
            .filter(|record| {
                request.query.as_ref().is_none_or(|query| {
                    record.entity.entity_id.contains(query)
                        || record
                            .label
                            .as_ref()
                            .is_some_and(|label| label.contains(query))
                        || record
                            .metadata_json
                            .as_ref()
                            .is_some_and(|metadata| metadata.contains(query))
                })
            })
            .cloned()
            .collect::<Vec<_>>();

        entities.sort_by_key(|record| {
            (
                record.entity.entity_type.clone(),
                record.entity.entity_id.clone(),
                record.entity.namespace.clone(),
                record.entity.version.clone(),
            )
        });
        entities.truncate(request.limit);
        Ok(entities)
    }
}

impl CanonicalEntityStoreProvider for FoundationDbProvider {
    fn upsert_canonical_entity(
        &self,
        record: CanonicalEntityRecord,
    ) -> Result<CanonicalEntityRecord, ProviderError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        state.canonical_entities.insert(
            Self::canonical_entity_key(&record.namespace, &record.entity_type, &record.entity_id),
            record.clone(),
        );
        state.entities.insert(
            Self::entity_key(&record.entity_ref()),
            EntityRecord {
                entity: record.entity_ref(),
                label: None,
                metadata_json: Some(record.data_json.to_string()),
            },
        );

        Ok(record)
    }

    fn get_canonical_entity(
        &self,
        namespace: SorNamespace,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Option<CanonicalEntityRecord>, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        Ok(state
            .canonical_entities
            .get(&Self::canonical_entity_key(
                &namespace,
                entity_type,
                entity_id,
            ))
            .cloned())
    }
}

impl CanonicalWriteProvider for FoundationDbProvider {
    fn apply_canonical_write(
        &self,
        request: CanonicalWriteRequest,
    ) -> Result<CanonicalWriteResult, ProviderError> {
        if request.event.namespace != request.entity.namespace {
            return Err(ProviderError::Validation(
                "event and entity namespaces must match".into(),
            ));
        }
        if request.event.entity_ref.entity_type != request.entity.entity_type
            || request.event.entity_ref.entity_id != request.entity.entity_id
        {
            return Err(ProviderError::Validation(
                "event entity_ref must target the canonical entity".into(),
            ));
        }

        let mut state = self
            .state
            .write()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

        if let Some(idempotency_key) = request.event.idempotency_key.as_deref() {
            let key = Self::idempotency_key(&request.event.namespace, idempotency_key);
            if state.idempotency_keys.contains_key(&key) {
                return Err(ProviderError::Validation(format!(
                    "idempotency key {idempotency_key} was already applied"
                )));
            }
        }

        let last_sequence = state
            .canonical_streams
            .get(&request.event.stream_id)
            .and_then(|events| events.last())
            .map(|event| event.sequence)
            .unwrap_or(0);
        if request.event.sequence != last_sequence + 1 {
            return Err(ProviderError::Validation(format!(
                "event sequence {} did not follow stream sequence {last_sequence}",
                request.event.sequence
            )));
        }

        state
            .canonical_streams
            .entry(request.event.stream_id.clone())
            .or_default()
            .push(request.event.clone());
        state.canonical_entities.insert(
            Self::canonical_entity_key(
                &request.entity.namespace,
                &request.entity.entity_type,
                &request.entity.entity_id,
            ),
            request.entity.clone(),
        );
        state.entities.insert(
            Self::entity_key(&request.entity.entity_ref()),
            EntityRecord {
                entity: request.entity.entity_ref(),
                label: None,
                metadata_json: Some(request.entity.data_json.to_string()),
            },
        );
        for relationship in request.relationships.iter().cloned() {
            Self::upsert_relationship_in_state(&mut state, relationship);
        }
        for link in request.entity_links.iter().cloned() {
            Self::upsert_evidence_link_in_state(&mut state, link);
        }
        if let Some(idempotency_key) = request.event.idempotency_key.as_deref() {
            state.idempotency_keys.insert(
                Self::idempotency_key(&request.event.namespace, idempotency_key),
                request.event.event_id.clone(),
            );
        }

        Ok(CanonicalWriteResult {
            event: request.event,
            entity: request.entity,
            relationships_written: request.relationships.len(),
            entity_links_written: request.entity_links.len(),
        })
    }
}

impl sorla_provider_core::OntologyGraphProvider for FoundationDbProvider {
    fn query_relationships(
        &self,
        request: RelationshipQuery,
    ) -> Result<Vec<RelationshipInstance>, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;
        let root_keys = request
            .root_entities
            .iter()
            .map(Self::entity_key)
            .collect::<Vec<_>>();
        let mut relationships = Self::sorted_relationships(
            state
                .relationships
                .iter()
                .filter(|relationship| {
                    request
                        .relationship_type
                        .as_ref()
                        .is_none_or(|relationship_type| {
                            relationship.relationship.relationship_type == *relationship_type
                        })
                        && (root_keys.is_empty()
                            || match request.direction {
                                RelationshipDirection::Outgoing => root_keys
                                    .contains(&Self::entity_key(&relationship.relationship.from)),
                                RelationshipDirection::Incoming => root_keys
                                    .contains(&Self::entity_key(&relationship.relationship.to)),
                                RelationshipDirection::Both => {
                                    root_keys.contains(&Self::entity_key(
                                        &relationship.relationship.from,
                                    )) || root_keys
                                        .contains(&Self::entity_key(&relationship.relationship.to))
                                }
                            })
                })
                .cloned(),
        );
        relationships.truncate(request.limit);
        Ok(relationships)
    }

    fn find_paths(&self, request: PathQuery) -> Result<Vec<OntologyPath>, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;
        let relationships = Self::sorted_relationships(state.relationships.clone());
        let target_key = Self::entity_key(&request.to);
        let mut paths = Vec::new();
        let mut queue = vec![(request.from.clone(), Vec::<OntologyPathStep>::new())];

        while let Some((current, steps)) = queue.pop() {
            if paths.len() >= request.limit {
                break;
            }
            if steps.len() >= usize::from(request.max_depth) {
                continue;
            }

            let current_key = Self::entity_key(&current);
            for relationship in relationships.iter().filter(|relationship| {
                Self::entity_key(&relationship.relationship.from) == current_key
                    && (request.relationship_types.is_empty()
                        || request.relationship_types.iter().any(|relationship_type| {
                            relationship_type == &relationship.relationship.relationship_type
                        }))
            }) {
                let next = relationship.relationship.to.clone();
                let next_key = Self::entity_key(&next);
                if steps.iter().any(|step| {
                    Self::entity_key(&step.relationship.from) == next_key
                        || Self::entity_key(&step.relationship.to) == next_key
                }) || Self::entity_key(&request.from) == next_key
                {
                    continue;
                }

                let mut next_steps = steps.clone();
                next_steps.push(OntologyPathStep {
                    relationship: relationship.relationship.clone(),
                    direction: RelationshipDirection::Outgoing,
                });

                if next_key == target_key {
                    paths.push(OntologyPath {
                        start: request.from.clone(),
                        end: request.to.clone(),
                        steps: next_steps,
                    });
                } else {
                    queue.insert(0, (next, next_steps));
                }
            }
        }

        paths.sort_by_key(|path| {
            path.steps
                .iter()
                .map(|step| {
                    Self::relationship_key(&RelationshipInstance {
                        relationship: step.relationship.clone(),
                        metadata_json: None,
                        provenance: None,
                    })
                })
                .collect::<Vec<_>>()
        });
        Ok(paths)
    }
}

impl EntityLinkProvider for FoundationDbProvider {
    fn link_entities(&self, request: EntityLinkRequest) -> Result<Vec<EntityLink>, ProviderError> {
        let state = self
            .state
            .read()
            .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;
        let mut links = state
            .evidence_links
            .values()
            .flat_map(|items| items.iter())
            .filter(|link| {
                request
                    .source_ref
                    .as_ref()
                    .is_none_or(|source_ref| &link.source_ref == source_ref)
                    && request
                        .evidence_id
                        .as_ref()
                        .is_none_or(|evidence_id| link.evidence_id.as_ref() == Some(evidence_id))
                    && (request.candidate_types.is_empty()
                        || request
                            .candidate_types
                            .iter()
                            .any(|candidate_type| candidate_type == &link.entity.entity_type))
            })
            .cloned()
            .collect::<Vec<_>>();
        links.sort_by(|left, right| {
            Self::entity_key(&left.entity)
                .cmp(&Self::entity_key(&right.entity))
                .then_with(|| left.source_ref.cmp(&right.source_ref))
                .then_with(|| left.evidence_id.cmp(&right.evidence_id))
        });
        Ok(links)
    }
}

pub fn pack_manifest() -> ProviderPackManifest {
    let provider = FoundationDbProvider::for_tests();
    ProviderPackManifest::from_metadata(
        &provider.metadata(),
        vec![ArtifactReference {
            kind: "gtpack-json".into(),
            uri: provider_artifact_file_uri(PROVIDER_ID),
        }],
        vec![provider_runtime_component(
            PROVIDER_ID,
            PROVIDER_VERSION,
            "foundationdb-runtime",
            "provider-foundationdb",
        )],
        ConfigSchemaRef {
            format: "json-schema".into(),
            path: "schemas/provider-config.schema.json".into(),
            schema_json: r#"{"type":"object","required":["cluster_file","tenant_prefix"],"properties":{"cluster_file":{"type":"string"},"tenant_prefix":{"type":"string"}},"additionalProperties":false}"#.into(),
        },
    )
    .with_sdk_binding(provider_sdk_binding(
        "provider-foundationdb",
        "provider_foundationdb",
        SORLA_PROVIDER_CONTRACT_VERSION,
        "FoundationDbProvider::new",
    ))
}

pub fn catalog_entry() -> ProviderCatalogEntry {
    let manifest = pack_manifest();
    ProviderCatalogEntry {
        provider_id: manifest.provider_id,
        provider_version: manifest.provider_version,
        provider_kind: manifest.provider_kind,
        capabilities: manifest.capabilities,
        tags: vec!["event-store".into(), "real".into()],
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
        sdk_binding: manifest.sdk_binding,
        ontology: manifest.ontology_capabilities.as_ref().map(|capabilities| {
            ProviderCatalogOntology {
                capabilities: vec![
                    ProviderCapability::EntityRead,
                    ProviderCapability::EntitySearch,
                    ProviderCapability::RelationshipQuery,
                    ProviderCapability::PathFind,
                    ProviderCapability::EntityLink,
                ],
                max_traversal_depth: capabilities.max_traversal_depth,
                supports_generic_entity_refs: true,
                supported_ontology_schema: capabilities
                    .compatibility
                    .supported_ontology_schema
                    .clone(),
                supported_ontology_schema_range: capabilities
                    .compatibility
                    .supported_ontology_schema_range
                    .clone(),
                supported_retrieval_binding_schema: capabilities
                    .compatibility
                    .supported_retrieval_binding_schema
                    .clone(),
                supported_external_mapping_schema: capabilities
                    .compatibility
                    .supported_external_mapping_schema
                    .clone(),
                index_capabilities: capabilities.index_capabilities.clone(),
                search_capabilities: capabilities.search_capabilities.clone(),
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        FoundationDbConfig, FoundationDbProvider, catalog_entry, encode_key_segment, pack_manifest,
    };
    use sorla_provider_core::{
        AppendEventRequest, CanonicalEntityRecord, CanonicalEntityStoreProvider,
        CanonicalWriteProvider, CanonicalWriteRequest, ConfigValidator, EntityLink,
        EntityLinkProvider, EntityLinkRequest, EntityRecord, EntityRef, EntitySearchQuery,
        EntityStoreProvider, EventStoreProvider, OntologyGraphProvider, PathQuery,
        PersistProjectionRequest, ProjectionProvider, ProjectionRebuildRequest, ProviderCapability,
        ProviderHealth, ProviderMetadataSource, RelationshipDirection, RelationshipInstance,
        RelationshipQuery, RelationshipRef, SorEventRecord, SorNamespace,
    };

    fn entity(entity_type: &str, entity_id: &str) -> EntityRef {
        EntityRef {
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            namespace: Some("test".into()),
            version: None,
        }
    }

    fn relationship(
        relationship_type: &str,
        from: EntityRef,
        to: EntityRef,
    ) -> RelationshipInstance {
        RelationshipInstance {
            relationship: RelationshipRef {
                relationship_type: relationship_type.into(),
                from,
                to,
            },
            metadata_json: None,
            provenance: Some("test".into()),
        }
    }

    fn namespace() -> SorNamespace {
        SorNamespace {
            tenant_id: "tenant/acme".into(),
            sor_id: "contracts".into(),
            environment_id: None,
        }
    }

    fn canonical_entity(revision: u64) -> CanonicalEntityRecord {
        CanonicalEntityRecord {
            namespace: namespace(),
            entity_type: "Contract".into(),
            entity_id: "contract-001".into(),
            canonical_version: "2026-05-22".into(),
            revision,
            data_json: serde_json::json!({"status": "active", "revision": revision}),
            created_at: "2026-05-22T10:00:00Z".into(),
            updated_at: "2026-05-22T11:00:00Z".into(),
        }
    }

    fn canonical_event(sequence: u64, idempotency_key: Option<&str>) -> SorEventRecord {
        let entity = canonical_entity(sequence);
        SorEventRecord {
            namespace: entity.namespace.clone(),
            event_id: format!("evt-{sequence:03}"),
            stream_id: "Contract/contract-001".into(),
            sequence,
            event_type: "contract.updated".into(),
            entity_ref: entity.entity_ref(),
            command_id: Some(format!("cmd-{sequence:03}")),
            idempotency_key: idempotency_key.map(str::to_owned),
            actor: Some("test".into()),
            source_view_version: Some("view-v1".into()),
            canonical_version: entity.canonical_version,
            payload_json: serde_json::json!({"sequence": sequence}),
            timestamp: "2026-05-22T11:00:00Z".into(),
        }
    }

    #[test]
    fn foundationdb_provider_advertises_event_capabilities() {
        let provider = FoundationDbProvider::for_tests();
        let metadata = provider.metadata();
        assert!(metadata.supports(ProviderCapability::EventAppend));
        assert!(metadata.supports(ProviderCapability::ProjectionCheckpoint));
        assert!(metadata.supports(ProviderCapability::ProjectionPut));
        assert!(metadata.supports(ProviderCapability::EntityRead));
        assert!(metadata.supports(ProviderCapability::RelationshipQuery));
        assert!(metadata.supports(ProviderCapability::PathFind));
        assert!(
            metadata
                .ontology_capabilities
                .as_ref()
                .is_some_and(|capabilities| capabilities.supports_path_find)
        );
    }

    #[test]
    fn foundationdb_provider_reports_health_and_pack_metadata() {
        let provider = FoundationDbProvider::for_tests();
        assert!(provider.health().is_ok());
        assert!(
            provider
                .validate_config(
                    r#"{"cluster_file":"/tmp/fdb.cluster","tenant_prefix":"tenant/demo"}"#
                )
                .is_ok()
        );
        assert!(provider.validate_config("{}").is_err());

        let manifest = pack_manifest();
        let entry = catalog_entry();

        assert_eq!(manifest.provider_id, entry.provider_id);
        assert_eq!(manifest.provider_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            manifest.oci_reference.as_deref(),
            Some("oci://ghcr.io/greenticai/sorla-providers/foundationdb:0.1.4")
        );
        assert_eq!(
            provider.pack_emission().artifact_ref,
            "file://generated/provider-foundationdb.gtpack"
        );
        assert_eq!(manifest.runtime_components.len(), 1);
    }

    #[test]
    fn append_and_read_events_follow_expected_revision() {
        let provider = FoundationDbProvider::for_tests();

        let first = provider
            .append_event(AppendEventRequest {
                stream_id: "building-123".into(),
                event_type: "building.created".into(),
                payload: "{\"building_id\":\"building-123\"}".into(),
                expected_revision: Some(0),
            })
            .expect("append should succeed");

        let second = provider
            .append_event(AppendEventRequest {
                stream_id: "building-123".into(),
                event_type: "building.updated".into(),
                payload: "{\"title\":\"Tower A\"}".into(),
                expected_revision: Some(1),
            })
            .expect("second append should succeed");

        assert_eq!(first.revision, 1);
        assert_eq!(second.revision, 2);

        let events = provider
            .read_event_stream(sorla_provider_core::EventStreamRequest {
                stream_id: "building-123".into(),
                from_revision: 1,
                limit: 10,
            })
            .expect("stream read should succeed");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "building.created");
        assert_eq!(events[1].event_type, "building.updated");

        let conflict = provider.append_event(AppendEventRequest {
            stream_id: "building-123".into(),
            event_type: "building.conflict".into(),
            payload: "{}".into(),
            expected_revision: Some(0),
        });
        assert!(conflict.is_err());
    }

    #[test]
    fn projections_can_be_persisted_read_and_rebuilt() {
        let provider = FoundationDbProvider::for_tests();

        provider
            .append_event(AppendEventRequest {
                stream_id: "building-123".into(),
                event_type: "building.created".into(),
                payload: "{\"building_id\":\"building-123\"}".into(),
                expected_revision: Some(0),
            })
            .expect("append should succeed");
        provider
            .append_event(AppendEventRequest {
                stream_id: "building-123".into(),
                event_type: "building.checked".into(),
                payload: "{\"status\":\"ok\"}".into(),
                expected_revision: Some(1),
            })
            .expect("append should succeed");

        let projection = provider
            .persist_projection(PersistProjectionRequest {
                projection_name: "building-summary".into(),
                projection_key: "building-123".into(),
                state_json: "{\"status\":\"ok\"}".into(),
                last_applied_revision: 2,
            })
            .expect("projection persist should succeed");

        let stored = provider
            .get_projection("building-summary", "building-123")
            .expect("projection read should succeed")
            .expect("projection should exist");
        let checkpoint = provider
            .projection_checkpoint("building-summary", "building-123")
            .expect("checkpoint read should succeed")
            .expect("checkpoint should exist");
        let rebuilt = provider
            .rebuild_projection(ProjectionRebuildRequest {
                projection_name: "building-summary".into(),
                from_checkpoint: Some(checkpoint.checkpoint_token.clone()),
            })
            .expect("rebuild should succeed");

        assert_eq!(projection.last_applied_revision, 2);
        assert_eq!(stored.state_json, "{\"status\":\"ok\"}");
        assert_eq!(checkpoint.checkpoint_token, "building-summary@2");
        assert_eq!(rebuilt.checkpoint_token, "building-summary@2");
    }

    #[test]
    fn keyspace_layout_is_stable_and_documented() {
        let provider = FoundationDbProvider::new(FoundationDbConfig {
            cluster_file: "/etc/foundationdb/fdb.cluster".into(),
            tenant_prefix: "tenant/acme".into(),
        });

        let layout = provider.keyspace_layout();
        let prefixes = [
            layout.events_prefix,
            layout.projections_prefix,
            layout.indexes_prefix,
            layout.metadata_prefix,
            layout.checkpoints_prefix,
            layout.compatibility_prefix,
            layout.ontology_model_prefix,
            layout.entities_prefix,
            layout.relationships_prefix,
            layout.relationship_from_index_prefix,
            layout.relationship_to_index_prefix,
            layout.evidence_links_prefix,
        ];

        let unique = prefixes.iter().collect::<BTreeSet<_>>();
        assert_eq!(prefixes.len(), unique.len());
        assert!(
            prefixes
                .iter()
                .all(|prefix| prefix.starts_with("tenant/acme/"))
        );
    }

    #[test]
    fn canonical_keyspace_layout_uses_sorx_namespace_without_environment() {
        let prod = SorNamespace {
            tenant_id: "tenant/acme".into(),
            sor_id: "contracts".into(),
            environment_id: None,
        };
        let dev = SorNamespace {
            environment_id: Some("dev".into()),
            ..prod.clone()
        };

        assert_eq!(encode_key_segment("tenant/acme"), "tenant%2Facme");
        let prod_layout = FoundationDbProvider::canonical_keyspace_layout(&prod);
        let dev_layout = FoundationDbProvider::canonical_keyspace_layout(&dev);

        assert_eq!(prod_layout, dev_layout);
        assert_eq!(
            prod_layout.current_schema_key,
            "/sorx/tenant%2Facme/contracts/meta/current_schema"
        );
        assert_eq!(
            prod_layout.entity_versions_prefix,
            "/sorx/tenant%2Facme/contracts/entity_versions"
        );
        assert_eq!(
            prod_layout.deployments_prefix,
            "/sorx/tenant%2Facme/contracts/deployments"
        );
    }

    #[test]
    fn entities_can_be_inserted_read_and_searched() {
        let provider = FoundationDbProvider::for_tests();
        let customer = EntityRecord {
            entity: entity("Customer", "customer-001"),
            label: Some("Acme Customer".into()),
            metadata_json: Some(r#"{"segment":"enterprise"}"#.into()),
        };

        provider
            .upsert_entity(customer.clone())
            .expect("entity upsert should succeed");
        let stored = provider
            .get_entity(customer.entity.clone())
            .expect("entity read should succeed")
            .expect("entity should exist");
        let found = provider
            .search_entities(EntitySearchQuery {
                entity_types: vec!["Customer".into()],
                query: Some("Acme".into()),
                namespace: Some("test".into()),
                metadata_json: None,
                limit: 10,
            })
            .expect("entity search should succeed");

        assert_eq!(stored, customer);
        assert_eq!(found, vec![customer]);
    }

    #[test]
    fn canonical_entities_can_be_upserted_read_and_projected_to_generic_entities() {
        let provider = FoundationDbProvider::for_tests();
        let record = canonical_entity(1);

        provider
            .upsert_canonical_entity(record.clone())
            .expect("canonical upsert should succeed");
        let stored = provider
            .get_canonical_entity(
                record.namespace.clone(),
                &record.entity_type,
                &record.entity_id,
            )
            .expect("canonical read should succeed")
            .expect("canonical entity should exist");
        let generic = provider
            .get_entity(record.entity_ref())
            .expect("generic read should succeed")
            .expect("generic entity projection should exist");

        assert_eq!(stored, record);
        assert_eq!(generic.metadata_json, Some(record.data_json.to_string()));
    }

    #[test]
    fn canonical_write_atomically_appends_event_updates_projection_and_edges() {
        let provider = FoundationDbProvider::for_tests();
        let record = canonical_entity(1);
        let document = entity("EvidenceDocument", "doc-001");
        let relationship = relationship("supported_by", record.entity_ref(), document.clone());
        let link = EntityLink {
            entity: document.clone(),
            source_ref: "sharepoint://tenant/acme/document/doc-001".into(),
            evidence_id: Some("evidence-001".into()),
            confidence: 1.0,
            match_kind: "external-id".into(),
            provenance: "test".into(),
            metadata_json: None,
        };

        let result = provider
            .apply_canonical_write(CanonicalWriteRequest {
                event: canonical_event(1, Some("idem-001")),
                entity: record.clone(),
                relationships: vec![relationship.clone()],
                entity_links: vec![link.clone()],
            })
            .expect("canonical write should succeed");

        assert_eq!(result.relationships_written, 1);
        assert_eq!(result.entity_links_written, 1);
        assert_eq!(
            provider
                .get_canonical_entity(
                    record.namespace.clone(),
                    &record.entity_type,
                    &record.entity_id
                )
                .expect("read should succeed"),
            Some(record.clone())
        );
        assert_eq!(
            provider
                .query_relationships(RelationshipQuery {
                    root_entities: vec![document],
                    relationship_type: Some("supported_by".into()),
                    direction: RelationshipDirection::Incoming,
                    max_depth: Some(1),
                    limit: 10,
                })
                .expect("reverse edge lookup should succeed"),
            vec![relationship]
        );
        assert_eq!(
            provider
                .link_entities(EntityLinkRequest {
                    source_ref: Some(link.source_ref.clone()),
                    evidence_id: link.evidence_id.clone(),
                    content_json: None,
                    candidate_types: vec![],
                    ontology_scope: None,
                })
                .expect("link lookup should succeed"),
            vec![link]
        );
    }

    #[test]
    fn canonical_write_rejects_duplicate_idempotency_without_partial_projection() {
        let provider = FoundationDbProvider::for_tests();
        provider
            .apply_canonical_write(CanonicalWriteRequest {
                event: canonical_event(1, Some("idem-001")),
                entity: canonical_entity(1),
                relationships: vec![],
                entity_links: vec![],
            })
            .expect("first write should succeed");

        let duplicate = provider.apply_canonical_write(CanonicalWriteRequest {
            event: canonical_event(2, Some("idem-001")),
            entity: canonical_entity(2),
            relationships: vec![],
            entity_links: vec![],
        });
        let stored = provider
            .get_canonical_entity(namespace(), "Contract", "contract-001")
            .expect("canonical read should succeed")
            .expect("canonical entity should exist");

        assert!(duplicate.is_err());
        assert_eq!(stored.revision, 1);
    }

    #[test]
    fn relationships_query_by_direction_and_type() {
        let provider = FoundationDbProvider::for_tests();
        let customer = entity("Customer", "customer-001");
        let contract = entity("Contract", "contract-001");
        let asset = entity("Asset", "asset-001");

        provider
            .upsert_relationship(relationship(
                "has_contract",
                customer.clone(),
                contract.clone(),
            ))
            .expect("relationship upsert should succeed");
        provider
            .upsert_relationship(relationship("governs", contract.clone(), asset))
            .expect("relationship upsert should succeed");

        let outgoing = provider
            .query_relationships(RelationshipQuery {
                root_entities: vec![customer.clone()],
                relationship_type: Some("has_contract".into()),
                direction: RelationshipDirection::Outgoing,
                max_depth: Some(1),
                limit: 10,
            })
            .expect("outgoing query should succeed");
        let incoming = provider
            .query_relationships(RelationshipQuery {
                root_entities: vec![contract.clone()],
                relationship_type: Some("has_contract".into()),
                direction: RelationshipDirection::Incoming,
                max_depth: Some(1),
                limit: 10,
            })
            .expect("incoming query should succeed");

        assert_eq!(outgoing.len(), 1);
        assert_eq!(incoming.len(), 1);
        assert_eq!(outgoing[0], incoming[0]);
    }

    #[test]
    fn path_finding_is_bounded_cycle_safe_and_stable() {
        let provider = FoundationDbProvider::for_tests();
        let customer = entity("Customer", "customer-001");
        let contract = entity("Contract", "contract-001");
        let asset = entity("Asset", "asset-001");
        let evidence = entity("EvidenceDocument", "doc-001");

        for item in [
            relationship("has_contract", customer.clone(), contract.clone()),
            relationship("governs", contract.clone(), asset.clone()),
            relationship("supports", asset.clone(), evidence.clone()),
            relationship("cycles_to", asset.clone(), customer.clone()),
        ] {
            provider
                .upsert_relationship(item)
                .expect("relationship upsert should succeed");
        }

        let paths = provider
            .find_paths(PathQuery {
                from: customer.clone(),
                to: evidence.clone(),
                relationship_types: vec![],
                max_depth: 4,
                limit: 10,
            })
            .expect("path finding should succeed");
        let too_shallow = provider
            .find_paths(PathQuery {
                from: customer,
                to: evidence,
                relationship_types: vec![],
                max_depth: 2,
                limit: 10,
            })
            .expect("path finding should succeed");

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].steps.len(), 3);
        assert!(too_shallow.is_empty());
    }

    #[test]
    fn evidence_links_are_persisted_and_queryable() {
        let provider = FoundationDbProvider::for_tests();
        let document = entity("EvidenceDocument", "doc-001");
        let link = EntityLink {
            entity: document.clone(),
            source_ref: "sharepoint://tenant/demo/document/doc-001".into(),
            evidence_id: Some("evidence-001".into()),
            confidence: 1.0,
            match_kind: "external-id".into(),
            provenance: "test".into(),
            metadata_json: None,
        };

        provider
            .upsert_evidence_link(link.clone())
            .expect("link upsert should succeed");
        let by_entity = provider
            .evidence_links_for_entity(&document)
            .expect("links should query by entity");
        let by_request = provider
            .link_entities(EntityLinkRequest {
                source_ref: Some(link.source_ref.clone()),
                evidence_id: None,
                content_json: None,
                candidate_types: vec!["EvidenceDocument".into()],
                ontology_scope: None,
            })
            .expect("links should query by request");

        assert_eq!(by_entity, vec![link.clone()]);
        assert_eq!(by_request, vec![link]);
    }
}
