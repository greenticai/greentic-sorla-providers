use std::time::{Duration, Instant};

use sorla_provider_core::SORLA_PROVIDER_CONTRACT_VERSION;
use sorla_provider_core::{
    ContractCompatibility, ProviderCapability, ProviderMetadata, ProviderStatus,
};

#[test]
fn metadata_checks_finish_quickly() {
    let metadata = ProviderMetadata {
        provider_id: "greentic.sorla.provider.core.timeout".into(),
        display_name: "Timeout".into(),
        provider_kind: "mock".into(),
        version: "0.1.0".into(),
        status: ProviderStatus::Experimental,
        is_mock: true,
        capabilities: vec![ProviderCapability::PackMetadataEmit; 16],
        compatibility: ContractCompatibility::new(
            SORLA_PROVIDER_CONTRACT_VERSION,
            "0.1",
            ">=0.1, <0.2",
        ),
    };

    let start = Instant::now();
    let mut checks = 0usize;

    for _ in 0..250_000 {
        if metadata.supports(ProviderCapability::PackMetadataEmit) {
            checks += 1;
        }
    }

    let elapsed = start.elapsed();

    assert!(checks > 0);
    assert!(
        elapsed < Duration::from_secs(2),
        "metadata checks were too slow: {elapsed:?}"
    );
}
