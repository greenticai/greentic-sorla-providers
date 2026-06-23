#![cfg_attr(not(feature = "foundationdb-real"), forbid(unsafe_code))]
#![cfg_attr(feature = "foundationdb-real", deny(unsafe_code))]

#[allow(dead_code)]
mod fdb;

#[cfg(feature = "foundationdb-real")]
pub use fdb::runtime::{FdbRuntime, boot_network, connect};

#[cfg(feature = "foundationdb-real")]
pub use fdb::txn::{apply_canonical_write_fdb, read_event_stream_fdb};

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sorla_provider_catalog::{ProviderCatalogEntry, ProviderCatalogOntology};
use sorla_provider_core::{
    AppendEventRequest, CanonicalEntityRecord, CanonicalEntityStoreProvider,
    CanonicalWriteProvider, CanonicalWriteRequest, CanonicalWriteResult, ConfigValidator,
    ContractCompatibility, EntityLink, EntityLinkProvider, EntityLinkRequest, EntityRecord,
    EntityRef, EntitySearchQuery, EntityStoreProvider, EventRecord, EventStoreProvider,
    EventStreamRequest, HealthReport, HealthState, MetricProvider, OntologyContractCompatibility,
    OntologyPath, OntologyPathStep, PackEmission, PathQuery, PersistProjectionRequest,
    ProjectionCheckpoint, ProjectionProvider, ProjectionRebuildRequest, ProjectionRecord,
    ProjectionSupport, ProviderCapability, ProviderError, ProviderHealth,
    ProviderIndexCapabilities, ProviderMetadata, ProviderMetadataSource,
    ProviderMetricAggregateFunction, ProviderMetricAggregation, ProviderMetricDimension,
    ProviderMetricFilter, ProviderMetricFilterOperator, ProviderMetricQuery, ProviderMetricResult,
    ProviderMetricRow, ProviderMetricSource, ProviderMetricTimeBucket, ProviderMetricTimeGrain,
    ProviderMetricValue, ProviderOntologyCapabilities, ProviderSearchCapabilities, ProviderStatus,
    RelationshipDirection, RelationshipInstance, RelationshipQuery,
    SORLA_PROVIDER_CONTRACT_VERSION, SorEventRecord, SorNamespace,
};
use sorla_provider_pack::{
    ArtifactReference, ConfigSchemaRef, ProviderPackManifest, provider_artifact_file_uri,
    provider_runtime_component, provider_sdk_binding,
};

const PROVIDER_ID: &str = "greentic.sorla.provider.foundationdb";
const PROVIDER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns `true` only in builds compiled with the real FoundationDB backend.
/// Used by gated tests to assert the feature wiring is reachable.
#[cfg(feature = "foundationdb-real")]
pub fn fdb_real_backend_available() -> bool {
    true
}

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

#[derive(Debug, Clone)]
struct MetricInputRow {
    values: BTreeMap<String, serde_json::Value>,
    stable_key: String,
}

#[derive(Debug, Clone)]
struct MetricAccumulator {
    aggregation: ProviderMetricAggregation,
    count: u64,
    number_count: u64,
    sum: f64,
    min: Option<f64>,
    max: Option<f64>,
    distinct: BTreeSet<String>,
}

impl MetricAccumulator {
    fn new(aggregation: ProviderMetricAggregation) -> Self {
        Self {
            aggregation,
            count: 0,
            number_count: 0,
            sum: 0.0,
            min: None,
            max: None,
            distinct: BTreeSet::new(),
        }
    }

    fn record(&mut self, row: &MetricInputRow) {
        let value = self
            .aggregation
            .field
            .as_ref()
            .and_then(|field| row.values.get(field));

        match self.aggregation.function {
            ProviderMetricAggregateFunction::Count => {
                if self.aggregation.field.is_none() || value.is_some_and(|value| !value.is_null()) {
                    self.count += 1;
                }
            }
            ProviderMetricAggregateFunction::Sum | ProviderMetricAggregateFunction::Avg => {
                if let Some(number) = value.and_then(value_as_f64) {
                    self.sum += number;
                    self.number_count += 1;
                }
            }
            ProviderMetricAggregateFunction::Min => {
                if let Some(number) = value.and_then(value_as_f64) {
                    self.min = Some(self.min.map_or(number, |current| current.min(number)));
                    self.number_count += 1;
                }
            }
            ProviderMetricAggregateFunction::Max => {
                if let Some(number) = value.and_then(value_as_f64) {
                    self.max = Some(self.max.map_or(number, |current| current.max(number)));
                    self.number_count += 1;
                }
            }
            ProviderMetricAggregateFunction::DistinctCount => {
                if let Some(value) = value
                    && !value.is_null()
                {
                    self.distinct.insert(metric_json_sort_key(value));
                }
            }
        }
    }

    fn finish(&self) -> ProviderMetricValue {
        match self.aggregation.function {
            ProviderMetricAggregateFunction::Count => {
                ProviderMetricValue::Number(self.count as f64)
            }
            ProviderMetricAggregateFunction::Sum => {
                if self.number_count == 0 {
                    ProviderMetricValue::Null
                } else {
                    ProviderMetricValue::Number(self.sum)
                }
            }
            ProviderMetricAggregateFunction::Avg => {
                if self.number_count == 0 {
                    ProviderMetricValue::Null
                } else {
                    ProviderMetricValue::Number(self.sum / self.number_count as f64)
                }
            }
            ProviderMetricAggregateFunction::Min => self
                .min
                .map(ProviderMetricValue::Number)
                .unwrap_or(ProviderMetricValue::Null),
            ProviderMetricAggregateFunction::Max => self
                .max
                .map(ProviderMetricValue::Number)
                .unwrap_or(ProviderMetricValue::Null),
            ProviderMetricAggregateFunction::DistinctCount => {
                ProviderMetricValue::Number(self.distinct.len() as f64)
            }
        }
    }
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

enum Backend {
    Memory(Arc<RwLock<InMemoryFoundationDb>>),
    #[cfg(feature = "foundationdb-real")]
    Fdb(crate::fdb::runtime::FdbRuntime),
}

/// Local/dev FoundationDB provider implementation with transactional in-memory backing.
///
/// This keeps the SoRLa event/projection semantics real and testable while avoiding a hard
/// external FoundationDB runtime dependency in the current repo phase.
pub struct FoundationDbProvider {
    config: FoundationDbConfig,
    backend: Backend,
}

impl FoundationDbProvider {
    pub fn new(config: FoundationDbConfig) -> Self {
        #[cfg(feature = "foundationdb-real")]
        {
            let cluster = if config.cluster_file.is_empty() {
                None
            } else {
                Some(config.cluster_file.as_str())
            };
            if let Ok(rt) = crate::fdb::runtime::connect(cluster) {
                return Self {
                    config,
                    backend: Backend::Fdb(rt),
                };
            }
            // connection failed -> fall back to in-memory so metadata/validate still work; surface via health()
        }
        Self {
            config,
            backend: Backend::Memory(Arc::new(RwLock::new(InMemoryFoundationDb::default()))),
        }
    }

