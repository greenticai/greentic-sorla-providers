#![cfg(feature = "foundationdb-real")]
//! Requires FDB_CLUSTER_FILE=/home/bima-pangestu/fdb/fdb.cluster

use provider_foundationdb::{boot_network, connect};

#[test]
fn connects_and_runs_a_trivial_future() {
    // Boot once; the guard drops at the end of this fn -> clean network stop.
    let _net = boot_network();
    let rt = connect(None).expect("connect to local cluster");
    let answer = rt.block_on(async { 1 + 1 });
    assert_eq!(answer, 2);
}
