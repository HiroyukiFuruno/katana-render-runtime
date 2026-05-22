//! Compatibility wrapper for `katana-render-runtime`.
//!
//! New consumers should depend on `katana-render-runtime`. This crate remains
//! available so existing `katana-diagram-renderer` consumers can move without
//! changing all imports at once.

pub use katana_render_runtime::*;
