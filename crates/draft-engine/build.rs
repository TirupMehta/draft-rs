//! Make the Vulkan SDK's import library visible to the final link.
//!
//! `transcribe-cpp` emits `cargo:rustc-link-lib=vulkan-1` but does not emit a
//! search path, so a Windows machine with the SDK outside the default MSVC
//! library path fails at link time. This build script forwards the standard
//! `VULKAN_SDK` location without hard-coding a machine-specific path.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let Some(sdk) = std::env::var_os("VULKAN_SDK") else {
        println!(
            "cargo:warning=VULKAN_SDK is not set; the Vulkan linker may not find vulkan-1.lib"
        );
        return;
    };

    let lib = PathBuf::from(sdk).join("Lib");
    if lib.is_dir() {
        println!("cargo:rustc-link-search=native={}", lib.display());
    } else {
        println!(
            "cargo:warning=VULKAN_SDK does not contain a Lib directory: {}",
            lib.display()
        );
    }
}
