//! Settings, read from the environment once at startup.

use std::path::PathBuf;

/// How the server runs. Every value has a default except the data file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `OPEN_DATA`: the data file to serve.
    pub data: PathBuf,
    /// `PORT`: the port to listen on, on every interface. Default 3000.
    pub port: u16,
    /// `OPEN_CONNECTIONS`: connection slots; each holds two read-only
    /// connections to the data file. Default 4.
    pub connections: usize,
    /// `OPEN_TRUSTED_PROXIES`: how many reverse proxies in front of the
    /// server add to `X-Forwarded-For`. Default 0: the header is ignored.
    pub trusted_proxies: usize,
    /// `OPEN_RATE_LIMIT`: requests one address may make in a minute.
    /// Default 600.
    pub requests_per_minute: u32,
    /// `OPEN_BASE_URL`: the public address, used in absolute links.
    pub base_url: String,
}

fn number<T: std::str::FromStr>(
    lookup: &impl Fn(&str) -> Option<String>,
    name: &str,
    default: T,
) -> Result<T, String> {
    match lookup(name) {
        None => Ok(default),
        Some(text) => text
            .trim()
            .parse()
            .map_err(|_| format!("{name} must be a number, but it is {text:?}")),
    }
}

impl Config {
    /// Reads the process environment.
    pub fn from_env() -> Result<Config, String> {
        Config::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads settings through `lookup`, so tests need not touch the
    /// environment.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Config, String> {
        let data = lookup("OPEN_DATA")
            .filter(|path| !path.trim().is_empty())
            .ok_or(
                "OPEN_DATA is not set. Set it to the path of a data file, or use: mise run serve",
            )?;
        let base_url = lookup("OPEN_BASE_URL")
            .unwrap_or_else(|| "https://open.wenmarpro.com".to_owned())
            .trim_end_matches('/')
            .to_owned();
        Ok(Config {
            data: PathBuf::from(data),
            port: number(&lookup, "PORT", 3000)?,
            connections: number(&lookup, "OPEN_CONNECTIONS", 4usize)?.clamp(1, 32),
            trusted_proxies: number(&lookup, "OPEN_TRUSTED_PROXIES", 0)?,
            requests_per_minute: number(&lookup, "OPEN_RATE_LIMIT", 600u32)?.max(1),
            base_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn only_the_data_file_is_required() {
        let config = Config::from_lookup(lookup(&[("OPEN_DATA", "data.sqlite3")])).unwrap();
        assert_eq!(config.data, PathBuf::from("data.sqlite3"));
        assert_eq!(config.port, 3000);
        assert_eq!(config.connections, 4);
        assert_eq!(config.trusted_proxies, 0);
        assert_eq!(config.requests_per_minute, 600);
        assert_eq!(config.base_url, "https://open.wenmarpro.com");
    }

    #[test]
    fn a_missing_data_file_setting_says_what_to_do() {
        let error = Config::from_lookup(lookup(&[])).unwrap_err();
        assert!(error.contains("OPEN_DATA is not set"), "{error}");
    }

    #[test]
    fn a_setting_that_is_not_a_number_is_named() {
        let error =
            Config::from_lookup(lookup(&[("OPEN_DATA", "d"), ("PORT", "http")])).unwrap_err();
        assert_eq!(error, "PORT must be a number, but it is \"http\"");
    }

    #[test]
    fn settings_are_kept_within_bounds() {
        let config = Config::from_lookup(lookup(&[
            ("OPEN_DATA", "d"),
            ("OPEN_CONNECTIONS", "0"),
            ("OPEN_RATE_LIMIT", "0"),
            ("OPEN_BASE_URL", "http://localhost:3000/"),
        ]))
        .unwrap();
        assert_eq!(config.connections, 1);
        assert_eq!(config.requests_per_minute, 1);
        assert_eq!(config.base_url, "http://localhost:3000");
    }
}
