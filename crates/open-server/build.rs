//! Works out the build id and hands it to the compiler as `OPEN_BUILD_ID`,
//! and writes the rates of the Canadian invoice page and the rules for
//! repair shops out as Rust. See `build_id.rs`, `rates_build.rs` and
//! `rules_build.rs` for what each is and why.

#[path = "battery_build.rs"]
mod battery_build;
#[path = "battery_groups_build.rs"]
mod battery_groups_build;
#[path = "build_id.rs"]
mod build_id;
#[path = "links_build.rs"]
mod links_build;
#[path = "rates_build.rs"]
mod rates_build;
#[path = "refrigerant_build.rs"]
mod refrigerant_build;
#[path = "rules_build.rs"]
mod rules_build;
#[path = "wire_build.rs"]
mod wire_build;

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

    // The refrigerant chart, read and checked the same way.
    let chart = root.join(refrigerant_build::PATH);
    let toml = fs::read_to_string(&chart)
        .map_err(|error| io::Error::other(format!("{}: {error}", chart.display())))?;
    let code = refrigerant_build::generate(&toml)
        .map_err(|why| io::Error::other(format!("{}: {why}", chart.display())))?;
    fs::write(out.join(refrigerant_build::OUT), code)?;

    // The battery table, read and checked the same way.
    let battery = root.join(battery_build::PATH);
    let toml = fs::read_to_string(&battery)
        .map_err(|error| io::Error::other(format!("{}: {error}", battery.display())))?;
    let code = battery_build::generate(&toml)
        .map_err(|why| io::Error::other(format!("{}: {why}", battery.display())))?;
    fs::write(out.join(battery_build::OUT), code)?;

    // The makers' own links: one that is not as makes.yaml says stops the build.
    let makes = root.join(links_build::PATH);
    let yaml = fs::read_to_string(&makes)
        .map_err(|error| io::Error::other(format!("{}: {error}", makes.display())))?;
    let code = links_build::generate(&yaml)
        .map_err(|why| io::Error::other(format!("{}: {why}", makes.display())))?;
    fs::write(out.join(links_build::OUT), code)?;

    // The wire table, read and checked the same way.
    let wire = root.join(wire_build::PATH);
    let toml = fs::read_to_string(&wire)
        .map_err(|error| io::Error::other(format!("{}: {error}", wire.display())))?;
    let code = wire_build::generate(&toml)
        .map_err(|why| io::Error::other(format!("{}: {why}", wire.display())))?;
    fs::write(out.join(wire_build::OUT), code)?;

    // The battery group sizes, read and checked the same way.
    let groups = root.join(battery_groups_build::PATH);
    let toml = fs::read_to_string(&groups)
        .map_err(|error| io::Error::other(format!("{}: {error}", groups.display())))?;
    let code = battery_groups_build::generate(&toml)
        .map_err(|why| io::Error::other(format!("{}: {why}", groups.display())))?;
    fs::write(out.join(battery_groups_build::OUT), code)?;

    // The rules for repair shops, read and checked the same way.
    let rules = root.join(rules_build::PATH);
    let toml = fs::read_to_string(&rules)
        .map_err(|error| io::Error::other(format!("{}: {error}", rules.display())))?;
    let code = rules_build::generate(&toml)
        .map_err(|why| io::Error::other(format!("{}: {why}", rules.display())))?;
    fs::write(out.join(rules_build::OUT), code)?;
    Ok(())
}
