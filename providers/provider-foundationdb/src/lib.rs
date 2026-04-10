#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sorla_provider_catalog::ProviderCatalogEntry;
use sorla_provider_core::{
    AppendEventRequest, ConfigValidator, ContractCompatibility, EventRecord, EventStoreProvider,
    EventStreamRequest, HealthReport, HealthState, PackEmission, PersistProjectionRequest,
    ProjectionCheckpoint, ProjectionProvider, ProjectionRebuildRequest, ProjectionRecord,
    ProviderCapability, ProviderError, ProviderHealth, ProviderMetadata, ProviderMetadataSource,
    ProviderStatus, SORLA_PROVIDER_CONTRACT_VERSION,
};
use sorla_provider_pack::{
    ArtifactReference, ConfigSchemaRef, ProviderPackManifest, RuntimeComponentRef,
};

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
}

#[derive(Debug, Clone)]
struct ProjectionState {
    record: ProjectionRecord,
    checkpoint: ProjectionCheckpoint,
}

#[derive(Debug, Default)]
struct InMemoryFoundationDb {
    streams: HashMap<String, Vec<EventRecord>>,
    projections: HashMap<(String, String), ProjectionState>,
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
        }
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
                ProviderCapability::ProjectionRebuild,
                ProviderCapability::ProjectionCheckpoint,
                ProviderCapability::HealthCheck,
                ProviderCapability::ConfigValidate,
                ProviderCapability::PackMetadataEmit,
            ],
            compatibility: ContractCompatibility::new(
                SORLA_PROVIDER_CONTRACT_VERSION,
                "0.1",
                ">=0.1, <0.2",
            ),
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

pub fn pack_manifest() -> ProviderPackManifest {
    let provider = FoundationDbProvider::for_tests();
    ProviderPackManifest::from_metadata(
        &provider.metadata(),
        vec![ArtifactReference {
            kind: "gtpack-json".into(),
            uri: "./greentic-sorla-provider-foundationdb.gtpack.json".into(),
        }],
        vec![RuntimeComponentRef {
            component_id: "foundationdb-runtime".into(),
            kind: "service".into(),
            entrypoint: "provider-foundationdb".into(),
            artifact_uri:
                "oci://ghcr.io/greenticai/greentic-sorla-providers/provider-foundationdb:v0.1.0"
                    .into(),
        }],
        ConfigSchemaRef {
            format: "json-schema".into(),
            path: "schemas/provider-config.schema.json".into(),
            schema_json: r#"{"type":"object","required":["cluster_file","tenant_prefix"],"properties":{"cluster_file":{"type":"string"},"tenant_prefix":{"type":"string"}},"additionalProperties":false}"#.into(),
        },
    )
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
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{FoundationDbConfig, FoundationDbProvider, catalog_entry, pack_manifest};
    use sorla_provider_core::{
        AppendEventRequest, ConfigValidator, EventStoreProvider, PersistProjectionRequest,
        ProjectionProvider, ProjectionRebuildRequest, ProviderCapability, ProviderHealth,
        ProviderMetadataSource,
    };

    #[test]
    fn foundationdb_provider_advertises_event_capabilities() {
        let provider = FoundationDbProvider::for_tests();
        let metadata = provider.metadata();
        assert!(metadata.supports(ProviderCapability::EventAppend));
        assert!(metadata.supports(ProviderCapability::ProjectionCheckpoint));
        assert!(metadata.supports(ProviderCapability::ProjectionPut));
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
        ];

        let unique = prefixes.iter().collect::<BTreeSet<_>>();
        assert_eq!(prefixes.len(), unique.len());
        assert!(
            prefixes
                .iter()
                .all(|prefix| prefix.starts_with("tenant/acme/"))
        );
    }
}
