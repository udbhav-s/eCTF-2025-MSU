//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.
//!
//! The build script also sets the linker flags to tell it which link script to use.

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.x`
    // here, we ensure the build script is only re-run when
    // `memory.x` is changed.
    println!("cargo:rerun-if-changed=memory.x");

    // Specify linker arguments.

    // `--nmagic` is required if memory section addresses are not aligned to 0x10000,
    // for example the FLASH and RAM sections in your `memory.x`.
    // See https://github.com/rust-embedded/cortex-m-quickstart/pull/95
    println!("cargo:rustc-link-arg=--nmagic");

    // Set the linker script to the one provided by cortex-m-rt.
    println!("cargo:rustc-link-arg=-Tlink.x");

    // Use the absolute path for global.secrets since it's mounted at /global.secrets.
    let secret_path = Path::new("../global.secrets");
    println!("cargo:rerun-if-changed=/global.secrets");

    // Read the secrets file.
    let secrets_contents =
        fs::read_to_string(&secret_path).expect("Unable to read global.secrets file");

    // Parse the JSON content.
    let secrets_json: serde_json::Value =
        serde_json::from_str(&secrets_contents).expect("Invalid JSON in global.secrets");

    // Extract the fields you need.
    let decoder_dk = secrets_json
        .get("decoder_dk")
        .and_then(|v| v.as_str())
        .expect("Missing or invalid decoder_dk");
    let host_key_pub = secrets_json
        .get("host_key_pub")
        .and_then(|v| v.as_str())
        .expect("Missing or invalid host_key_pub");

    // Get and parse the DECODER_ID from the environment.
    let decoder_id_str =
        env::var("DECODER_ID").expect("DECODER_ID environment variable must be set");

    // Remove a potential "0x" prefix.
    let decoder_id_str = decoder_id_str.trim_start_matches("0x");
    let decoder_id_val: u32 =
        u32::from_str_radix(decoder_id_str, 16).expect("Failed to parse DECODER_ID as hex");

    // If you want an explicit little-endian byte array:
    let decoder_id_le = decoder_id_val.to_le_bytes();

    // Generate the Rust code for the secrets.
    let generated_code = format!(
        "pub const DECODER_DK: &'static [u8] = b{:?};\n\
        pub const HOST_KEY_PUB: &'static [u8] = b{:?};\n\
        pub const DECODER_ID: [u8; 4] = {:?};\n",
        decoder_dk, host_key_pub, decoder_id_le
    );

    // Write the generated code to $OUT_DIR/secrets.rs.
    File::create(out.join("secrets.rs"))
        .unwrap()
        .write_all(generated_code.as_bytes())
        .unwrap();
}
