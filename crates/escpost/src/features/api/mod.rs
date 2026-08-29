//! The print route, and the origin rule that guards it.
//!
//! Reading is already served by `/api/printers/list`, so only sending bytes is
//! added here, onto the same router.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use tokio::sync::Mutex;

/// One lock per printer name, held across a whole print, so two requests for
/// one printer queue rather than both opening the device. Async, because the
/// guard is held across `.await`.
pub(crate) type PrinterLocks = Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>;

mod error;
mod origin;
mod print;

#[derive(Clone, Debug, Default)]
pub(crate) struct ApiState {
    /// When set, only this extension id may call.
    pub(crate) extension_id: Option<String>,
    pub(crate) config: Option<std::path::PathBuf>,
    /// Names jobs within one run. Nothing persists a job id, so a counter is
    /// enough and a UUID dependency is not.
    pub(crate) job_sequence: Arc<AtomicU64>,
    pub(crate) printer_locks: PrinterLocks,
}

/// The origin rule guards this router alone. The rest of `/api` answers the
/// workbench on its own web origin, which the rule would reject.
pub(crate) fn router(state: ApiState) -> axum::Router {
    use axum::extract::DefaultBodyLimit;
    use axum::middleware;

    print::router()
        // Well above a raster receipt, and stated rather than inherited.
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), origin::guard))
        .with_state(state)
}
