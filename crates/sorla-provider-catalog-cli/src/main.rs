#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use sorla_provider_catalog::{ProviderCatalog, read_manifest, write_catalog};

fn arg_value(flag: &str, default: &str) -> PathBuf {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == flag
            && let Some(path) = args.next()
        {
            return PathBuf::from(path);
        }
    }
    PathBuf::from(default)
}

fn manifest_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(root)
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| path.join("manifest.json"))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let packs_dir = arg_value("--packs-dir", "examples/generated-packs");
    let output_dir = arg_value("--output-dir", "examples/generated-catalog");

    let manifests = manifest_paths(&packs_dir)
        .into_iter()
        .map(read_manifest)
        .collect::<Result<Vec<_>, _>>()?;

    let catalog = ProviderCatalog::from_manifests(&manifests);
    write_catalog(&output_dir, &catalog)?;

    Ok(())
}
