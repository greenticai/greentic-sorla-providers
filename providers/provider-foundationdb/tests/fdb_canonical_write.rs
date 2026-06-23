#![cfg(feature = "foundationdb-real")]
//! Requires FDB_CLUSTER_FILE=/home/bima-pangestu/fdb/fdb.cluster

use provider_foundationdb::{
    apply_canonical_write_fdb, boot_network, connect, read_event_stream_fdb,
};
use sorla_provider_core::{
    CanonicalEntityRecord, CanonicalWriteRequest, EntityRef, SorEventRecord, SorNamespace,
};

fn unique_ns(salt: u128) -> SorNamespace {
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

fn write_req(ns: &SorNamespace, seq: u64, idem: Option<&str>) -> CanonicalWriteRequest {
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
            idempotency_key: idem.map(ToString::to_string),
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
fn canonical_write_durability_and_guards() {
    let _net = boot_network();
    let rt = connect(None).expect("connect");

    // Scenario 1: atomic write then read
    let ns1 = unique_ns(0);
    apply_canonical_write_fdb(&rt, &write_req(&ns1, 1, None)).expect("write 1");
    let events = read_event_stream_fdb(&rt, &ns1, "Contract/c-1", 0, 100).expect("read");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].sequence, 1);

    // Scenario 2: duplicate idempotency key rejected, does not persist a 2nd event
    let ns2 = unique_ns(1);
    apply_canonical_write_fdb(&rt, &write_req(&ns2, 1, Some("idem-1"))).expect("first");
    let dup = apply_canonical_write_fdb(&rt, &write_req(&ns2, 2, Some("idem-1")));
    assert!(matches!(
        dup,
        Err(sorla_provider_core::ProviderError::Validation(_))
    ));
    assert_eq!(
        read_event_stream_fdb(&rt, &ns2, "Contract/c-1", 0, 100)
            .expect("read")
            .len(),
        1
    );

    // Scenario 3: out-of-order sequence rejected
    let ns3 = unique_ns(2);
    apply_canonical_write_fdb(&rt, &write_req(&ns3, 1, None)).expect("seq 1");
    let gap = apply_canonical_write_fdb(&rt, &write_req(&ns3, 3, None));
    assert!(matches!(
        gap,
        Err(sorla_provider_core::ProviderError::Validation(_))
    ));
}
