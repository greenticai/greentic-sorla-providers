//! Real FoundationDB backend modules (codec is cluster-free; runtime/keyspace/txn
//! are split out in later tasks).
pub mod codec;
pub mod keyspace;
#[cfg(feature = "foundationdb-real")]
pub mod runtime;
#[cfg(feature = "foundationdb-real")]
pub mod txn;
