//! Target backend trait and per-platform implementations.
//!
//! Each supported platform gets its own file implementing `TargetBackend`.
//! The codegen and driver call trait methods instead of checking platform
//! flags directly.  Adding a new platform means adding a file here and
//! a match arm in `backend_for` — no changes to codegen or driver.

pub mod host;
pub mod esp_idf;

use crate::platform::Platform;
use crate::errors::CompileResult;
use std::path::{Path, PathBuf};

/// Structured entry point code returned by `emit_entry_open`.
pub struct EntryPointCode {
    /// Function signature + opening brace + setup (argc/argv, setvbuf).
    pub init_open: String,
    /// Code emitted after module init calls but before the module body.
    /// For ESP-IDF: closes MxInit, opens app_main, calls MxInit().
    /// For Host: None.
    pub body_open: Option<String>,
    /// Closing brace(s) for the entry point.
    pub close: String,
}

/// Data passed to `write_build_output`.
pub struct BuildOutputOpts<'a> {
    pub input_path: &'a Path,
    pub stem: &'a str,
    pub out_dir: &'a Path,
    pub extra_c_files: &'a [PathBuf],
    pub verbose: bool,
}

/// Extension points for platform-specific code generation and build output.
pub trait TargetBackend {
    /// C pragmas/diagnostics at top of generated .c file.
    fn c_preamble(&self) -> String;

    /// C code at end of generated .c file (after module body).
    fn c_epilogue(&self) -> String;

    /// Patch the host runtime header for this platform's constraints.
    fn patch_runtime_header(&self, header: &str) -> String;

    /// Emit the program entry point function(s).
    fn emit_entry_open(&self, debug_mode: bool) -> EntryPointCode;

    /// Whether the module body should emit `return 0;` as default.
    fn entry_returns_value(&self) -> bool;

    /// Handle build output. Return `Some` if handled (e.g. project generation),
    /// `None` to fall through to default cc compilation.
    fn write_build_output(
        &self,
        c_code: &str,
        opts: &BuildOutputOpts,
    ) -> Option<CompileResult<()>>;
}

/// Get the backend implementation for a platform.
pub fn backend_for(platform: &Platform) -> Box<dyn TargetBackend> {
    match platform {
        Platform::Host => Box::new(host::HostBackend),
        Platform::EspIdf => Box::new(esp_idf::EspIdfBackend),
    }
}
