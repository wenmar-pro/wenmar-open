//! The build id: a short name for everything the server's answers are made
//! from, apart from the data file.
//!
//! A browser or a CDN keeps a page until its `ETag` changes, and keeps the
//! stylesheet for a week at an address that names the build. The crate
//! version alone will not do for either: a reworded page or a fixed
//! stylesheet is usually deployed without a new version number. So the id
//! is worked out from the files themselves when the crate is built, and is
//! the same for the same files wherever they are built.
//!
//! This file is compiled into the build script, and into the crate's unit
//! tests. The server itself reads no file but the data file.

use std::fs;
use std::io;
use std::path::Path;

/// What the id is made from, as paths from this crate's directory: the
/// pages, the stylesheet and the script, the server's code, the two
/// libraries it answers with, and the versions of everything else.
pub const PARTS: [&str; 7] = [
    "Cargo.toml",
    "assets",
    "src",
    "templates",
    "../wenmar-vehicles/src",
    "../wenmar-vin/src",
    "../../Cargo.lock",
];

/// Every file at or under `path`, as its name from `root` and its contents.
fn collect(root: &Path, name: &str, files: &mut Vec<(String, Vec<u8>)>) -> io::Result<()> {
    let path = root.join(name);
    if path.is_dir() {
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            let file_name = entry.file_name();
            collect(
                root,
                &format!("{name}/{}", file_name.to_string_lossy()),
                files,
            )?;
        }
    } else {
        files.push((name.to_owned(), fs::read(&path)?));
    }
    Ok(())
}

/// FNV-1a, 64 bits: small enough to write here, so the build script needs
/// no dependency, and the same on every machine and toolchain.
fn mix(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The id of the files named by `parts` under `root`: 16 hexadecimal
/// digits. It changes when a file's contents or name does, or when a file
/// is added or removed. A part that does not exist is an error, so that a
/// moved directory cannot quietly drop out of the id.
pub fn of(root: &Path, parts: &[&str]) -> io::Result<String> {
    let mut files = Vec::new();
    for part in parts {
        collect(root, part, &mut files)?;
    }
    // The order a directory is listed in differs between machines.
    files.sort();
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for (name, contents) in &files {
        hash = mix(hash, name.as_bytes());
        // A name cannot run into the contents, or one file into the next.
        hash = mix(hash, &[0]);
        hash = mix(hash, &(contents.len() as u64).to_le_bytes());
        hash = mix(hash, contents);
    }
    Ok(format!("{hash:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, name: &str, contents: &str) {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn site() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        write(directory.path(), "templates/about.html", "<p>One line.</p>");
        write(directory.path(), "templates/home.html", "<h1>Home</h1>");
        write(directory.path(), "assets/site.css", "p{margin:0}");
        directory
    }

    const SITE: [&str; 2] = ["templates", "assets"];

    #[test]
    fn the_same_files_have_the_same_id() {
        let (one, other) = (site(), site());
        let id = of(one.path(), &SITE).unwrap();
        assert_eq!(id, of(other.path(), &SITE).unwrap());
        assert_eq!(id.len(), 16);
        assert!(id.bytes().all(|byte| byte.is_ascii_hexdigit()), "{id}");
        // The order the parts are named in does not matter.
        assert_eq!(id, of(one.path(), &["assets", "templates"]).unwrap());
    }

    #[test]
    fn any_change_to_a_page_or_an_asset_changes_the_id() {
        let directory = site();
        let root = directory.path();
        let before = of(root, &SITE).unwrap();

        // A reworded page.
        write(root, "templates/about.html", "<p>One line!</p>");
        let reworded = of(root, &SITE).unwrap();
        assert_ne!(reworded, before);
        write(root, "templates/about.html", "<p>One line.</p>");
        assert_eq!(of(root, &SITE).unwrap(), before);

        // A fixed stylesheet.
        write(root, "assets/site.css", "p{margin:1px}");
        assert_ne!(of(root, &SITE).unwrap(), before);
        write(root, "assets/site.css", "p{margin:0}");

        // A new file, an empty one, and one in a new directory.
        write(root, "templates/new.html", "");
        let added = of(root, &SITE).unwrap();
        assert_ne!(added, before);
        write(root, "templates/parts/new.html", "");
        assert_ne!(of(root, &SITE).unwrap(), added);
        fs::remove_dir_all(root.join("templates/parts")).unwrap();
        fs::remove_file(root.join("templates/new.html")).unwrap();
        assert_eq!(of(root, &SITE).unwrap(), before);

        // A renamed file, and text moved from one file to the next.
        fs::rename(
            root.join("templates/home.html"),
            root.join("templates/start.html"),
        )
        .unwrap();
        assert_ne!(of(root, &SITE).unwrap(), before);
        fs::rename(
            root.join("templates/start.html"),
            root.join("templates/home.html"),
        )
        .unwrap();
        write(root, "templates/about.html", "<p>One line.</p><h1>");
        write(root, "templates/home.html", "Home</h1>");
        assert_ne!(of(root, &SITE).unwrap(), before);
    }

    #[test]
    fn a_part_that_is_not_there_is_an_error() {
        let directory = site();
        assert!(of(directory.path(), &["templates", "styles"]).is_err());
    }

    #[test]
    fn the_id_compiled_in_is_the_id_of_this_source() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(of(root, &PARTS).unwrap(), crate::BUILD_ID);
        // The pages, the stylesheet and the script are all among the parts.
        for part in ["templates", "assets", "src"] {
            assert!(PARTS.contains(&part), "{part}");
        }
    }
}
