//! GitHub authentication: token storage, `gh` CLI token reuse, and the
//! OAuth device flow.
//!
//! The device flow needs an OAuth app client id with device flow enabled,
//! read from the `QCLI_GITHUB_CLIENT_ID` environment variable. Reusing an
//! existing `gh` CLI login works without any client id.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

use crate::paths::app_dir;

const TOKEN_FILE: &str = "github_token";
const USER_FILE: &str = "github_user";
const USER_AGENT: &str = concat!("q-cli/", env!("CARGO_PKG_VERSION"));
pub const SCOPE: &str = "repo";

#[derive(Debug, Error)]
pub enum GithubError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("github request failed: {0}")]
    Request(String),
    #[error("{0}")]
    Auth(String),
}

impl From<ureq::Error> for GithubError {
    fn from(error: ureq::Error) -> Self {
        GithubError::Request(error.to_string())
    }
}

/// Where the active token came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    /// Stored by q's own device flow.
    Stored,
    /// Borrowed from the `gh` CLI.
    GhCli,
}

fn token_path() -> std::io::Result<PathBuf> {
    Ok(app_dir()?.join(TOKEN_FILE))
}

/// Token stored by q's own device flow, if any.
pub fn stored_token() -> std::io::Result<Option<String>> {
    let path = token_path()?;
    match std::fs::read_to_string(&path) {
        Ok(token) => {
            let token = token.trim().to_string();
            Ok((!token.is_empty()).then_some(token))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Stores the token with owner-only permissions.
pub fn store_token(token: &str) -> std::io::Result<()> {
    let path = token_path()?;
    std::fs::write(&path, format!("{token}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn delete_token() -> std::io::Result<bool> {
    let path = token_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Login cached from the last successful GitHub connection. Used for
/// attribution without a network round-trip.
pub fn cached_login() -> std::io::Result<Option<String>> {
    let path = app_dir()?.join(USER_FILE);
    match std::fs::read_to_string(&path) {
        Ok(login) => {
            let login = login.trim().to_string();
            Ok((!login.is_empty()).then_some(login))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn store_cached_login(login: &str) -> std::io::Result<()> {
    std::fs::write(app_dir()?.join(USER_FILE), format!("{login}\n"))
}

pub fn clear_cached_login() -> std::io::Result<()> {
    match std::fs::remove_file(app_dir()?.join(USER_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Token from an existing `gh` CLI login, if the binary exists and is
/// authenticated.
pub fn gh_cli_token() -> Option<String> {
    let output = Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!token.is_empty()).then_some(token)
}

/// The token q should use: its own stored token first, then the `gh` CLI.
pub fn resolve_token() -> std::io::Result<Option<(String, TokenSource)>> {
    if let Some(token) = stored_token()? {
        return Ok(Some((token, TokenSource::Stored)));
    }
    Ok(gh_cli_token().map(|token| (token, TokenSource::GhCli)))
}

/// OAuth app client id for the device flow.
pub fn client_id() -> Option<String> {
    std::env::var("QCLI_GITHUB_CLIENT_ID")
        .ok()
        .filter(|id| !id.trim().is_empty())
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceAuthorization {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

impl DeviceAuthorization {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.interval.max(1))
    }
}

/// Starts the device flow: returns the code the user must enter at the
/// verification URI.
pub fn start_device_flow(client_id: &str) -> Result<DeviceAuthorization, GithubError> {
    let response = ureq::post("https://github.com/login/device/code")
        .set("Accept", "application/json")
        .set("User-Agent", USER_AGENT)
        .send_form(&[("client_id", client_id), ("scope", SCOPE)])?;
    Ok(response.into_json()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicePoll {
    /// The user has not finished authorizing yet; keep polling.
    Pending { slow_down: bool },
    /// Authorization finished; the access token is ready.
    Token(String),
}

#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Polls the device flow once.
pub fn poll_device_flow(client_id: &str, device_code: &str) -> Result<DevicePoll, GithubError> {
    let response = ureq::post("https://github.com/login/oauth/access_token")
        .set("Accept", "application/json")
        .set("User-Agent", USER_AGENT)
        .send_form(&[
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])?;
    let parsed: AccessTokenResponse = response.into_json()?;
    if let Some(token) = parsed.access_token {
        return Ok(DevicePoll::Token(token));
    }
    match parsed.error.as_deref() {
        Some("authorization_pending") => Ok(DevicePoll::Pending { slow_down: false }),
        Some("slow_down") => Ok(DevicePoll::Pending { slow_down: true }),
        Some(error) => Err(GithubError::Auth(
            parsed
                .error_description
                .unwrap_or_else(|| error.to_string()),
        )),
        None => Err(GithubError::Auth("empty device flow response".to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    login: String,
}

/// Topic marking repositories that hold q workspaces.
pub const WORKSPACE_TOPIC: &str = "q-workspace";

#[derive(Debug, Clone, Deserialize)]
pub struct CreatedRepo {
    pub full_name: String,
    pub clone_url: String,
}

/// Creates a private repository for a team workspace under the user's
/// account, or under `org` when given, and tags it with the workspace topic.
pub fn create_workspace_repo(
    token: &str,
    org: Option<&str>,
    name: &str,
) -> Result<CreatedRepo, GithubError> {
    let url = match org {
        Some(org) => format!("https://api.github.com/orgs/{org}/repos"),
        None => "https://api.github.com/user/repos".to_string(),
    };
    let response = ureq::post(&url)
        .set("Accept", "application/vnd.github+json")
        .set("Authorization", &format!("Bearer {token}"))
        .set("User-Agent", USER_AGENT)
        .send_json(ureq::json!({
            "name": name,
            "private": true,
            "description": "q prompt queue workspace",
        }))?;
    let repo: CreatedRepo = response.into_json()?;
    ureq::request(
        "PUT",
        &format!("https://api.github.com/repos/{}/topics", repo.full_name),
    )
    .set("Accept", "application/vnd.github+json")
    .set("Authorization", &format!("Bearer {token}"))
    .set("User-Agent", USER_AGENT)
    .send_json(ureq::json!({ "names": [WORKSPACE_TOPIC] }))?;
    Ok(repo)
}

#[derive(Debug, Deserialize)]
struct OrgResponse {
    login: String,
}

/// Logins of the organizations the token's user belongs to.
pub fn list_org_logins(token: &str) -> Result<Vec<String>, GithubError> {
    let response = ureq::get("https://api.github.com/user/orgs")
        .set("Accept", "application/vnd.github+json")
        .set("Authorization", &format!("Bearer {token}"))
        .set("User-Agent", USER_AGENT)
        .call()?;
    let orgs: Vec<OrgResponse> = response.into_json()?;
    Ok(orgs.into_iter().map(|org| org.login).collect())
}

/// Fetches the login of the token's user.
pub fn fetch_login(token: &str) -> Result<String, GithubError> {
    let response = ureq::get("https://api.github.com/user")
        .set("Accept", "application/vnd.github+json")
        .set("Authorization", &format!("Bearer {token}"))
        .set("User-Agent", USER_AGENT)
        .call()?;
    let parsed: UserResponse = response.into_json()?;
    Ok(parsed.login)
}

#[cfg(test)]
#[path = "../tests/unit/github.rs"]
mod tests;