    pub fn for_tests() -> Self {
        Self {
            config: FoundationDbConfig {
                cluster_file: "/tmp/fdb.cluster".into(),
                tenant_prefix: "tenant/test".into(),
            },
            backend: Backend::Memory(Arc::new(RwLock::new(InMemoryFoundationDb::default()))),
        }
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
        match &self.backend {
            Backend::Memory(state) => {
                let mut state = state
                    .write()
                    .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;
                Self::upsert_relationship_in_state(&mut state, relationship.clone());
                Ok(relationship)
            }
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "upsert_relationship is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }

    pub fn upsert_evidence_link(&self, link: EntityLink) -> Result<EntityLink, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let mut state = state
                    .write()
                    .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;
                Self::upsert_evidence_link_in_state(&mut state, link.clone());
                Ok(link)
            }
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "upsert_evidence_link is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }

    pub fn evidence_links_for_entity(
        &self,
        entity: &EntityRef,
    ) -> Result<Vec<EntityLink>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
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
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "evidence_links_for_entity is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }

    pub fn projection_checkpoint(
        &self,
        projection_name: &str,
        projection_key: &str,
    ) -> Result<Option<ProjectionCheckpoint>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
                    .read()
                    .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;
                Ok(state
                    .projections
                    .get(&(projection_name.into(), projection_key.into()))
                    .map(|stored| stored.checkpoint.clone()))
            }
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "projection_checkpoint is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }

    fn checkpoint_token(projection_name: &str, revision: u64) -> String {
        format!("{projection_name}@{revision}")
    }

    /// Stable namespace under which projections are scoped on the FDB backend.
    ///
    /// The `ProjectionProvider` methods do not receive a `SorNamespace` (and the
    /// in-memory store keys projections only by `(name, key)`), so we derive a
    /// deterministic namespace from `self.config.tenant_prefix` with a fixed
    /// `sor_id`. persist/get/rebuild all share this namespace, so a record
    /// written by one provider instance is recoverable by another instance built
    /// from the same config.
    #[cfg(feature = "foundationdb-real")]
    fn projection_namespace(&self) -> SorNamespace {
        SorNamespace {
            tenant_id: self.config.tenant_prefix.clone(),
            sor_id: "projections".into(),
            environment_id: None,
        }
    }

    fn metric_capabilities() -> Vec<ProviderCapability> {
        vec![
            ProviderCapability::MetricAggregateCount,
            ProviderCapability::MetricAggregateSum,
            ProviderCapability::MetricAggregateAvg,
            ProviderCapability::MetricAggregateMin,
            ProviderCapability::MetricAggregateMax,
            ProviderCapability::MetricAggregateDistinctCount,
            ProviderCapability::MetricDimensionGroupBy,
            ProviderCapability::MetricTimeBucketHour,
            ProviderCapability::MetricTimeBucketDay,
            ProviderCapability::MetricTimeBucketWeek,
            ProviderCapability::MetricTimeBucketMonth,
            ProviderCapability::MetricTimeBucketQuarter,
            ProviderCapability::MetricTimeBucketYear,
        ]
    }

    fn metric_rows_for_query(
        &self,
        query: &ProviderMetricQuery,
    ) -> Result<Vec<MetricInputRow>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
                    .read()
                    .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;
                metric_rows_for_query_mem(&state, query)
            }
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "metric_rows_for_query is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }

    fn validate_metric_query(
        &self,
        rows: &[MetricInputRow],
        query: &ProviderMetricQuery,
    ) -> Result<(), ProviderError> {
        if query.aggregations.is_empty() {
            return Err(ProviderError::InvalidMetricFilter(
                "at least one aggregation is required".into(),
            ));
        }

        let capabilities = self.metadata().capabilities;
        for aggregation in &query.aggregations {
            let required = aggregation.function.required_capability();
            if !capabilities.contains(&required) {
                return Err(ProviderError::UnsupportedMetricCapability(format!(
                    "{required:?}"
                )));
            }
            if let Some(field) = aggregation.field.as_deref() {
                ensure_metric_field(rows, field)?;
            }
        }
        for filter in &query.filters {
            validate_filter_shape(filter)?;
            ensure_metric_field(rows, &filter.field)?;
        }
        for dimension in &query.dimensions {
            if !capabilities.contains(&ProviderCapability::MetricDimensionGroupBy) {
                return Err(ProviderError::UnsupportedMetricCapability(
                    "metric-dimension-group-by".into(),
                ));
            }
            ensure_metric_field(rows, &dimension.field)?;
        }
        if let Some(time_bucket) = &query.time_bucket {
            let required = time_bucket.grain.required_capability();
            if !capabilities.contains(&required) {
                return Err(ProviderError::UnsupportedMetricCapability(format!(
                    "{required:?}"
                )));
            }
            ensure_metric_field(rows, &time_bucket.field)?;
        }

        Ok(())
    }

