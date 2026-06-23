#![cfg(feature = "foundationdb-real")]

use foundationdb::api::NetworkAutoStop;
use sorla_provider_core::ProviderError;

/// Boot the FDB client network. The CALLER MUST hold the returned guard until
/// all DB use is finished; dropping it stops the network cleanly (its `Drop`
/// joins the client network thread). `foundationdb` permits a single boot per
/// process — call this once (process startup, or once at the top of a test).
#[allow(unsafe_code)]
pub fn boot_network() -> NetworkAutoStop {
    // SAFETY: `foundationdb::boot()` starts the client network thread and must
    // be called exactly once per process before any Database use. The returned
    // guard MUST be dropped (stops the network) before the process exits.
    unsafe { foundationdb::boot() }
}

pub struct FdbRuntime {
    rt: tokio::runtime::Runtime,
    db: foundationdb::Database,
}

/// Open a Database and build a current-thread runtime. Does NOT boot the
/// network — the caller must have already called `boot_network()` and be
/// holding the guard.
pub fn connect(cluster_file: Option<&str>) -> Result<FdbRuntime, ProviderError> {
    if let Some(path) = cluster_file {
        // SAFETY: edition 2024 marks set_var unsafe; set before opening the DB.
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

impl FdbRuntime {
    pub fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        self.rt.block_on(fut)
    }

    pub fn database(&self) -> &foundationdb::Database {
        &self.db
    }
}
