use std::time::{Duration, Instant};

use sorla_provider_core::{ContractCompatibility, SORLA_PROVIDER_CONTRACT_VERSION};
use sorla_provider_core::{ProviderCapability, ProviderMetadata, ProviderStatus};

fn run_workload(threads: usize) -> Duration {
    let start = Instant::now();

    let handles: Vec<_> = (0..threads)
        .map(|idx| {
            std::thread::spawn(move || {
                let metadata = ProviderMetadata {
                    provider_id: format!("provider-{idx}"),
                    display_name: format!("Provider {idx}"),
                    provider_kind: "mock".into(),
                    version: "0.1.0".into(),
                    status: ProviderStatus::Experimental,
                    is_mock: true,
                    capabilities: vec![
                        ProviderCapability::HealthCheck,
                        ProviderCapability::ConfigValidate,
                        ProviderCapability::PackMetadataEmit,
                    ],
                    compatibility: ContractCompatibility::new(
                        SORLA_PROVIDER_CONTRACT_VERSION,
                        "0.1",
                        ">=0.1, <0.2",
                    ),
                };

                for _ in 0..50_000 {
                    assert!(metadata.supports(ProviderCapability::HealthCheck));
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("worker should complete");
    }

    start.elapsed()
}

#[test]
fn scaling_should_not_degrade_badly() {
    let t1 = run_workload(1);
    let t4 = run_workload(4);
    let t8 = run_workload(8);

    assert!(
        t4 <= t1.mul_f64(20.0),
        "unexpected slowdown: t1={t1:?} t4={t4:?}"
    );
    assert!(
        t8 <= t4.mul_f64(4.0),
        "unexpected slowdown: t4={t4:?} t8={t8:?}"
    );
}
