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
    provider_version: String,
    provider_slug: String,
    manifest_path: String,
    artifact_path: String,
    schema_path: String,
    oci_reference: Option<String>,
}

pub fn manifests(provider: Option<&str>) -> Vec<ProviderPackManifest> {
    let mut manifests = provider_manifests()
        .into_iter()
        .filter(|manifest| {
            provider
                .map(|selected| provider_matches(manifest, selected))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    manifests.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
    manifests
}

fn provider_manifests() -> Vec<ProviderPackManifest> {
    vec![
        foundationdb_manifest(),
        sharepoint_manifest(),
        rag_manifest(),
    ]
}

fn provider_matches(manifest: &ProviderPackManifest, selected: &str) -> bool {
    let selected_slug = selected.strip_prefix("provider-").unwrap_or(selected);
    let manifest_slug = manifest.provider_slug();
    selected == "all"
        || selected == manifest.provider_id
        || selected == manifest_slug
        || selected_slug == manifest_slug
}

fn output_dir_and_provider_from_args() -> (PathBuf, Option<String>) {
    let mut args = env::args().skip(1);
    let mut output_dir = PathBuf::from("examples/generated-packs");
    let mut provider = None;
    while let Some(arg) = args.next() {
        if arg == "--output-dir"
            && let Some(path) = args.next()
        {
            output_dir = PathBuf::from(path);
        } else if arg == "--provider"
            && let Some(selected_provider) = args.next()
            && selected_provider != "all"
        {
            provider = Some(selected_provider);
        }
    }
    (output_dir, provider)
}

pub fn index_entry(
    layout: &GeneratedPackLayout,
    manifest: &ProviderPackManifest,
) -> GeneratedPackIndexEntry {
    GeneratedPackIndexEntry {
        provider_id: manifest.provider_id.clone(),
        provider_version: manifest.provider_version.clone(),
        provider_slug: manifest.provider_slug(),
        manifest_path: layout.manifest_path.to_string_lossy().into_owned(),
        artifact_path: layout.artifact_path.to_string_lossy().into_owned(),
        schema_path: layout.schema_path.to_string_lossy().into_owned(),
        oci_reference: manifest.oci_reference.clone(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (output_dir, provider) = output_dir_and_provider_from_args();
    fs::create_dir_all(&output_dir)?;

    let manifests = manifests(provider.as_deref());
    if manifests.is_empty() {
        return Err(format!(
            "no provider matched '{}'",
            provider.as_deref().unwrap_or("all")
        )
        .into());
    }
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
