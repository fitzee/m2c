# Embedded Platform Support

mx can generate C for embedded targets. The C backend produces portable code that the target platform's toolchain compiles. mx does not invoke the cross-compiler directly.

## Platforms

| Platform | Flag | Entry point | I/O | Status |
|----------|------|-------------|-----|--------|
| `host` | default | `main(argc, argv)` | printf/scanf | stable |
| `esp-idf` | `--platform esp-idf` | `app_main()` | ESP_LOGx | alpha |

## ESP-IDF

### Quick start

```
mx compile --platform esp-idf Hello.mod
```

This generates a complete ESP-IDF project:

```
hello/
    CMakeLists.txt                  # top-level project
    main/
        CMakeLists.txt              # main component
        main.c                      # stub (app_main is in the mx component)
    components/
        mx_hello/
            CMakeLists.txt          # component registration
            mx_hello.c              # generated C (runtime + program)
```

Build and flash:

```
cd hello
idf.py set-target esp32     # or esp32s3, esp32c3, esp32c6
idf.py build
idf.py flash monitor
```

### What changes vs host

The ESP-IDF platform generates C with these differences:

- No `stdio.h` -- I/O goes through `ESP_LOGI` instead of `printf`
- No `argc/argv` -- no process model
- No `__thread` -- no thread-local storage
- Entry point is `void app_main(void)` instead of `int main()`
- `abort()` instead of `exit()` for unhandled exceptions
- `setjmp/longjmp` still available (ESP-IDF newlib supports it)
- 32-bit pointers (ILP32 data model)

### What stays the same

- All SYSTEM operations (ADR, CAST, TSIZE, ADDRESS)
- Strings module
- Value types (INTEGER, CARDINAL, REAL, BOOLEAN, CHAR)
- Record types, arrays, pointers
- Exception handling (TRY/EXCEPT via setjmp)
- Module initialization order

### m2.toml

You can set the platform in your manifest:

```toml
name=myproject
version=1.0.0
edition=m2plus
entry=src/Main.mod
platform=esp-idf
```

### Emit C only

If you want just the C file without the project wrapper:

```
mx compile --platform esp-idf --emit-c Hello.mod
```

This writes `Hello.c` that you can integrate into an existing ESP-IDF project yourself.

## Adding new platforms

A platform is defined by a `PlatformProfile` in `src/platform.rs`. To add support for a new embedded target (Zephyr, STM32, RP2040):

1. Add a variant to the `Platform` enum
2. Implement `PlatformProfile::new_platform()` with the right feature flags
3. Add a project generator if the platform has a specific build system (like `esp_idf.rs`)
4. Add the platform name to `Platform::from_str()`

No code generation changes are needed. The C backend produces the same structure for all platforms. The differences are in the runtime preamble (includes, I/O, exception handling) and the entry point wrapper.
