//! Settings, read from the environment once at startup.

use std::path::PathBuf;

/// A setting that must not be shown: a token. `{:?}` prints `Secret(..)`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(text: impl Into<String>) -> Secret {
        Secret(text.into())
    }

    /// The setting itself, for the one place that sends it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Secret(..)")
    }
}

/// How the server runs. Every value has a default except the data file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `OPEN_DATA`: the data file to serve.
    pub data: PathBuf,
    /// `OPEN_SHOP_DATA`: the shop file to serve beside the data file.
    /// Unset or blank: no shop data.
    pub shop_data: Option<PathBuf>,
    /// `PORT`: the port to listen on, on every interface. Default 3000.
    pub port: u16,
    /// `OPEN_CONNECTIONS`: connection slots; each holds two read-only
    /// connections to the data file. Default 4.
    pub connections: usize,
    /// `OPEN_VIN_CACHE_ROWS`: how many pattern rows of the data file are
    /// kept between VIN decodes, shared by every connection. About 190
    /// bytes each. Default 200,000; 0 turns the cache off.
    pub vin_cache_rows: usize,
    /// `OPEN_TRUSTED_PROXIES`: how many reverse proxies in front of the
    /// server add to `X-Forwarded-For`. Default 0: the header is ignored.
    pub trusted_proxies: usize,
    /// `OPEN_RATE_LIMIT`: requests one address may make in a minute.
    /// Default 600.
    pub requests_per_minute: u32,
    /// `OPEN_BASE_URL`: the public address, used in absolute links.
    pub base_url: String,
    /// `OPEN_FEEDBACK_EMAIL`: where "report an error by email" sends mail.
    /// Unset: no email link is shown.
    pub feedback_email: Option<String>,
    /// `OPEN_GITHUB_REPO`: the repository error reports are opened in, as
    /// `owner/name`. Default `wenmar-pro/wenmar-open`.
    pub github_repo: String,
    /// `OPEN_GITHUB_TOKEN`: a fine-grained token with only "issues: write"
    /// on `github_repo`. Unset: `/feedback` shows links and files nothing.
    pub github_token: Option<Secret>,
    /// `OPEN_GITHUB_API`: where GitHub's API is. Default
    /// `https://api.github.com`. Tests point it at a stub.
    pub github_api: String,
}

/// Whether `text` can follow `mailto:` in a link: one `@`, a dot inside the
/// host, and no character that would end the address or start a query.
fn is_email(text: &str) -> bool {
    let mut parts = text.split('@');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(name), Some(host), None) => {
            !name.is_empty()
                && host.contains('.')
                && !host.starts_with('.')
                && !host.ends_with('.')
                && text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "@._+-".contains(c))
        }
        _ => false,
    }
}

