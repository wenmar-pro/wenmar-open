//! What the tool reads from its surroundings: environment variables, the
//! platform's directories, and whether it is talking to a terminal.
//!
//! Everything is read once, in `main`, into an [`Env`]. Nothing else in the
//! crate looks at the process environment, so tests pass their own.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use crate::error::{CliError, NO_DATA};

/// The hosted API.
pub const DEFAULT_API: &str = "https://open.wenmarpro.com";

/// Where the data releases are listed.
pub const DEFAULT_RELEASES: &str = "https://api.github.com/repos/wenmar-pro/wenmar-open/releases";

/// The data file's name inside the data directory.
pub const DATA_FILE: &str = "wenmar-open.sqlite3";

/// The directory this tool makes inside the platform's data directory.
const DIRECTORY: &str = "wenmar-open";

#[derive(Debug, Clone, Default)]
pub struct Env {
    /// `WENMAR_OPEN_DATA_DIR`: the directory holding the data file.
    pub data_dir: Option<PathBuf>,
    /// `WENMAR_OPEN_API`: the base URL of the hosted API.
    pub api: Option<String>,
    /// `WENMAR_OPEN_RELEASES`: where the data releases are listed.
    pub releases: Option<String>,
    /// The platform's directory for application data, if it has one.
    pub platform_data_dir: Option<PathBuf>,
    /// The person's home directory, if there is one.
    pub home: Option<PathBuf>,
    /// `PATH`, for finding `claude` and `codex`.
    pub path: Option<std::ffi::OsString>,
    /// This program's own file. `--jq` runs its expression in a second
    /// process of it; without one the expression runs in this process.
    pub program: Option<PathBuf>,
    pub stdin_terminal: bool,
    pub stdout_terminal: bool,
    /// `NO_COLOR` is set to something.
    pub no_color: bool,
}

/// A variable that is set to nothing counts as not set.
fn variable(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

impl Env {
    /// Reads the real environment.
    pub fn from_process() -> Env {
        Env {
            data_dir: variable("WENMAR_OPEN_DATA_DIR").map(PathBuf::from),
            api: variable("WENMAR_OPEN_API").and_then(|value| value.into_string().ok()),
            releases: variable("WENMAR_OPEN_RELEASES").and_then(|value| value.into_string().ok()),
            platform_data_dir: dirs::data_dir(),
            home: dirs::home_dir(),
            path: variable("PATH"),
            program: std::env::current_exe().ok(),
            stdin_terminal: std::io::stdin().is_terminal(),
            stdout_terminal: std::io::stdout().is_terminal(),
            no_color: variable("NO_COLOR").is_some(),
        }
    }

    /// The directory holding the data file: `--data-dir`, then
    /// `WENMAR_OPEN_DATA_DIR`, then `wenmar-open` inside the platform's
    /// data directory.
    pub fn data_directory(&self, flag: Option<&Path>) -> Result<PathBuf, CliError> {
        if let Some(directory) = flag.filter(|path| !path.as_os_str().is_empty()) {
            return Ok(directory.to_path_buf());
        }
        if let Some(directory) = &self.data_dir {
            return Ok(directory.clone());
        }
        match &self.platform_data_dir {
            Some(directory) => Ok(directory.join(DIRECTORY)),
            None => Err(CliError::new(
                NO_DATA,
                "There is no home directory to keep the data file in.",
            )
            .with_hint("Set WENMAR_OPEN_DATA_DIR to a directory, or pass --data-dir.")),
        }
    }

    /// The path of the data file, whether or not it exists.
    pub fn data_file(&self, flag: Option<&Path>) -> Result<PathBuf, CliError> {
        Ok(self.data_directory(flag)?.join(DATA_FILE))
    }

    /// The API's base URL: `--api`, then `WENMAR_OPEN_API`, then the hosted
    /// one. A trailing slash is dropped.
    pub fn api_base(&self, flag: Option<&str>) -> String {
        flag.map(str::trim)
            .filter(|base| !base.is_empty())
            .or(self.api.as_deref())
            .unwrap_or(DEFAULT_API)
            .trim_end_matches('/')
            .to_owned()
    }

    /// Where the releases are listed: `--releases`, then
    /// `WENMAR_OPEN_RELEASES`, then GitHub.
    pub fn releases_url(&self, flag: Option<&str>) -> String {
        flag.map(str::trim)
            .filter(|url| !url.is_empty())
            .or(self.releases.as_deref())
            .unwrap_or(DEFAULT_RELEASES)
            .trim_end_matches('/')
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_wins_then_the_variable_then_the_platform() {
        let env = Env {
            data_dir: Some(PathBuf::from("/from/variable")),
            platform_data_dir: Some(PathBuf::from("/platform")),
            ..Env::default()
        };
        assert_eq!(
            env.data_file(Some(Path::new("/from/flag"))).unwrap(),
            Path::new("/from/flag/wenmar-open.sqlite3")
        );
        assert_eq!(
            env.data_file(None).unwrap(),
            Path::new("/from/variable/wenmar-open.sqlite3")
        );
        let env = Env {
            data_dir: None,
            ..env
        };
        assert_eq!(
            env.data_file(None).unwrap(),
            Path::new("/platform/wenmar-open/wenmar-open.sqlite3")
        );
        // An empty flag is the same as none.
        assert_eq!(
            env.data_directory(Some(Path::new(""))).unwrap(),
            Path::new("/platform/wenmar-open")
        );
    }

    #[test]
    fn with_no_home_directory_the_error_says_what_to_set() {
        let error = Env::default().data_file(None).unwrap_err();
        assert_eq!(error.code, "no_data");
        assert_eq!(
            error.hint(),
            Some("Set WENMAR_OPEN_DATA_DIR to a directory, or pass --data-dir.")
        );
    }

    #[test]
    fn the_api_base_loses_its_trailing_slash() {
        let env = Env {
            api: Some("http://localhost:3000/".to_owned()),
            ..Env::default()
        };
        assert_eq!(env.api_base(None), "http://localhost:3000");
        assert_eq!(
            env.api_base(Some("https://example.test//")),
            "https://example.test"
        );
        assert_eq!(env.api_base(Some("  ")), "http://localhost:3000");
        assert_eq!(Env::default().api_base(None), "https://open.wenmarpro.com");
        assert_eq!(
            Env::default().releases_url(None),
            "https://api.github.com/repos/wenmar-pro/wenmar-open/releases"
        );
    }
}
