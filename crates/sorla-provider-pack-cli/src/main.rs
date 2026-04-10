#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::PathBuf;

use provider_foundationdb::pack_manifest as foundationdb_manifest;
use provider_rag_mock::pack_manifest as rag_manifest;
use provider_sharepoint_mock::pack_manifest as sharepoint_manifest;
use serde::Serialize;
use sorla_provider_pack::{GeneratedPackLayout, ProviderPackManifest, write_generated_pack};

#[derive(Debug, Serialize)]
pub struct GeneratedPackIndexEntry {
    provider_id: String,
    manifest_path: String,
    artifact_path: String,
    schema_path: String,
}

pub fn manifests() -> Vec<ProviderPackManifest> {
    let mut manifests = vec![
        foundationdb_manifest(),
        sharepoint_manifest(),
        rag_manifest(),
    ];
    manifests.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
    manifests
}

fn output_dir_from_args() -> PathBuf {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-dir"
            && let Some(path) = args.next()
        {
            return PathBuf::from(path);
        }
    }
    PathBuf::from("examples/generated-packs")
}

pub fn index_entry(
    layout: &GeneratedPackLayout,
    manifest: &ProviderPackManifest,
) -> GeneratedPackIndexEntry {
    GeneratedPackIndexEntry {
        provider_id: manifest.provider_id.clone(),
        manifest_path: layout.manifest_path.to_string_lossy().into_owned(),
        artifact_path: layout.artifact_path.to_string_lossy().into_owned(),
        schema_path: layout.schema_path.to_string_lossy().into_owned(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = output_dir_from_args();
    fs::create_dir_all(&output_dir)?;

    let manifests = manifests();
    let mut index = Vec::with_capacity(manifests.len());

    for manifest in &manifests {
        let layout = write_generated_pack(&output_dir, manifest)?;
        index.push(index_entry(&layout, manifest));
    }

    let index_path = output_dir.join("index.json");
    let json = serde_json::to_string_pretty(&index)?;
    fs::write(index_path, format!("{json}\n"))?;

    Ok(())
}
