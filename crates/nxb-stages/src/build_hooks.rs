//! Callbacks of [`crate::build`] for a consumer that runs beside it (the pipeline archive).

use std::sync::Arc;

/// The completion callback: `(key, status)`.
pub type DoneFn = dyn Fn(&str, &str) + Send + Sync;

/// The start callback: the key list.
pub type StartFn = dyn Fn(&[String]) + Send + Sync;

/// Called once per shader when its outputs are final: `(key, status)` (`ok` or the failure class). The pipeline uses it
/// to start work that needs a shader's libraries (archive batches) before the whole step ends.
#[derive(Clone)]
pub struct OnDone(pub Arc<DoneFn>);

impl std::fmt::Debug for OnDone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OnDone")
    }
}

/// Called once with the keys the build will translate, before any shader starts.
#[derive(Clone)]
pub struct OnStart(pub Arc<StartFn>);

impl std::fmt::Debug for OnStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OnStart")
    }
}
