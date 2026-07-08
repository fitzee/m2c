//! ESP-IDF (ESP32 family) target backend.
//!
//! Handles diagnostic suppression, runtime patching, app_main entry
//! point, and ESP-IDF project/component generation.

use std::fs;
use std::path::{Path, PathBuf};

use super::{TargetBackend, EntryPointCode, BuildOutputOpts};
use crate::errors::{CompileError, CompileResult};

pub struct EspIdfBackend;

impl TargetBackend for EspIdfBackend {
    fn c_preamble(&self) -> String {
        let mut s = String::new();
        s += "#pragma GCC diagnostic push\n";
        s += "#pragma GCC diagnostic ignored \"-Wunused-function\"\n";
        s += "#pragma GCC diagnostic ignored \"-Wunused-label\"\n";
        s += "#pragma GCC diagnostic ignored \"-Wunused-const-variable\"\n";
        s += "#pragma GCC diagnostic ignored \"-Wunused-variable\"\n";
        s += "#pragma GCC diagnostic ignored \"-Wunused-but-set-variable\"\n";
        s += "#pragma GCC diagnostic ignored \"-Wpointer-to-int-cast\"\n";
        s += "#pragma GCC diagnostic ignored \"-Wint-to-pointer-cast\"\n";
        s += "#pragma GCC diagnostic ignored \"-Wincompatible-pointer-types\"\n";
        s += "#pragma GCC diagnostic ignored \"-Wmaybe-uninitialized\"\n";
        s
    }

    fn c_epilogue(&self) -> String {
        "#pragma GCC diagnostic pop\n".to_string()
    }

    fn patch_runtime_header(&self, header: &str) -> String {
        let mut h = header.to_string();

        // No process model: replace exit() with abort()
        h = h.replace("exit(0)", "abort()");
        h = h.replace("exit(1)", "abort()");

        // No TLS on ESP32 variants
        h = h.replace("static __thread ", "static ");

        // Add ESP-IDF includes after setjmp.h
        h = h.replace(
            "#include <setjmp.h>\n",
            "#include <setjmp.h>\n\
             #include \"esp_log.h\"\n\
             #define MX_LOG_TAG \"mx\"\n",
        );

        // Tag the header
        h = h.replacen(
            "/* Modula-2 Runtime Support */",
            "/* Modula-2 Runtime Support (esp-idf) */",
            1,
        );

        h
    }

    fn emit_entry_open(&self, _debug_mode: bool) -> EntryPointCode {
        EntryPointCode {
            init_open: "void MxInit(void) {\n".to_string(),
            body_open: Some(
                "}\n\nvoid app_main(void) {\n    MxInit();\n".to_string(),
            ),
            close: "}\n".to_string(),
        }
    }

    fn entry_returns_value(&self) -> bool {
        false
    }

    fn write_build_output(
        &self,
        c_code: &str,
        opts: &BuildOutputOpts,
    ) -> Option<CompileResult<()>> {
        let project_name = opts.stem;
        let component_name = &format!("mx_{}", opts.stem);
        let result = write_project(
            c_code,
            project_name,
            component_name,
            opts.out_dir,
            opts.extra_c_files,
        );
        if result.is_ok() {
            eprintln!("{}: generated ESP-IDF project in {}/{}",
                crate::identity::COMPILER_NAME, opts.out_dir.display(), opts.stem);
        }
        Some(result)
    }
}

// ── ESP-IDF project/component generation ────────────────────────────

