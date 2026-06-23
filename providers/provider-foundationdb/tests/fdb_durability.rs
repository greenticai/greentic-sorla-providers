#![cfg(feature = "foundationdb-real")]
//! Requires FDB_CLUSTER_FILE=/home/bima-pangestu/fdb/fdb.cluster
//!
//! Durability proof against the LIVE cluster: data written through one
//! `FoundationDbProvider` survives the provider being dropped and recreated
//! (the restart analogue) and projection checkpoints persist for rebuild.

use provider_foundationdb::{FoundationDbConfig, FoundationDbProvider, boot_network};
use sorla_provider_core::{
    CanonicalEntityRecord, CanonicalEntityStoreProvider, CanonicalWriteProvider,
    CanonicalWriteRequest, EntityRef, PersistProjectionRequest, ProjectionProvider,
    ProjectionRebuildRequest, SorEventRecord, SorNamespace,
};

/// Empty `cluster_file` makes `connect()` fall back to the `FDB_CLUSTER_FILE`
/// env var, so `FoundationDbProvider::new` takes the real Fdb backend. A
/// non-empty `tenant_prefix` keeps projection keys scoped to this test run.
fn cfg_with_cluster() -> FoundationDbConfig {
    let prefix = format!(
        "tenant-durability-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    );
    FoundationDbConfig {
        cluster_file: String::new(),
        tenant_prefix: prefix,
    }
}

fn ns(salt: u128) -> SorNamespace {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos()
        + salt;
    SorNamespace {
        tenant_id: format!("tenant-{n}"),
        sor_id: "contracts".into(),
        environment_id: None,
    }
}

fn write_req(ns: &SorNamespace, seq: u64) -> CanonicalWriteRequest {
    let entity_ref = EntityRef {
        entity_type: "Contract".into(),
        entity_id: "c-1".into(),
        namespace: Some(ns.to_entity_namespace()),
        version: Some("v1".into()),
    };
    CanonicalWriteRequest {
        event: SorEventRecord {
            namespace: ns.clone(),
            event_id: format!("evt-{seq}"),
            stream_id: "Contract/c-1".into(),
            sequence: seq,
            event_type: "contract.updated".into(),
            entity_ref: entity_ref.clone(),
            command_id: None,
            idempotency_key: None,
            actor: None,
            source_view_version: None,
            canonical_version: "v1".into(),
            payload_json: serde_json::json!({ "status": "active" }),
            timestamp: "2026-06-23T00:00:00Z".into(),
        },
        entity: CanonicalEntityRecord {
            namespace: ns.clone(),
            entity_type: "Contract".into(),
            entity_id: "c-1".into(),
            canonical_version: "v1".into(),
            revision: seq,
            data_json: serde_json::json!({ "status": "active" }),
            created_at: "2026-06-23T00:00:00Z".into(),
            updated_at: "2026-06-23T00:00:00Z".into(),
        },
        relationships: vec![],
        entity_links: vec![],
    }
}

#[test]
fn fdb_durability_survives_recreation_and_rebuild() {
    let _net = boot_network();

    // Scenario A: canonical entity survives provider recreation (restart analogue).
    // This only passes on an FDB-backed provider; an in-memory backend would lose
    // the entity on drop, so a green assertion is itself the proof.
    let n = ns(0);
    {
        let p = FoundationDbProvider::new(cfg_with_cluster());
        p.apply_canonical_write(write_req(&n, 1)).expect("write");
    } // provider dropped

    let p2 = FoundationDbProvider::new(cfg_with_cluster());
    let got = p2
        .get_canonical_entity(n.clone(), "Contract", "c-1")
        .expect("get");
    assert!(
        got.is_some(),
        "entity must survive provider recreation against the cluster"
    );
    let record = got.expect("entity present");
    assert_eq!(record.entity_id, "c-1");
    assert_eq!(record.revision, 1);

    // Scenario B: projection checkpoint persists for rebuild.
    let pname = format!(
        "Portfolio-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    );

    // Reuse one config so the persist and the rebuild scope to the same subspace.
    let cfg = cfg_with_cluster();
    {
        let p = FoundationDbProvider::new(cfg.clone());
        p.persist_projection(PersistProjectionRequest {
            projection_name: pname.clone(),
            projection_key: "all".into(),
            state_json: "{\"count\":3}".into(),
            last_applied_revision: 12,
        })
        .expect("persist");

        let stored = p
            .get_projection(&pname, "all")
            .expect("get projection")
            .expect("projection present after persist");
        assert_eq!(stored.last_applied_revision, 12);
        assert_eq!(stored.state_json, "{\"count\":3}");
    } // provider dropped

    let p3 = FoundationDbProvider::new(cfg);
    let checkpoint = p3
        .rebuild_projection(ProjectionRebuildRequest {
            projection_name: pname.clone(),
            from_checkpoint: None,
        })
        .expect("rebuild");
    assert!(
        checkpoint.checkpoint_token.contains("12"),
        "checkpoint token must carry last_applied_revision, got {}",
        checkpoint.checkpoint_token
    );
}
