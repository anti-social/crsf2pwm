//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.

use std::env;
use std::fs;
use std::path::PathBuf;

/// Build-time configuration: `(environment variable, default value, optional
/// maximum)`. The variables are exposed to the crate under their own name via
/// `env!`.
const CONFIG_VARS: [(&str, u64, Option<u64>); 3] = [
    ("CRSF2PWM_MOTOR_SOFT_START_DELAY", 1_000, None),
    ("CRSF2PWM_MOTOR_SOFT_STOP_DELAY", 1_000, None),
    ("CRSF2PWM_FILTER_CUT_FREQ", 5, Some(u16::MAX as u64)),
];

fn main() {
    let memory_filename = if env::var("CARGO_FEATURE_RP2350").is_ok() {
        "memory-rp2350.x"
    } else {
        "memory-rp2040.x"
    };

    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::copy(memory_filename, out.join("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.x`
    // here, we ensure the build script is only re-run when
    // `memory.x` is changed.
    println!("cargo:rerun-if-changed=memory-rp2040.x");
    println!("cargo:rerun-if-changed=memory-rp2350.x");

    for (env_name, default, max) in CONFIG_VARS {
        // Re-read the variable (and rebuild the crate) when it changes.
        println!("cargo:rerun-if-env-changed={env_name}");
        let value = match env::var(env_name) {
            Err(_) => default,
            Ok(raw) => match raw.parse::<u64>() {
                Ok(v) if v > 0 => v,
                Ok(_) => panic!("{env_name}={raw:?}: value must be greater than 0"),
                Err(_) => panic!("{env_name}={raw:?}: expected a positive integer"),
            },
        };
        if let Some(max) = max {
            if value > max {
                panic!("{env_name}={value}: value must be at most {max}");
            }
        }
        println!("cargo:rustc-env={env_name}={value}");
    }

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    // println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
