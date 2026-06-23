#![cfg(feature = "foundationdb-real")]
//! Atomic canonical write + stream read transactions against a real FoundationDB
//! cluster. Every mutation of one canonical write (idempotency marker, stream
//! head bump, event, entity projection, relationship edges, entity links) is
//! applied inside a single `Database::run` transaction so the write is all-or-
//! nothing. Reads use a bounded half-open range scan over the stream subspace.

use std::error::Error;
use std::fmt;

use foundationdb::{FdbBindingError, RangeOption};
use sorla_provider_core::{
    CanonicalWriteRequest, CanonicalWriteResult, EntityRef, ProviderError, SorEventRecord,
    SorNamespace,
};

use super::codec::{decode_value, encode_value};
use super::keyspace::FdbKeyspace;
use super::runtime::FdbRuntime;

/// Signalled when a write reuses an already-applied idempotency key.
#[derive(Debug)]
struct DuplicateIdempotency(String);

impl fmt::Display for DuplicateIdempotency {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "idempotency key {} was already applied", self.0)
    }
}

impl Error for DuplicateIdempotency {}

/// Signalled when a write does not follow the current stream head sequence.
#[derive(Debug)]
struct SequenceConflict {
    expected: u64,
    actual: u64,
}

impl fmt::Display for SequenceConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "event sequence {} did not follow stream head sequence {} (expected {})",
            self.actual,
            self.expected.saturating_sub(1),
            self.expected
        )
    }
}

impl Error for SequenceConflict {}

/// Wraps a codec failure that surfaces inside a transaction closure.
#[derive(Debug)]
struct CodecError(ProviderError);

impl fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl Error for CodecError {}

/// Lift a domain error out of the transaction closure as a custom binding error.
fn custom_error<E: Error + Send + Sync + 'static>(err: E) -> FdbBindingError {
    FdbBindingError::new_custom_error(Box::new(err))
}

/// Encode big-endian so byte order matches numeric order for head keys.
fn encode_u64(value: u64) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

/// Decode a big-endian u64 head value, treating any non-8-byte slice as 0.
fn decode_u64(bytes: &[u8]) -> u64 {
    match <[u8; 8]>::try_from(bytes) {
        Ok(array) => u64::from_be_bytes(array),
        Err(_) => 0,
    }
}

/// Stable token identifying an entity inside edge keys: namespace, type, id,
/// version joined by unit separators (mirrors the in-memory provider layout).
fn entity_token(entity: &EntityRef) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
        entity.namespace.as_deref().unwrap_or_default(),
        entity.entity_type,
        entity.entity_id,
        entity.version.as_deref().unwrap_or_default()
    )
}

/// Map any error returned from `Database::run` to a provider validation error.
/// Domain guards (duplicate idempotency, sequence conflict) and codec failures
/// surface here through their `Display` text; genuine FDB transport/conflict
/// errors are also reported as validation failures so the durable contract
/// never leaks an `unwrap`/`panic`.
fn map_binding_error(err: FdbBindingError) -> ProviderError {
    ProviderError::Validation(err.to_string())
}

