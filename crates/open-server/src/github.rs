//! Files an issue on GitHub: the one request this server makes to anyone.
//!
//! The call blocks. Run it with `tokio::task::spawn_blocking`.

use std::time::Duration;

use serde_json::{Value, json};

/// Longest the whole request may take: less than `crate::REQUEST_TIMEOUT`.
const TIMEOUT: Duration = Duration::from_secs(8);
/// The longest answer read.
const LONGEST_ANSWER: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub title: String,
    pub body: String,
}

/// Why an issue was not filed. None of these holds the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileError {
    /// No answer came.
    Unreachable(String),
    /// GitHub answered with this status instead of 201.
    Refused(u16),
    /// The answer was not the JSON of a new issue.
    Unreadable,
}

/// Where issues of `repo` are created, on the API at `api`.
pub fn address(api: &str, repo: &str) -> String {
    format!("{api}/repos/{repo}/issues")
}

/// Creates the issue and returns the address of its page.
pub fn file(api: &str, repo: &str, token: &str, issue: &Issue) -> Result<String, FileError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(TIMEOUT))
        .user_agent(format!("wenmar-open-server/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let body = json!({ "title": issue.title, "body": issue.body, "labels": ["data"] }).to_string();
    let mut response = agent
        .post(&address(api, repo))
        .header("authorization", format!("Bearer {token}"))
        .header("accept", "application/vnd.github+json")
        .header("x-github-api-version", "2022-11-28")
        .content_type("application/json")
        .send(body)
        .map_err(|error| FileError::Unreachable(error.to_string()))?;
    let status = response.status().as_u16();
    if status != 201 {
        return Err(FileError::Refused(status));
    }
    let text = response
        .body_mut()
        .with_config()
        .limit(LONGEST_ANSWER)
        .read_to_string()
        .map_err(|_| FileError::Unreadable)?;
    let answer: Value = serde_json::from_str(&text).map_err(|_| FileError::Unreadable)?;
    match answer["html_url"].as_str() {
        // The address goes into a link on a page, so only GitHub's own is taken.
        Some(page)
            if page.starts_with("https://github.com/") && !page.contains(['"', '<', '>', ' ']) =>
        {
            Ok(page.to_owned())
        }
        _ => Err(FileError::Unreadable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_is_the_repository_issues_endpoint() {
        assert_eq!(
            address("https://api.github.com", "wenmar-pro/wenmar-open"),
            "https://api.github.com/repos/wenmar-pro/wenmar-open/issues"
        );
    }

    #[test]
    fn an_api_that_cannot_be_reached_is_an_error_without_the_token() {
        let issue = Issue {
            title: "t".to_owned(),
            body: "b".to_owned(),
        };
        // Nothing listens on port 1.
        let error = file("http://127.0.0.1:1", "a/b", "s3cret", &issue).unwrap_err();
        assert!(matches!(error, FileError::Unreachable(_)), "{error:?}");
        assert!(!format!("{error:?}").contains("s3cret"));
    }
}