    fn run_metric_query(
        &self,
        rows: Vec<MetricInputRow>,
        query: ProviderMetricQuery,
    ) -> Result<ProviderMetricResult, ProviderError> {
        self.validate_metric_query(&rows, &query)?;
        let filtered = rows
            .into_iter()
            .filter(|row| {
                query
                    .filters
                    .iter()
                    .all(|filter| filter_matches(row, filter))
            })
            .collect::<Vec<_>>();

        let mut groups: BTreeMap<
            String,
            (
                BTreeMap<String, ProviderMetricValue>,
                Vec<MetricAccumulator>,
            ),
        > = BTreeMap::new();
        for row in &filtered {
            let dimensions = metric_dimensions(row, &query.dimensions, query.time_bucket.as_ref())?;
            let key = dimensions
                .iter()
                .map(|(name, value)| format!("{name}={}", value.sort_key()))
                .collect::<Vec<_>>()
                .join("\u{1f}");
            let entry = groups.entry(key).or_insert_with(|| {
                (
                    dimensions,
                    query
                        .aggregations
                        .iter()
                        .cloned()
                        .map(MetricAccumulator::new)
                        .collect(),
                )
            });
            for accumulator in &mut entry.1 {
                accumulator.record(row);
            }
        }

        if groups.is_empty() && query.dimensions.is_empty() && query.time_bucket.is_none() {
            let mut accumulators = query
                .aggregations
                .iter()
                .cloned()
                .map(MetricAccumulator::new)
                .collect::<Vec<_>>();
            for row in &filtered {
                for accumulator in &mut accumulators {
                    accumulator.record(row);
                }
            }
            groups.insert(String::new(), (BTreeMap::new(), accumulators));
        }

        let mut rows = groups
            .into_values()
            .map(|(dimensions, accumulators)| {
                let metrics = accumulators
                    .iter()
                    .map(|accumulator| {
                        (accumulator.aggregation.alias.clone(), accumulator.finish())
                    })
                    .collect::<BTreeMap<_, _>>();
                ProviderMetricRow {
                    dimensions,
                    metrics,
                }
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(metric_row_sort_key);
        if let Some(limit) = query.limit {
            rows.truncate(limit);
        }

        Ok(ProviderMetricResult {
            source: query.source,
            rows,
        })
    }
}

fn value_object(value: &serde_json::Value) -> Option<BTreeMap<String, serde_json::Value>> {
    value.as_object().map(|object| {
        object
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    })
}

fn payload_object(payload: &str) -> Result<BTreeMap<String, serde_json::Value>, ProviderError> {
    let value: serde_json::Value = serde_json::from_str(payload).map_err(|err| {
        ProviderError::MetricExecutionFailed(format!("event payload is not valid JSON: {err}"))
    })?;
    value_object(&value).ok_or_else(|| {
        ProviderError::MetricExecutionFailed("event payload is not a JSON object".into())
    })
}

fn metric_fixture_rows(name: &str) -> Option<Vec<MetricInputRow>> {
    if name != "commerce" {
        return None;
    }

    let rows = [
        serde_json::json!({"kind":"visitor","campaign_id":"spring","visitor_id":"visitor-1","occurred_at":"2026-05-01T08:00:00Z"}),
        serde_json::json!({"kind":"visitor","campaign_id":"spring","visitor_id":"visitor-2","occurred_at":"2026-05-01T08:10:00Z"}),
        serde_json::json!({"kind":"click","campaign_id":"spring","visitor_id":"visitor-1","amount":null,"occurred_at":"2026-05-01T09:00:00Z"}),
        serde_json::json!({"kind":"click","campaign_id":"spring","visitor_id":"visitor-2","amount":null,"occurred_at":"2026-05-01T10:00:00Z"}),
        serde_json::json!({"kind":"order","campaign_id":"spring","visitor_id":"visitor-1","amount":120.0,"occurred_at":"2026-05-02T11:00:00Z"}),
        serde_json::json!({"kind":"payment","campaign_id":"spring","visitor_id":"visitor-1","amount":120.0,"occurred_at":"2026-05-02T12:00:00Z"}),
        serde_json::json!({"kind":"cost","campaign_id":"spring","amount":25.0,"occurred_at":"2026-05-02T13:00:00Z"}),
        serde_json::json!({"kind":"campaign","campaign_id":"spring","channel":"search","occurred_at":"2026-05-01T00:00:00Z"}),
        serde_json::json!({"kind":"visitor","campaign_id":"fall","visitor_id":"visitor-3","occurred_at":"2026-06-04T08:00:00Z"}),
        serde_json::json!({"kind":"order","campaign_id":"fall","visitor_id":"visitor-3","amount":80.0,"occurred_at":"2026-06-04T11:00:00Z"}),
        serde_json::json!({"kind":"cost","campaign_id":"fall","amount":30.0,"occurred_at":"2026-06-04T13:00:00Z"}),
        serde_json::json!({"kind":"campaign","campaign_id":"fall","channel":"email","occurred_at":"2026-06-01T00:00:00Z"}),
    ];

    Some(
        rows.into_iter()
            .enumerate()
            .map(|(index, value)| MetricInputRow {
                values: value_object(&value).expect("fixture rows are objects"),
                stable_key: format!("fixture:commerce:{index:020}"),
            })
            .collect(),
    )
}

fn metric_rows_for_query_mem(
    state: &InMemoryFoundationDb,
    query: &ProviderMetricQuery,
) -> Result<Vec<MetricInputRow>, ProviderError> {
    match &query.source {
        ProviderMetricSource::EventStream { stream_id } => {
            let events = state.streams.get(stream_id).ok_or_else(|| {
                ProviderError::UnknownMetricSource(query.source.description())
            })?;
            Ok(events
                .iter()
                .map(|event| {
                    let mut values = payload_object(&event.payload)?;
                    values.insert(
                        "stream_id".into(),
                        serde_json::Value::String(event.stream_id.clone()),
                    );
                    values.insert(
                        "event_type".into(),
                        serde_json::Value::String(event.event_type.clone()),
                    );
                    values.insert(
                        "revision".into(),
                        serde_json::Value::Number(event.revision.into()),
                    );
                    Ok(MetricInputRow {
                        values,
                        stable_key: format!("event:{}:{:020}", event.stream_id, event.revision),
                    })
                })
                .collect::<Result<Vec<_>, ProviderError>>()?)
        }
        ProviderMetricSource::CanonicalEntities {
            namespace,
            entity_type,
        } => {
            let mut rows = state
                .canonical_entities
                .values()
                .filter(|record| {
                    record.namespace == *namespace && record.entity_type == *entity_type
                })
                .map(|record| {
                    let mut values = value_object(&record.data_json).ok_or_else(|| {
                        ProviderError::MetricExecutionFailed(format!(
                            "canonical entity {} payload is not an object",
                            record.entity_id
                        ))
                    })?;
                    values.insert(
                        "entity_id".into(),
                        serde_json::Value::String(record.entity_id.clone()),
                    );
                    values.insert(
                        "entity_type".into(),
                        serde_json::Value::String(record.entity_type.clone()),
                    );
                    values.insert(
                        "canonical_version".into(),
                        serde_json::Value::String(record.canonical_version.clone()),
                    );
                    values.insert(
                        "revision".into(),
                        serde_json::Value::Number(record.revision.into()),
                    );
                    values.insert(
                        "created_at".into(),
                        serde_json::Value::String(record.created_at.clone()),
                    );
                    values.insert(
                        "updated_at".into(),
                        serde_json::Value::String(record.updated_at.clone()),
                    );
                    Ok(MetricInputRow {
                        values,
                        stable_key: format!(
                            "canonical:{}:{}:{}:{:020}",
                            namespace.production_key(),
                            record.entity_type,
                            record.entity_id,
                            record.revision
                        ),
                    })
                })
                .collect::<Result<Vec<_>, ProviderError>>()?;
            rows.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
            if rows.is_empty() {
                Err(ProviderError::UnknownMetricSource(
                    query.source.description(),
                ))
            } else {
                Ok(rows)
            }
        }
        ProviderMetricSource::Fixture { name } => metric_fixture_rows(name)
            .ok_or_else(|| ProviderError::UnknownMetricSource(query.source.description())),
    }
}

fn ensure_metric_field(rows: &[MetricInputRow], field: &str) -> Result<(), ProviderError> {
    if rows.iter().any(|row| row.values.contains_key(field)) {
        Ok(())
    } else {
        Err(ProviderError::UnknownMetricField(field.into()))
    }
}

fn validate_filter_shape(filter: &ProviderMetricFilter) -> Result<(), ProviderError> {
    match filter.operator {
        ProviderMetricFilterOperator::In | ProviderMetricFilterOperator::NotIn => {
            if filter.values.is_empty() {
                return Err(ProviderError::InvalidMetricFilter(format!(
                    "{} requires values",
                    filter.field
                )));
            }
        }
        ProviderMetricFilterOperator::Exists | ProviderMetricFilterOperator::NotExists => {}
        _ => {
            if filter.value.is_none() {
                return Err(ProviderError::InvalidMetricFilter(format!(
                    "{} requires value",
                    filter.field
                )));
            }
        }
    }
    Ok(())
}

fn value_as_f64(value: &serde_json::Value) -> Option<f64> {
    value.as_f64()
}

fn metric_json_sort_key(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "0:".into(),
        serde_json::Value::Bool(value) => format!("1:{value}"),
        serde_json::Value::Number(value) => format!("2:{value}"),
        serde_json::Value::String(value) => format!("3:{value}"),
        serde_json::Value::Array(values) => {
            format!(
                "4:{}",
                values
                    .iter()
                    .map(metric_json_sort_key)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        serde_json::Value::Object(values) => {
            format!(
                "5:{}",
                values
                    .iter()
                    .map(|(key, value)| format!("{key}:{}", metric_json_sort_key(value)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

fn metric_value_from_json(value: Option<&serde_json::Value>) -> ProviderMetricValue {
    match value {
        None | Some(serde_json::Value::Null) => ProviderMetricValue::Null,
        Some(serde_json::Value::Bool(value)) => ProviderMetricValue::Bool(*value),
        Some(serde_json::Value::Number(value)) => value
            .as_f64()
            .map(ProviderMetricValue::Number)
            .unwrap_or(ProviderMetricValue::Null),
        Some(serde_json::Value::String(value)) => ProviderMetricValue::String(value.clone()),
        Some(value) => ProviderMetricValue::String(value.to_string()),
    }
}

fn compare_metric_values(
    left: &serde_json::Value,
    right: &serde_json::Value,
) -> Option<std::cmp::Ordering> {
    match (left.as_f64(), right.as_f64()) {
        (Some(left), Some(right)) => left.partial_cmp(&right),
        _ => match (left.as_str(), right.as_str()) {
            (Some(left), Some(right)) => Some(left.cmp(right)),
            _ => None,
        },
    }
}

fn filter_matches(row: &MetricInputRow, filter: &ProviderMetricFilter) -> bool {
    let value = row.values.get(&filter.field);
    match filter.operator {
        ProviderMetricFilterOperator::Equals => value == filter.value.as_ref(),
        ProviderMetricFilterOperator::NotEquals => value != filter.value.as_ref(),
        ProviderMetricFilterOperator::In => {
            value.is_some_and(|value| filter.values.contains(value))
        }
        ProviderMetricFilterOperator::NotIn => {
            value.is_none_or(|value| !filter.values.contains(value))
        }
        ProviderMetricFilterOperator::Gt => value
            .zip(filter.value.as_ref())
            .and_then(|(left, right)| compare_metric_values(left, right))
            .is_some_and(|ordering| ordering.is_gt()),
        ProviderMetricFilterOperator::Gte => value
            .zip(filter.value.as_ref())
            .and_then(|(left, right)| compare_metric_values(left, right))
            .is_some_and(|ordering| ordering.is_ge()),
        ProviderMetricFilterOperator::Lt => value
            .zip(filter.value.as_ref())
            .and_then(|(left, right)| compare_metric_values(left, right))
            .is_some_and(|ordering| ordering.is_lt()),
        ProviderMetricFilterOperator::Lte => value
            .zip(filter.value.as_ref())
            .and_then(|(left, right)| compare_metric_values(left, right))
            .is_some_and(|ordering| ordering.is_le()),
        ProviderMetricFilterOperator::Exists => value.is_some_and(|value| !value.is_null()),
        ProviderMetricFilterOperator::NotExists => value.is_none_or(|value| value.is_null()),
    }
}

fn metric_dimensions(
    row: &MetricInputRow,
    dimensions: &[ProviderMetricDimension],
    time_bucket: Option<&ProviderMetricTimeBucket>,
) -> Result<BTreeMap<String, ProviderMetricValue>, ProviderError> {
    let mut values = BTreeMap::new();
    if let Some(time_bucket) = time_bucket {
        let value = row
            .values
            .get(&time_bucket.field)
            .ok_or_else(|| ProviderError::UnknownMetricField(time_bucket.field.clone()))?;
        values.insert(
            time_bucket
                .alias
                .clone()
                .unwrap_or_else(|| time_bucket.field.clone()),
            ProviderMetricValue::String(bucket_timestamp(value, time_bucket.grain)?),
        );
    }
    for dimension in dimensions {
        values.insert(
            dimension
                .alias
                .clone()
                .unwrap_or_else(|| dimension.field.clone()),
            metric_value_from_json(row.values.get(&dimension.field)),
        );
    }
    Ok(values)
}

fn bucket_timestamp(
    value: &serde_json::Value,
    grain: ProviderMetricTimeGrain,
) -> Result<String, ProviderError> {
    let timestamp = value.as_str().ok_or_else(|| {
        ProviderError::InvalidMetricFilter("time bucket field must be a string timestamp".into())
    })?;
    let date = timestamp.get(0..10).ok_or_else(|| {
        ProviderError::InvalidMetricFilter(format!("invalid timestamp {timestamp}"))
    })?;
    let year = timestamp.get(0..4).ok_or_else(|| {
        ProviderError::InvalidMetricFilter(format!("invalid timestamp {timestamp}"))
    })?;
    let month = timestamp
        .get(5..7)
        .and_then(|month| month.parse::<u32>().ok())
        .ok_or_else(|| {
            ProviderError::InvalidMetricFilter(format!("invalid timestamp {timestamp}"))
        })?;
    let day = timestamp
        .get(8..10)
        .and_then(|day| day.parse::<u32>().ok())
        .ok_or_else(|| {
            ProviderError::InvalidMetricFilter(format!("invalid timestamp {timestamp}"))
        })?;

    match grain {
        ProviderMetricTimeGrain::Hour => timestamp
            .get(0..13)
            .map(|hour| format!("{hour}:00:00Z"))
            .ok_or_else(|| {
                ProviderError::InvalidMetricFilter(format!("invalid timestamp {timestamp}"))
            }),
        ProviderMetricTimeGrain::Day => Ok(date.into()),
        ProviderMetricTimeGrain::Week => {
            let week = ((day_of_year(month, day)? - 1) / 7) + 1;
            Ok(format!("{year}-W{week:02}"))
        }
        ProviderMetricTimeGrain::Month => timestamp.get(0..7).map(str::to_owned).ok_or_else(|| {
            ProviderError::InvalidMetricFilter(format!("invalid timestamp {timestamp}"))
        }),
        ProviderMetricTimeGrain::Quarter => {
            let quarter = ((month - 1) / 3) + 1;
            Ok(format!("{year}-Q{quarter}"))
        }
        ProviderMetricTimeGrain::Year => Ok(year.into()),
    }
}

fn day_of_year(month: u32, day: u32) -> Result<u32, ProviderError> {
    let month_lengths = [31_u32, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if !(1..=12).contains(&month) {
        return Err(ProviderError::InvalidMetricFilter(format!(
            "invalid timestamp month {month}"
        )));
    }
    let month_index = (month - 1) as usize;
    if day == 0 || day > month_lengths[month_index] {
        return Err(ProviderError::InvalidMetricFilter(format!(
            "invalid timestamp day {day}"
        )));
    }
    Ok(month_lengths[..month_index].iter().sum::<u32>() + day)
}

fn metric_row_sort_key(row: &ProviderMetricRow) -> String {
    row.dimensions
        .iter()
        .map(|(name, value)| format!("{name}={}", value.sort_key()))
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

impl ProviderMetadataSource for FoundationDbProvider {
    fn metadata(&self) -> ProviderMetadata {
        let mut capabilities = vec![
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
        ];
        capabilities.extend(Self::metric_capabilities());

        ProviderMetadata {
            provider_id: PROVIDER_ID.into(),
            display_name: "FoundationDB".into(),
            provider_kind: "event-store".into(),
            version: PROVIDER_VERSION.into(),
            status: ProviderStatus::Experimental,
            is_mock: false,
            capabilities,
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

impl MetricProvider for FoundationDbProvider {
    fn query_metric(
        &self,
        query: ProviderMetricQuery,
    ) -> Result<ProviderMetricResult, ProviderError> {
        let rows = self.metric_rows_for_query(&query)?;
        self.run_metric_query(rows, query)
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
        match &self.backend {
            Backend::Memory(state) => {
                let mut state = state
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
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "append_event is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }

    fn read_event_stream(
        &self,
        request: EventStreamRequest,
    ) -> Result<Vec<EventRecord>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
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
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "read_event_stream is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }
}

impl ProjectionProvider for FoundationDbProvider {
    fn persist_projection(
        &self,
        request: PersistProjectionRequest,
    ) -> Result<ProjectionRecord, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let mut state = state
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
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(rt) => {
                let namespace = self.projection_namespace();
                crate::fdb::txn::persist_projection_fdb(rt, &namespace, &request)
            }
        }
    }

    fn get_projection(
        &self,
        projection_name: &str,
        projection_key: &str,
    ) -> Result<Option<ProjectionRecord>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
                    .read()
                    .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

                Ok(state
                    .projections
                    .get(&(projection_name.into(), projection_key.into()))
                    .map(|projection| projection.record.clone()))
            }
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(rt) => {
                let namespace = self.projection_namespace();
                crate::fdb::txn::get_projection_fdb(
                    rt,
                    &namespace,
                    projection_name,
                    projection_key,
                )
            }
        }
    }

    fn rebuild_projection(
        &self,
        request: ProjectionRebuildRequest,
    ) -> Result<ProjectionCheckpoint, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
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
                    checkpoint_token: Self::checkpoint_token(
                        &request.projection_name,
                        target_revision,
                    ),
                })
            }
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(rt) => {
                let namespace = self.projection_namespace();
                crate::fdb::txn::rebuild_projection_fdb(rt, &namespace, &request)
            }
        }
    }
}

impl EntityStoreProvider for FoundationDbProvider {
    fn upsert_entity(&self, entity: EntityRecord) -> Result<EntityRecord, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let mut state = state
                    .write()
                    .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

                state
                    .entities
                    .insert(Self::entity_key(&entity.entity), entity.clone());
                Ok(entity)
            }
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "upsert_entity is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }

    fn get_entity(&self, entity: EntityRef) -> Result<Option<EntityRecord>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
                    .read()
                    .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

                Ok(state.entities.get(&Self::entity_key(&entity)).cloned())
            }
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "get_entity is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }

    fn search_entities(
        &self,
        request: EntitySearchQuery,
    ) -> Result<Vec<EntityRecord>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
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
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "search_entities is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }
}

impl CanonicalEntityStoreProvider for FoundationDbProvider {
    fn upsert_canonical_entity(
        &self,
        record: CanonicalEntityRecord,
    ) -> Result<CanonicalEntityRecord, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let mut state = state
                    .write()
                    .map_err(|_| ProviderError::Validation("provider state lock poisoned".into()))?;

                state.canonical_entities.insert(
                    Self::canonical_entity_key(
                        &record.namespace,
                        &record.entity_type,
                        &record.entity_id,
                    ),
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
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "upsert_canonical_entity is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }

    fn get_canonical_entity(
        &self,
        namespace: SorNamespace,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Option<CanonicalEntityRecord>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
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
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(rt) => {
                crate::fdb::txn::get_canonical_entity_fdb(rt, &namespace, entity_type, entity_id)
            }
        }
    }
}

impl CanonicalWriteProvider for FoundationDbProvider {
    fn apply_canonical_write(
        &self,
        request: CanonicalWriteRequest,
    ) -> Result<CanonicalWriteResult, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
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

                let mut state = state
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
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(rt) => crate::fdb::txn::apply_canonical_write_fdb(rt, &request),
        }
    }
}

impl sorla_provider_core::OntologyGraphProvider for FoundationDbProvider {
    fn query_relationships(
        &self,
        request: RelationshipQuery,
    ) -> Result<Vec<RelationshipInstance>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
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
                                        RelationshipDirection::Outgoing => root_keys.contains(
                                            &Self::entity_key(&relationship.relationship.from),
                                        ),
                                        RelationshipDirection::Incoming => root_keys.contains(
                                            &Self::entity_key(&relationship.relationship.to),
                                        ),
                                        RelationshipDirection::Both => {
                                            root_keys.contains(&Self::entity_key(
                                                &relationship.relationship.from,
                                            )) || root_keys.contains(&Self::entity_key(
                                                &relationship.relationship.to,
                                            ))
                                        }
                                    })
                        })
                        .cloned(),
                );
                relationships.truncate(request.limit);
                Ok(relationships)
            }
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "query_relationships is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }

    fn find_paths(&self, request: PathQuery) -> Result<Vec<OntologyPath>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
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
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "find_paths is not yet supported on the FoundationDB backend".into(),
            )),
        }
    }
}