/// Whether `text` is `owner/name`, as GitHub writes a repository.
fn is_repo(text: &str) -> bool {
    let part = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    };
    let mut parts = text.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(name), None) => part(owner) && part(name),
        _ => false,
    }
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
        let given = |name: &str| {
            lookup(name)
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty())
        };
        let feedback_email = given("OPEN_FEEDBACK_EMAIL");
        if let Some(email) = &feedback_email
            && !is_email(email)
        {
            return Err(format!(
                "OPEN_FEEDBACK_EMAIL must be an email address, but it is {email:?}"
            ));
        }
        let github_repo =
            given("OPEN_GITHUB_REPO").unwrap_or_else(|| "wenmar-pro/wenmar-open".to_owned());
        if !is_repo(&github_repo) {
            return Err(format!(
                "OPEN_GITHUB_REPO must be owner/name, but it is {github_repo:?}"
            ));
        }
        let github_token = given("OPEN_GITHUB_TOKEN").map(Secret::new);
        let github_api = given("OPEN_GITHUB_API")
            .unwrap_or_else(|| "https://api.github.com".to_owned())
            .trim_end_matches('/')
            .to_owned();
        if !(github_api.starts_with("http://") || github_api.starts_with("https://"))
            || github_api.chars().any(char::is_whitespace)
        {
            return Err(format!(
                "OPEN_GITHUB_API must start with http:// or https://, but it is {github_api:?}"
            ));
        }
        Ok(Config {
            data: PathBuf::from(data),
            shop_data: lookup("OPEN_SHOP_DATA")
                .filter(|path| !path.trim().is_empty())
                .map(PathBuf::from),
            port: number(&lookup, "PORT", 3000)?,
            connections: number(&lookup, "OPEN_CONNECTIONS", 4usize)?.clamp(1, 32),
            vin_cache_rows: number(
                &lookup,
                "OPEN_VIN_CACHE_ROWS",
                wenmar_open_db::DEFAULT_VIN_CACHE_ROWS,
            )?,
            trusted_proxies: number(&lookup, "OPEN_TRUSTED_PROXIES", 0)?,
            requests_per_minute: number(&lookup, "OPEN_RATE_LIMIT", 600u32)?.max(1),
            base_url,
            feedback_email,
            github_repo,
            github_token,
            github_api,
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
        assert_eq!(config.vin_cache_rows, 200_000);
        assert_eq!(config.trusted_proxies, 0);
        assert_eq!(config.requests_per_minute, 600);
        assert_eq!(config.base_url, "https://open.wenmarpro.com");
        assert_eq!(config.shop_data, None);
    }

    #[test]
    fn the_shop_file_is_optional_and_a_blank_one_is_none() {
        let config = Config::from_lookup(lookup(&[
            ("OPEN_DATA", "d"),
            ("OPEN_SHOP_DATA", "shop.sqlite3"),
        ]))
        .unwrap();
        assert_eq!(config.shop_data, Some(PathBuf::from("shop.sqlite3")));
        let blank =
            Config::from_lookup(lookup(&[("OPEN_DATA", "d"), ("OPEN_SHOP_DATA", "  ")])).unwrap();
        assert_eq!(blank.shop_data, None);
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

    fn with(name: &'static str, value: &'static str) -> Result<Config, String> {
        Config::from_lookup(move |asked| match asked {
            "OPEN_DATA" => Some("d".to_owned()),
            asked if asked == name => Some(value.to_owned()),
            _ => None,
        })
    }

    #[test]
    fn feedback_settings_have_defaults_and_are_tidied() {
        let config = with("PORT", "3000").unwrap();
        assert_eq!(config.feedback_email, None);
        assert_eq!(config.github_repo, "wenmar-pro/wenmar-open");
        let config = with("OPEN_FEEDBACK_EMAIL", " data@open.example ").unwrap();
        assert_eq!(config.feedback_email.as_deref(), Some("data@open.example"));
        assert_eq!(
            with("OPEN_GITHUB_REPO", "someone/a-fork.rs")
                .unwrap()
                .github_repo,
            "someone/a-fork.rs"
        );
        // An empty setting is the same as none.
        assert_eq!(
            with("OPEN_FEEDBACK_EMAIL", " ").unwrap().feedback_email,
            None
        );
        assert_eq!(
            with("OPEN_GITHUB_REPO", "").unwrap().github_repo,
            "wenmar-pro/wenmar-open"
        );
    }

    #[test]
    fn the_github_settings_are_read_and_the_token_is_never_printed() {
        let config = with("PORT", "3000").unwrap();
        assert_eq!(config.github_token, None);
        assert_eq!(config.github_api, "https://api.github.com");
        let config = with("OPEN_GITHUB_TOKEN", " github_pat_abc123 ").unwrap();
        assert_eq!(
            config.github_token.as_ref().map(Secret::expose),
            Some("github_pat_abc123")
        );
        assert!(!format!("{config:?}").contains("abc123"));
        assert!(format!("{config:?}").contains("Secret(..)"));
        assert_eq!(with("OPEN_GITHUB_TOKEN", " ").unwrap().github_token, None);
        assert_eq!(
            with("OPEN_GITHUB_API", "http://127.0.0.1:9/")
                .unwrap()
                .github_api,
            "http://127.0.0.1:9"
        );
        let error = with("OPEN_GITHUB_API", "api.github.com").unwrap_err();
        assert!(
            error.starts_with("OPEN_GITHUB_API must start with http:// or https://"),
            "{error}"
        );
    }

    #[test]
    fn a_feedback_setting_that_is_wrong_is_named() {
        for bad in [
            "nobody",
            "a@b",
            "a b@c.d",
            "a@b.c?subject=x",
            "<a@b.c>",
            "a@@b.c",
        ] {
            let error = with("OPEN_FEEDBACK_EMAIL", bad).unwrap_err();
            assert!(
                error.starts_with("OPEN_FEEDBACK_EMAIL must be an email address"),
                "{bad}: {error}"
            );
        }
        for bad in ["wenmar-open", "a/b/c", "a/", "/b", "a b/c", "a/b?c"] {
            let error = with("OPEN_GITHUB_REPO", bad).unwrap_err();
            assert!(
                error.starts_with("OPEN_GITHUB_REPO must be owner/name"),
                "{bad}: {error}"
            );
        }
    }
}
