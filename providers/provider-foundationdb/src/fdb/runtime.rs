#![cfg(feature = "foundationdb-real")]

use std::sync::OnceLock;

use sorla_provider_core::ProviderError;

/// Process-global FDB network boot guard. `boot()` returns a guard that must
/// outlive all DB usage; we leak it for the process lifetime.
static BOOT: OnceLock<()> = OnceLock::new();

fn ensure_booted() {
    BOOT.get_or_init(|| {
        // SAFETY: `foundationdb::boot()` starts the client network thread and
        // must be called exactly once per process before any Database use.
        // The returned guard is intentionally leaked so the network stays up
        // for the whole process; OnceLock guarantees single initialization.
        #[allow(unsafe_code)]
        let guard = unsafe { foundationdb::boot() };
        std::mem::forget(guard);
    });
}

#[allow(dead_code)]
pub struct FdbRuntime {
    rt: tokio::runtime::Runtime,
    db: foundationdb::Database,
}

pub fn connect(cluster_file: Option<&str>) -> Result<FdbRuntime, ProviderError> {
    ensure_booted();
    if let Some(path) = cluster_file {
        // SAFETY: env mutation guarded by single-threaded init before any
        // Database is opened; edition 2024 marks set_var unsafe.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("FDB_CLUSTER_FILE", path);
        }
    }
    let db = foundationdb::Database::default()
        .map_err(|err| ProviderError::Validation(format!("fdb open failed: {err}")))?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| ProviderError::Validation(format!("tokio runtime: {err}")))?;
    Ok(FdbRuntime { rt, db })
}

#[allow(dead_code)]
impl FdbRuntime {
    pub fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        self.rt.block_on(fut)
    }

    pub fn database(&self) -> &foundationdb::Database {
        &self.db
    }
}

#[cfg(all(test, feature = "foundationdb-real"))]
mod tests {
    use super::connect;

    // Requires FDB_CLUSTER_FILE=/home/bima-pangestu/fdb/fdb.cluster
    #[test]
    fn connects_and_runs_a_trivial_future() {
        let rt = connect(None).expect("connect to local cluster");
        let answer = rt.block_on(async { 1 + 1 });
        assert_eq!(answer, 2);
    }
}
