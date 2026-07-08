//! Platform profiles for embedded and desktop targets.
//!
//! A platform describes the runtime environment: what OS services are
//! available, how I/O works, what the entry point looks like, and how
//! memory is allocated.  The C backend generates identical code structure
//! for all platforms -- the differences are in the runtime preamble and
//! entry point wrapper.
//!
//! `Platform::Host` reproduces the current behavior exactly.
//! Adding a new platform (e.g. Zephyr, STM32) means defining a new
//! variant and its PlatformProfile -- no codegen changes required.

use std::fmt;

// ── Platform enum ────────────────────────────────────────────────────

/// Supported platform targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Desktop POSIX (Linux, macOS).  Default.
    Host,
    /// Espressif ESP-IDF (ESP32, ESP32-S2/S3, ESP32-C3/C6).
    EspIdf,
}

impl Platform {
    /// Parse from a string (command-line or m2.toml).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "host" => Some(Platform::Host),
            "esp-idf" | "espidf" | "esp_idf" => Some(Platform::EspIdf),
            _ => None,
        }
    }

    /// Canonical name for display and serialization.
    pub fn name(&self) -> &'static str {
        match self {
            Platform::Host => "host",
            Platform::EspIdf => "esp-idf",
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ── Entry point model ────────────────────────────────────────────────

/// How the compiled program is entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryModel {
    /// `int main(int argc, char **argv)` -- standard C.
    MainArgcArgv,
    /// `void app_main(void)` -- ESP-IDF / FreeRTOS.
    AppMain,
}

// ── I/O model ────────────────────────────────────────────────────────

/// How WriteString/WriteLn/WriteInt etc. are implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoModel {
    /// Standard C printf/scanf.
    Stdio,
    /// ESP-IDF ESP_LOGx macros.
    EspLog,
    /// No-op stubs.  User links their own implementation.
    Stub,
}

// ── Allocator model ──────────────────────────────────────────────────

/// How NEW/DISPOSE and internal allocation work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocModel {
    /// Standard libc malloc/free.
    Libc,
    /// ESP-IDF heap_caps / pvPortMalloc.
    EspIdf,
    /// No-op stubs.
    Stub,
}

// ── Platform profile ─────────────────────────────────────────────────

/// Complete description of a platform's runtime capabilities.
/// Drives conditional code in the runtime preamble and entry point.
#[derive(Debug, Clone)]
pub struct PlatformProfile {
    pub platform: Platform,

    // ── Available features ────────────────────────────────────────
    /// printf/scanf/FILE* available.
    pub has_stdio: bool,
    /// fopen/fread/fwrite available.
    pub has_filesystem: bool,
    /// Process model: argc/argv, exit().
    pub has_process: bool,
    /// setjmp/longjmp available.
    pub has_setjmp: bool,
    /// POSIX threads (pthread_create etc.).
    pub has_threads: bool,
    /// Thread-local storage (__thread keyword).
    pub has_tls: bool,
    /// Signal handlers (signal/sigaction).
    pub has_signals: bool,

    // ── Models ───────────────────────────────────────────────────
    pub entry_model: EntryModel,
    pub io_model: IoModel,
    pub alloc_model: AllocModel,

    // ── Target properties ────────────────────────────────────────
    /// Pointer width in bits (32 or 64).
    pub pointer_bits: u32,
}

impl PlatformProfile {
    /// Desktop POSIX profile.  Matches current mx behavior exactly.
    pub fn host() -> Self {
        Self {
            platform: Platform::Host,
            has_stdio: true,
            has_filesystem: true,
            has_process: true,
            has_setjmp: true,
            has_threads: true,
            has_tls: true,
            has_signals: true,
            entry_model: EntryModel::MainArgcArgv,
            io_model: IoModel::Stdio,
            alloc_model: AllocModel::Libc,
            pointer_bits: 64,
        }
    }

    /// ESP-IDF profile (ESP32 family).
    /// ESP-IDF ships newlib which provides stdio, stdlib, string, math.
    /// The main differences from host are: no process model (no argc/argv,
    /// no exit), no pthreads by default, no TLS, and the entry point is
    /// app_main instead of main.
    pub fn esp_idf() -> Self {
        Self {
            platform: Platform::EspIdf,
            has_stdio: true,     // newlib provides printf/putchar/getchar
            has_filesystem: true, // VFS layer provides fopen/fread (SPIFFS, FAT, etc.)
            has_process: false,   // no argc/argv, no exit()
            has_setjmp: true,     // newlib supports it
            has_threads: false,   // FreeRTOS available but not auto-mapped to pthreads
            has_tls: false,       // __thread not reliable on all ESP32 variants
            has_signals: false,
            entry_model: EntryModel::AppMain,
            io_model: IoModel::Stdio,  // putchar/getchar via newlib UART
            alloc_model: AllocModel::Libc, // newlib provides malloc/free
            pointer_bits: 32,
        }
    }

    /// Construct from a Platform enum.
    pub fn from_platform(p: Platform) -> Self {
        match p {
            Platform::Host => Self::host(),
            Platform::EspIdf => Self::esp_idf(),
        }
    }

    /// Whether this is a desktop/hosted platform.
    pub fn is_hosted(&self) -> bool {
        self.has_process && self.has_stdio
    }

    /// Whether this is a bare-metal or RTOS platform.
    pub fn is_embedded(&self) -> bool {
        !self.is_hosted()
    }
}