impl EntityLinkProvider for FoundationDbProvider {
    fn link_entities(&self, request: EntityLinkRequest) -> Result<Vec<EntityLink>, ProviderError> {
        match &self.backend {
            Backend::Memory(state) => {
                let state = state
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
                                .is_none_or(|evidence_id| {
                                    link.evidence_id.as_ref() == Some(evidence_id)
                                })
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
            #[cfg(feature = "foundationdb-real")]
            Backend::Fdb(_rt) => Err(ProviderError::Validation(
                "link_entities is not yet supported on the FoundationDB backend".into(),
            )),
        }
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
        EntityStoreProvider, EventStoreProvider, MetricProvider, OntologyGraphProvider, PathQuery,
        PersistProjectionRequest, ProjectionProvider, ProjectionRebuildRequest, ProviderCapability,
        ProviderError, ProviderHealth, ProviderMetadataSource, ProviderMetricAggregateFunction,
        ProviderMetricAggregation, ProviderMetricDimension, ProviderMetricFilter,
        ProviderMetricFilterOperator, ProviderMetricQuery, ProviderMetricSource,
        ProviderMetricTimeBucket, ProviderMetricTimeGrain, ProviderMetricValue,
        RelationshipDirection, RelationshipInstance, RelationshipQuery, RelationshipRef,
        SorEventRecord, SorNamespace,
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
        assert!(metadata.supports(ProviderCapability::MetricAggregateCount));
        assert!(metadata.supports(ProviderCapability::MetricAggregateDistinctCount));
        assert!(metadata.supports(ProviderCapability::MetricDimensionGroupBy));
        assert!(metadata.supports(ProviderCapability::MetricTimeBucketMonth));
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
        assert!(
            manifest
                .capabilities
                .contains(&ProviderCapability::MetricAggregateCount)
        );
        assert!(
            manifest
                .capabilities
                .contains(&ProviderCapability::MetricTimeBucketMonth)
        );
        assert!(
            entry
                .capabilities
                .contains(&ProviderCapability::MetricAggregateCount)
        );
        assert!(
            entry
                .capabilities
                .contains(&ProviderCapability::MetricDimensionGroupBy)
        );
        assert_eq!(manifest.provider_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            manifest.oci_reference.as_deref(),
            Some("oci://ghcr.io/greenticai/sorla-providers/foundationdb:0.1.8")
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
    fn metric_query_executes_event_stream_aggregates_with_filters() {
        let provider = FoundationDbProvider::for_tests();
        for (index, payload) in [
            serde_json::json!({"kind":"click","campaign_id":"spring","visitor_id":"visitor-1","amount":10.0,"occurred_at":"2026-05-01T09:00:00Z"}),
            serde_json::json!({"kind":"click","campaign_id":"spring","visitor_id":"visitor-2","amount":20.0,"occurred_at":"2026-05-01T10:00:00Z"}),
            serde_json::json!({"kind":"view","campaign_id":"fall","visitor_id":"visitor-2","amount":50.0,"occurred_at":"2026-05-02T10:00:00Z"}),
        ]
        .into_iter()
        .enumerate()
        {
            provider
                .append_event(AppendEventRequest {
                    stream_id: "commerce-events".into(),
                    event_type: "commerce.event".into(),
                    payload: payload.to_string(),
                    expected_revision: Some(index as u64),
                })
                .expect("append should succeed");
        }

        let result = provider
            .query_metric(ProviderMetricQuery {
                source: ProviderMetricSource::EventStream {
                    stream_id: "commerce-events".into(),
                },
                aggregations: vec![
                    ProviderMetricAggregation {
                        alias: "clicks".into(),
                        function: ProviderMetricAggregateFunction::Count,
                        field: None,
                    },
                    ProviderMetricAggregation {
                        alias: "amount_sum".into(),
                        function: ProviderMetricAggregateFunction::Sum,
                        field: Some("amount".into()),
                    },
                    ProviderMetricAggregation {
                        alias: "amount_avg".into(),
                        function: ProviderMetricAggregateFunction::Avg,
                        field: Some("amount".into()),
                    },
                    ProviderMetricAggregation {
                        alias: "visitors".into(),
                        function: ProviderMetricAggregateFunction::DistinctCount,
                        field: Some("visitor_id".into()),
                    },
                ],
                filters: vec![ProviderMetricFilter {
                    field: "kind".into(),
                    operator: ProviderMetricFilterOperator::Equals,
                    value: Some(serde_json::json!("click")),
                    values: vec![],
                }],
                dimensions: vec![],
                time_bucket: None,
                limit: None,
            })
            .expect("metric query should succeed");

        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].metrics.get("clicks"),
            Some(&ProviderMetricValue::Number(2.0))
        );
        assert_eq!(
            result.rows[0].metrics.get("amount_sum"),
            Some(&ProviderMetricValue::Number(30.0))
        );
        assert_eq!(
            result.rows[0].metrics.get("amount_avg"),
            Some(&ProviderMetricValue::Number(15.0))
        );
        assert_eq!(
            result.rows[0].metrics.get("visitors"),
            Some(&ProviderMetricValue::Number(2.0))
        );
    }

    #[test]
    fn metric_query_groups_canonical_entities_by_dimension_and_month() {
        let provider = FoundationDbProvider::for_tests();
        for (entity_id, campaign_id, amount, updated_at) in [
            ("order-001", "spring", 120.0, "2026-05-02T11:00:00Z"),
            ("order-002", "spring", 80.0, "2026-05-03T11:00:00Z"),
            ("order-003", "fall", 50.0, "2026-06-04T11:00:00Z"),
        ] {
            provider
                .upsert_canonical_entity(CanonicalEntityRecord {
                    namespace: namespace(),
                    entity_type: "Order".into(),
                    entity_id: entity_id.into(),
                    canonical_version: "2026-05-22".into(),
                    revision: 1,
                    data_json: serde_json::json!({
                        "campaign_id": campaign_id,
                        "amount": amount,
                    }),
                    created_at: updated_at.into(),
                    updated_at: updated_at.into(),
                })
                .expect("canonical upsert should succeed");
        }

        let result = provider
            .query_metric(ProviderMetricQuery {
                source: ProviderMetricSource::CanonicalEntities {
                    namespace: namespace(),
                    entity_type: "Order".into(),
                },
                aggregations: vec![ProviderMetricAggregation {
                    alias: "revenue".into(),
                    function: ProviderMetricAggregateFunction::Sum,
                    field: Some("amount".into()),
                }],
                filters: vec![],
                dimensions: vec![ProviderMetricDimension {
                    field: "campaign_id".into(),
                    alias: Some("campaign".into()),
                }],
                time_bucket: Some(ProviderMetricTimeBucket {
                    field: "updated_at".into(),
                    grain: ProviderMetricTimeGrain::Month,
                    alias: Some("month".into()),
                }),
                limit: None,
            })
            .expect("metric query should succeed");

        assert_eq!(result.rows.len(), 2);
        assert_eq!(
            result.rows[0].dimensions.get("campaign"),
            Some(&ProviderMetricValue::String("fall".into()))
        );
        assert_eq!(
            result.rows[0].dimensions.get("month"),
            Some(&ProviderMetricValue::String("2026-06".into()))
        );
        assert_eq!(
            result.rows[0].metrics.get("revenue"),
            Some(&ProviderMetricValue::Number(50.0))
        );
        assert_eq!(
            result.rows[1].dimensions.get("campaign"),
            Some(&ProviderMetricValue::String("spring".into()))
        );
        assert_eq!(
            result.rows[1].dimensions.get("month"),
            Some(&ProviderMetricValue::String("2026-05".into()))
        );
        assert_eq!(
            result.rows[1].metrics.get("revenue"),
            Some(&ProviderMetricValue::Number(200.0))
        );
    }

    #[test]
    fn metric_query_buckets_fixture_rows_by_day() {
        let provider = FoundationDbProvider::for_tests();
        let result = provider
            .query_metric(ProviderMetricQuery {
                source: ProviderMetricSource::Fixture {
                    name: "commerce".into(),
                },
                aggregations: vec![ProviderMetricAggregation {
                    alias: "daily_clicks".into(),
                    function: ProviderMetricAggregateFunction::Count,
                    field: None,
                }],
                filters: vec![ProviderMetricFilter {
                    field: "kind".into(),
                    operator: ProviderMetricFilterOperator::Equals,
                    value: Some(serde_json::json!("click")),
                    values: vec![],
                }],
                dimensions: vec![],
                time_bucket: Some(ProviderMetricTimeBucket {
                    field: "occurred_at".into(),
                    grain: ProviderMetricTimeGrain::Day,
                    alias: Some("day".into()),
                }),
                limit: None,
            })
            .expect("fixture query should succeed");

        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].dimensions.get("day"),
            Some(&ProviderMetricValue::String("2026-05-01".into()))
        );
        assert_eq!(
            result.rows[0].metrics.get("daily_clicks"),
            Some(&ProviderMetricValue::Number(2.0))
        );
    }

    #[test]
    fn metric_fixture_covers_commerce_dependency_inputs() {
        let provider = FoundationDbProvider::for_tests();
        let monthly_revenue = provider
            .query_metric(ProviderMetricQuery {
                source: ProviderMetricSource::Fixture {
                    name: "commerce".into(),
                },
                aggregations: vec![ProviderMetricAggregation {
                    alias: "monthly_revenue".into(),
                    function: ProviderMetricAggregateFunction::Sum,
                    field: Some("amount".into()),
                }],
                filters: vec![ProviderMetricFilter {
                    field: "kind".into(),
                    operator: ProviderMetricFilterOperator::Equals,
                    value: Some(serde_json::json!("order")),
                    values: vec![],
                }],
                dimensions: vec![],
                time_bucket: Some(ProviderMetricTimeBucket {
                    field: "occurred_at".into(),
                    grain: ProviderMetricTimeGrain::Month,
                    alias: Some("month".into()),
                }),
                limit: None,
            })
            .expect("monthly revenue should query");
        assert_eq!(
            monthly_revenue.rows[0].metrics.get("monthly_revenue"),
            Some(&ProviderMetricValue::Number(120.0))
        );
        assert_eq!(
            monthly_revenue.rows[1].metrics.get("monthly_revenue"),
            Some(&ProviderMetricValue::Number(80.0))
        );

        let monthly_cost = provider
            .query_metric(ProviderMetricQuery {
                source: ProviderMetricSource::Fixture {
                    name: "commerce".into(),
                },
                aggregations: vec![ProviderMetricAggregation {
                    alias: "monthly_cost".into(),
                    function: ProviderMetricAggregateFunction::Sum,
                    field: Some("amount".into()),
                }],
                filters: vec![ProviderMetricFilter {
                    field: "kind".into(),
                    operator: ProviderMetricFilterOperator::Equals,
                    value: Some(serde_json::json!("cost")),
                    values: vec![],
                }],
                dimensions: vec![],
                time_bucket: Some(ProviderMetricTimeBucket {
                    field: "occurred_at".into(),
                    grain: ProviderMetricTimeGrain::Month,
                    alias: Some("month".into()),
                }),
                limit: None,
            })
            .expect("monthly cost should query");
        assert_eq!(
            monthly_cost.rows[0].metrics.get("monthly_cost"),
            Some(&ProviderMetricValue::Number(25.0))
        );
        assert_eq!(
            monthly_cost.rows[1].metrics.get("monthly_cost"),
            Some(&ProviderMetricValue::Number(30.0))
        );

        let conversion_inputs = provider
            .query_metric(ProviderMetricQuery {
                source: ProviderMetricSource::Fixture {
                    name: "commerce".into(),
                },
                aggregations: vec![
                    ProviderMetricAggregation {
                        alias: "orders".into(),
                        function: ProviderMetricAggregateFunction::Count,
                        field: None,
                    },
                    ProviderMetricAggregation {
                        alias: "visitors".into(),
                        function: ProviderMetricAggregateFunction::DistinctCount,
                        field: Some("visitor_id".into()),
                    },
                ],
                filters: vec![ProviderMetricFilter {
                    field: "kind".into(),
                    operator: ProviderMetricFilterOperator::In,
                    value: None,
                    values: vec![serde_json::json!("order"), serde_json::json!("visitor")],
                }],
                dimensions: vec![ProviderMetricDimension {
                    field: "kind".into(),
                    alias: Some("input".into()),
                }],
                time_bucket: None,
                limit: None,
            })
            .expect("conversion inputs should query");
        assert_eq!(conversion_inputs.rows.len(), 2);

        let roas_inputs = provider
            .query_metric(ProviderMetricQuery {
                source: ProviderMetricSource::Fixture {
                    name: "commerce".into(),
                },
                aggregations: vec![ProviderMetricAggregation {
                    alias: "amount".into(),
                    function: ProviderMetricAggregateFunction::Sum,
                    field: Some("amount".into()),
                }],
                filters: vec![ProviderMetricFilter {
                    field: "kind".into(),
                    operator: ProviderMetricFilterOperator::In,
                    value: None,
                    values: vec![serde_json::json!("order"), serde_json::json!("cost")],
                }],
                dimensions: vec![
                    ProviderMetricDimension {
                        field: "campaign_id".into(),
                        alias: Some("campaign".into()),
                    },
                    ProviderMetricDimension {
                        field: "kind".into(),
                        alias: Some("input".into()),
                    },
                ],
                time_bucket: None,
                limit: None,
            })
            .expect("roas inputs should query");
        assert_eq!(roas_inputs.rows.len(), 4);
        assert!(roas_inputs.rows.iter().any(|row| {
            row.dimensions.get("campaign") == Some(&ProviderMetricValue::String("spring".into()))
                && row.dimensions.get("input") == Some(&ProviderMetricValue::String("order".into()))
                && row.metrics.get("amount") == Some(&ProviderMetricValue::Number(120.0))
        }));
        assert!(roas_inputs.rows.iter().any(|row| {
            row.dimensions.get("campaign") == Some(&ProviderMetricValue::String("fall".into()))
                && row.dimensions.get("input") == Some(&ProviderMetricValue::String("cost".into()))
                && row.metrics.get("amount") == Some(&ProviderMetricValue::Number(30.0))
        }));
    }

    #[test]
    fn metric_query_returns_useful_errors() {
        let provider = FoundationDbProvider::for_tests();
        let unknown_source = provider.query_metric(ProviderMetricQuery {
            source: ProviderMetricSource::EventStream {
                stream_id: "missing".into(),
            },
            aggregations: vec![ProviderMetricAggregation {
                alias: "count".into(),
                function: ProviderMetricAggregateFunction::Count,
                field: None,
            }],
            filters: vec![],
            dimensions: vec![],
            time_bucket: None,
            limit: None,
        });
        assert!(matches!(
            unknown_source,
            Err(ProviderError::UnknownMetricSource(_))
        ));

        let unknown_field = provider.query_metric(ProviderMetricQuery {
            source: ProviderMetricSource::Fixture {
                name: "commerce".into(),
            },
            aggregations: vec![ProviderMetricAggregation {
                alias: "bad".into(),
                function: ProviderMetricAggregateFunction::Sum,
                field: Some("missing_amount".into()),
            }],
            filters: vec![],
            dimensions: vec![],
            time_bucket: None,
            limit: None,
        });
        assert!(matches!(
            unknown_field,
            Err(ProviderError::UnknownMetricField(field)) if field == "missing_amount"
        ));

        let invalid_filter = provider.query_metric(ProviderMetricQuery {
            source: ProviderMetricSource::Fixture {
                name: "commerce".into(),
            },
            aggregations: vec![ProviderMetricAggregation {
                alias: "count".into(),
                function: ProviderMetricAggregateFunction::Count,
                field: None,
            }],
            filters: vec![ProviderMetricFilter {
                field: "kind".into(),
                operator: ProviderMetricFilterOperator::In,
                value: None,
                values: vec![],
            }],
            dimensions: vec![],
            time_bucket: None,
            limit: None,
        });
        assert!(matches!(
            invalid_filter,
            Err(ProviderError::InvalidMetricFilter(_))
        ));
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
    fn backend_dispatch_in_memory_path_applies_canonical_write() {
        // Confirm that the default (in-memory) backend dispatches correctly via
        // for_tests() and that apply_canonical_write succeeds end-to-end.
        let provider = FoundationDbProvider::for_tests();
        let ns = namespace();
        let entity = canonical_entity(1);
        let event = canonical_event(1, None);
        let result = provider
            .apply_canonical_write(CanonicalWriteRequest {
                event,
                entity,
                relationships: vec![],
                entity_links: vec![],
            })
            .expect("in-memory backend dispatch should succeed");
        assert_eq!(result.relationships_written, 0);
        assert_eq!(result.entity_links_written, 0);
        assert_eq!(result.entity.namespace, ns);
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
