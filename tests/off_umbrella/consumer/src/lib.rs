//! Off-umbrella proof consumer: builds a `RuntimeContext` the way a
//! crates.io user of `oxideav-meta` does. The assertions live in
//! `tests/proof.rs`.

use oxideav_core::RuntimeContext;

/// A fresh context with everything meta wired at build time installed.
pub fn registered_context() -> RuntimeContext {
    let mut ctx = RuntimeContext::new();
    oxideav_meta::register_all(&mut ctx);
    ctx
}
