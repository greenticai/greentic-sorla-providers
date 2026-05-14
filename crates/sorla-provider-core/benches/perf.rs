use criterion::{Criterion, criterion_group, criterion_main};
use sorla_provider_core::{ContractCompatibility, SORLA_PROVIDER_CONTRACT_VERSION};
use sorla_provider_core::{ProviderCapability, ProviderMetadata, ProviderStatus};

fn bench_metadata_support_lookup(c: &mut Criterion) {
    let metadata = ProviderMetadata {
        provider_id: "greentic.sorla.provider.core.bench".into(),
        display_name: "Benchmark".into(),
        provider_kind: "mock".into(),
        version: "0.1.0".into(),
        status: ProviderStatus::Experimental,
        is_mock: true,
        capabilities: vec![
            ProviderCapability::HealthCheck,
            ProviderCapability::ConfigValidate,
            ProviderCapability::ExternalReferenceResolve,
            ProviderCapability::EvidenceQuery,
            ProviderCapability::PackMetadataEmit,
        ],
        compatibility: ContractCompatibility::new(
            SORLA_PROVIDER_CONTRACT_VERSION,
            "0.1",
            ">=0.1, <0.2",
        ),
        ontology_capabilities: None,
    };

    c.bench_function("provider_metadata_supports_lookup", |b| {
        b.iter(|| {
            let _ = metadata.supports(ProviderCapability::EvidenceQuery);
        })
    });
}

criterion_group!(benches, bench_metadata_support_lookup);
criterion_main!(benches);
