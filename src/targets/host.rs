//! Host (desktop POSIX) target backend.
//!
//! All methods are pass-through / identity — this reproduces the
//! existing mx behavior exactly.

use super::{TargetBackend, EntryPointCode, BuildOutputOpts};
use crate::errors::CompileResult;

pub struct HostBackend;

impl TargetBackend for HostBackend {
    fn c_preamble(&self) -> String {
        String::new()
    }

    fn c_epilogue(&self) -> String {
        String::new()
    }

    fn patch_runtime_header(&self, header: &str) -> String {
        header.to_string()
    }

    fn emit_entry_open(&self, debug_mode: bool) -> EntryPointCode {
        let mut init = String::from("int main(int _m2_argc, char **_m2_argv) {\n");
        init += "    m2_argc = _m2_argc; m2_argv = _m2_argv;\n";
        if debug_mode {
            init += "    setvbuf(stdout, NULL, _IONBF, 0);\n";
        }
        EntryPointCode {
            init_open: init,
            body_open: None,
            close: "}\n".to_string(),
        }
    }

    fn entry_returns_value(&self) -> bool {
        true
    }

    fn write_build_output(
        &self,
        _c_code: &str,
        _opts: &BuildOutputOpts,
    ) -> Option<CompileResult<()>> {
        None // fall through to default cc compilation
    }
}
