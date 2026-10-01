//! Works out the build id and hands it to the compiler as `OPEN_BUILD_ID`.
//! See `build_id.rs` for what it is and why.

#[path = "build_id.rs"]
mod build_id;

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
    Ok(())
}