/// Write an ESP-IDF component from generated C source.
pub fn write_component(
    c_source: &str,
    component_name: &str,
    output_dir: &Path,
    extra_c_files: &[PathBuf],
) -> CompileResult<()> {
    let comp_dir = output_dir.join(component_name);
    fs::create_dir_all(&comp_dir).map_err(|e| {
        CompileError::driver(format!(
            "cannot create component directory '{}': {}",
            comp_dir.display(), e
        ))
    })?;

    // Write the generated C source
    let c_path = comp_dir.join(format!("{}.c", component_name));
    fs::write(&c_path, c_source).map_err(|e| {
        CompileError::driver(format!("cannot write '{}': {}", c_path.display(), e))
    })?;

    // Copy extra C files and their headers into the component
    let mut srcs = format!("\"{}.c\"", component_name);
    for extra in extra_c_files {
        if let Some(fname) = extra.file_name() {
            let dest = comp_dir.join(fname);
            fs::copy(extra, &dest).map_err(|e| {
                CompileError::driver(format!(
                    "cannot copy '{}' to component: {}", extra.display(), e
                ))
            })?;
            srcs += &format!(" \"{}\"", fname.to_string_lossy());
        }
        // Also copy matching .h file if it exists
        let h_path = extra.with_extension("h");
        if h_path.exists() {
            if let Some(hname) = h_path.file_name() {
                let hdest = comp_dir.join(hname);
                fs::copy(&h_path, &hdest).map_err(|e| {
                    CompileError::driver(format!(
                        "cannot copy '{}' to component: {}", h_path.display(), e
                    ))
                })?;
            }
        }
    }

    // Detect ESP-IDF component dependencies from extra C files
    // and the generated C source
    let mut requires = Vec::new();
    let mut all_sources = vec![c_source.to_string()];
    for extra in extra_c_files {
        if let Ok(content) = fs::read_to_string(extra) {
            all_sources.push(content);
        }
    }
    for content in &all_sources {
        if content.contains("esp_wifi") { requires.push("esp_wifi"); }
        if content.contains("esp_event") { requires.push("esp_event"); }
        if content.contains("nvs_flash") { requires.push("nvs_flash"); }
        if content.contains("esp_netif") { requires.push("esp_netif"); }
        if content.contains("socket(") || content.contains("recv(")
           || content.contains("send(") || content.contains("bind(")
           || content.contains("accept(") || content.contains("listen(")
           || content.contains("htons(") || content.contains("htonl(") {
            requires.push("lwip");
        }
    }
    requires.sort();
    requires.dedup();

    // Write CMakeLists.txt
    let requires_str = if requires.is_empty() {
        String::new()
    } else {
        format!("\n    REQUIRES {}", requires.join(" "))
    };
    let cmake = format!(
        "idf_component_register(\n    SRCS {srcs}\n    INCLUDE_DIRS \".\"{req}\n)\n",
        srcs = srcs,
        req = requires_str,
    );
    let cmake_path = comp_dir.join("CMakeLists.txt");
    fs::write(&cmake_path, cmake).map_err(|e| {
        CompileError::driver(format!("cannot write '{}': {}", cmake_path.display(), e))
    })?;

    Ok(())
}

/// Generate a minimal ESP-IDF project wrapper that references the component.
pub fn write_project(
    c_source: &str,
    project_name: &str,
    component_name: &str,
    output_dir: &Path,
    extra_c_files: &[PathBuf],
) -> CompileResult<()> {
    let proj_dir = output_dir.join(project_name);
    let comp_dir = proj_dir.join("components");
    let main_dir = proj_dir.join("main");

    fs::create_dir_all(&comp_dir).map_err(|e| {
        CompileError::driver(format!("cannot create '{}': {}", comp_dir.display(), e))
    })?;
    fs::create_dir_all(&main_dir).map_err(|e| {
        CompileError::driver(format!("cannot create '{}': {}", main_dir.display(), e))
    })?;

    // Write the component
    write_component(c_source, component_name, &comp_dir, extra_c_files)?;

    // Top-level CMakeLists.txt
    let top_cmake = format!(
        r#"cmake_minimum_required(VERSION 3.16)

include($ENV{{IDF_PATH}}/tools/cmake/project.cmake)
project({name})
"#,
        name = project_name,
    );
    fs::write(proj_dir.join("CMakeLists.txt"), top_cmake).map_err(|e| {
        CompileError::driver(format!("cannot write top CMakeLists.txt: {}", e))
    })?;

    // main/CMakeLists.txt -- registers main component, depends on mx component
    let main_cmake = format!(
        r#"idf_component_register(
    SRCS "main.c"
    INCLUDE_DIRS "."
    REQUIRES {comp}
)
"#,
        comp = component_name,
    );
    fs::write(main_dir.join("CMakeLists.txt"), main_cmake).map_err(|e| {
        CompileError::driver(format!("cannot write main CMakeLists.txt: {}", e))
    })?;

    // main/main.c -- empty, app_main is in the mx component
    let main_c = r#"/* ESP-IDF main stub.
   app_main() is provided by the mx-generated component. */
"#;
    fs::write(main_dir.join("main.c"), main_c).map_err(|e| {
        CompileError::driver(format!("cannot write main.c: {}", e))
    })?;

    // sdkconfig.defaults -- use large partition + size optimization
    let sdkconfig = "CONFIG_PARTITION_TABLE_SINGLE_APP_LARGE=y\n\
                     CONFIG_COMPILER_OPTIMIZATION_SIZE=y\n\
                     CONFIG_ESP_MAIN_TASK_STACK_SIZE=8192\n\
                     CONFIG_TASK_WDT_TIMEOUT_S=30\n";
    fs::write(proj_dir.join("sdkconfig.defaults"), sdkconfig).map_err(|e| {
        CompileError::driver(format!("cannot write sdkconfig.defaults: {}", e))
    })?;

    // Copy any sdkconfig.defaults from the source directory (user overrides)
    if let Some(first_extra) = extra_c_files.first() {
        if let Some(src_dir) = first_extra.parent() {
            let user_sdkconfig = src_dir.join("sdkconfig.defaults");
            if user_sdkconfig.exists() {
                let _ = fs::copy(&user_sdkconfig, proj_dir.join("sdkconfig.defaults"));
            }
        }
    }

    Ok(())
}
