//! Confirms the in-memory default path compiles without the FDB feature,
//! and that the feature name is spelled exactly `foundationdb-real`.

#[test]
fn default_build_uses_in_memory_backend() {
    let provider = provider_foundationdb::FoundationDbProvider::for_tests();
    // metadata() must work with no cluster present
    let meta = sorla_provider_core::ProviderMetadataSource::metadata(&provider);
    assert_eq!(meta.provider_id, "greentic.sorla.provider.foundationdb");
}

#[cfg(feature = "foundationdb-real")]
#[test]
fn real_feature_symbol_exists() {
    // Compile-only: the real backend module must be reachable behind the gate.
    let _ = provider_foundationdb::fdb_real_backend_available();
}
