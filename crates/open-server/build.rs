//! Works out the build id and hands it to the compiler as `OPEN_BUILD_ID`,
//! and writes the rates of the Canadian invoice page out as Rust. See
//! `build_id.rs` and `rates_build.rs` for what each is and why.

#[path = "build_id.rs"]
mod build_id;
#[path = "rates_build.rs"]
mod rates_build;

use std::fs;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let root = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("CARGO_MANIFEST_DIR is not set"))?;
    // Cargo runs this again when anything the id is made from changes.
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=build_id.rs");
    for part in build_id::PARTS {
        println!("cargo::rerun-if-changed={part}");
    }
    let id = build_id::of(&root, &build_id::PARTS)?;
    println!("cargo::rustc-env=OPEN_BUILD_ID={id}");

    // The rates file: a file that is not as its schema says stops the build.
    let rates = root.join(rates_build::PATH);
    let toml = fs::read_to_string(&rates)
        .map_err(|error| io::Error::other(format!("{}: {error}", rates.display())))?;
    let code = rates_build::generate(&toml)
        .map_err(|why| io::Error::other(format!("{}: {why}", rates.display())))?;
    let out = std::env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("OUT_DIR is not set"))?;
    fs::write(out.join(rates_build::OUT), code)?;
    Ok(())
}