/// Apply one canonical write atomically against the real FoundationDB cluster.
///
/// All mutations run inside a single `Database::run` transaction, so either the
/// whole write commits or nothing is persisted. The closure may execute more
/// than once (FDB retries), so it re-derives all state from owned, cloned input.
pub fn apply_canonical_write_fdb(
    rt: &FdbRuntime,
    req: &CanonicalWriteRequest,
) -> Result<CanonicalWriteResult, ProviderError> {
    if req.event.namespace != req.entity.namespace {
        return Err(ProviderError::Validation(
            "event and entity namespaces must match".into(),
        ));
    }
    if req.event.entity_ref.entity_type != req.entity.entity_type
        || req.event.entity_ref.entity_id != req.entity.entity_id
    {
        return Err(ProviderError::Validation(
            "event entity_ref must target the canonical entity".into(),
        ));
    }

    let db = rt.database();
    rt.block_on(async {
        db.run(|trx, _maybe_committed| {
            // Clone owned inputs into the closure: `run` may retry it.
            let req = req.clone();
            async move {
                let ks = FdbKeyspace::new(&req.event.namespace);
                let event = &req.event;

                // 1. Idempotency guard: reject and persist nothing on replay.
                if let Some(idem) = event.idempotency_key.as_deref() {
                    let idem_key = ks.idempotency_key(idem);
                    if trx.get(&idem_key, false).await?.is_some() {
                        return Err(custom_error(DuplicateIdempotency(idem.to_string())));
                    }
                    trx.set(&idem_key, event.event_id.as_bytes());
                }

                // 2. Optimistic sequence guard: require sequence == head + 1.
                let head_key = ks.canonical_event_head_key(&event.stream_id);
                let current = match trx.get(&head_key, false).await? {
                    Some(bytes) => decode_u64(&bytes),
                    None => 0,
                };
                if event.sequence != current + 1 {
                    return Err(custom_error(SequenceConflict {
                        expected: current + 1,
                        actual: event.sequence,
                    }));
                }
                trx.set(&head_key, &encode_u64(event.sequence));

                // 3. Append the immutable event.
                let event_bytes =
                    encode_value(event).map_err(|err| custom_error(CodecError(err)))?;
                trx.set(
                    &ks.canonical_event_key(&event.stream_id, event.sequence),
                    &event_bytes,
                );

                // 4. Upsert the canonical entity projection.
                let entity_bytes =
                    encode_value(&req.entity).map_err(|err| custom_error(CodecError(err)))?;
                trx.set(
                    &ks.canonical_entity_key(&req.entity.entity_type, &req.entity.entity_id),
                    &entity_bytes,
                );

                // 5. Relationship edges (out + in indexes).
                for relationship in &req.relationships {
                    let rel = &relationship.relationship;
                    let from = entity_token(&rel.from);
                    let to = entity_token(&rel.to);
                    let edge_bytes =
                        encode_value(relationship).map_err(|err| custom_error(CodecError(err)))?;
                    trx.set(
                        &ks.edge_out_key(&rel.relationship_type, &from, &to),
                        &edge_bytes,
                    );
                    trx.set(
                        &ks.edge_in_key(&rel.relationship_type, &to, &from),
                        &edge_bytes,
                    );
                }

                // 6. Entity links, keyed by linked entity + source + match kind.
                for link in &req.entity_links {
                    let link_bytes =
                        encode_value(link).map_err(|err| custom_error(CodecError(err)))?;
                    trx.set(
                        &ks.entity_link_key(
                            &entity_token(&link.entity),
                            &link.source_ref,
                            &link.match_kind,
                        ),
                        &link_bytes,
                    );
                }

                Ok(())
            }
        })
        .await
    })
    .map_err(map_binding_error)?;

    Ok(CanonicalWriteResult {
        event: req.event.clone(),
        entity: req.entity.clone(),
        relationships_written: req.relationships.len(),
        entity_links_written: req.entity_links.len(),
    })
}

/// Read an event stream slice from the real cluster: events with
/// `sequence > from`, ordered by sequence, capped at `limit`.
pub fn read_event_stream_fdb(
    rt: &FdbRuntime,
    namespace: &SorNamespace,
    stream_id: &str,
    from: u64,
    limit: usize,
) -> Result<Vec<SorEventRecord>, ProviderError> {
    let db = rt.database();
    let ks = FdbKeyspace::new(namespace);
    let range = ks.canonical_event_stream_range(stream_id);

    rt.block_on(async {
        let trx = db
            .create_trx()
            .map_err(|err| ProviderError::Validation(format!("fdb create_trx failed: {err}")))?;
        let mut opt = RangeOption::from(range);
        opt.limit = Some(limit.max(1));
        let kvs = trx
            .get_range(&opt, 1, false)
            .await
            .map_err(|err| ProviderError::Validation(format!("fdb get_range failed: {err}")))?;

        let mut events = Vec::new();
        for kv in kvs.iter() {
            let record: SorEventRecord = decode_value(kv.value())?;
            if record.sequence > from {
                events.push(record);
            }
        }
        events.sort_by_key(|event| event.sequence);
        events.truncate(limit);
        Ok(events)
    })
}
