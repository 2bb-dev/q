//! Codex (OpenAI CLI) integration: binary resolution, auth status, device-auth
//! flow, subscription-backed prompt upgrade, ANSI/warning helpers.
//!
//! Ported from `a3io/q:src-tauri/src/lib.rs`. Tauri bindings stripped:
//!   - `tauri::AppHandle` is gone; `open::that` handles browser-launch.
//!   - `State<'_, CodexLoginState>` becomes `Arc<Mutex<CodexDeviceAuthProgress>>`
//!     passed by the caller, so the TUI and the CLI can both manage their
//!     own progress container.
//!   - `#[tauri::command]` attributes removed.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

use crate::META_PROMPT;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAuthStatus {
    pub logged_in: bool,
    pub subscription_ready: bool,
    pub status_text: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexDeviceAuthProgress {
    pub state: String,
    pub verification_url: String,
    pub user_code: String,
    pub message: String,
    pub browser_opened: bool,
}

// ---- Environment / binary resolution ----

fn home_join(path: &str) -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| Path::new(&home).join(path))
}

fn effective_machine_arch() -> String {
    if cfg!(target_os = "macos") {
        if let Ok(output) = std::process::Command::new("/usr/bin/uname")
            .arg("-m")
            .output()
        {
            let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !arch.is_empty() {
                return match arch.as_str() {
                    "arm64" => "aarch64".to_string(),
                    other => other.to_string(),
                };
            }
        }
    }

    std::env::consts::ARCH.to_string()
}

fn resolve_codex_binary_and_vendor_path() -> (PathBuf, Option<PathBuf>) {
    let arch = effective_machine_arch();
    let platform_candidates: &[(&str, &str)] = match std::env::consts::OS {
        "macos" if arch == "aarch64" => &[
            ("@openai/codex-darwin-arm64", "aarch64-apple-darwin"),
            ("@openai/codex-darwin-x64", "x86_64-apple-darwin"),
        ],
        "macos" => &[
            ("@openai/codex-darwin-x64", "x86_64-apple-darwin"),
            ("@openai/codex-darwin-arm64", "aarch64-apple-darwin"),
        ],
        "linux" if arch == "aarch64" => &[
            ("@openai/codex-linux-arm64", "aarch64-unknown-linux-musl"),
            ("@openai/codex-linux-x64", "x86_64-unknown-linux-musl"),
        ],
        "linux" => &[
            ("@openai/codex-linux-x64", "x86_64-unknown-linux-musl"),
            ("@openai/codex-linux-arm64", "aarch64-unknown-linux-musl"),
        ],
        "windows" if arch == "aarch64" => &[
            ("@openai/codex-win32-arm64", "aarch64-pc-windows-msvc"),
            ("@openai/codex-win32-x64", "x86_64-pc-windows-msvc"),
        ],
        "windows" => &[
            ("@openai/codex-win32-x64", "x86_64-pc-windows-msvc"),
            ("@openai/codex-win32-arm64", "aarch64-pc-windows-msvc"),
        ],
        _ => &[],
    };

    for (pkg, triple) in platform_candidates {
        let binary_name = if cfg!(target_os = "windows") {
            "codex.exe"
        } else {
            "codex"
        };
        let native_candidates = [
            home_join(&format!(
                ".npm-global/lib/node_modules/@openai/codex/node_modules/{pkg}/vendor/{triple}/codex/{binary_name}"
            )),
            home_join(&format!(
                ".npm-global/lib/node_modules/{pkg}/vendor/{triple}/codex/{binary_name}"
            )),
            home_join(&format!(
                ".bun/install/global/node_modules/@openai/codex/node_modules/{pkg}/vendor/{triple}/codex/{binary_name}"
            )),
        ];

        for binary in native_candidates.into_iter().flatten() {
            if binary.exists() {
                let vendor_path = binary
                    .parent()
                    .and_then(|p| p.parent())
                    .map(|p| p.join("path"))
                    .filter(|p| p.exists());
                return (binary, vendor_path);
            }
        }
    }

    let candidates = [
        home_join(".npm-global/bin/codex"),
        home_join(".local/bin/codex"),
        Some(PathBuf::from("/opt/homebrew/bin/codex")),
        Some(PathBuf::from("/usr/local/bin/codex")),
        Some(PathBuf::from("codex")),
    ];

    let binary = candidates
        .into_iter()
        .flatten()
        .find(|candidate| candidate == Path::new("codex") || candidate.exists())
        .unwrap_or_else(|| PathBuf::from("codex"));

    (binary, None)
}

fn codex_command() -> Command {
    let (binary, vendor_path) = resolve_codex_binary_and_vendor_path();
    let mut command = Command::new(binary);

    if let Some(extra_path) = vendor_path {
        let existing = std::env::var_os("PATH").unwrap_or_default();
        let mut combined = std::env::split_paths(&existing).collect::<Vec<_>>();
        combined.insert(0, extra_path);
        if let Ok(joined) = std::env::join_paths(combined) {
            command.env("PATH", joined);
        }
    }

    command
}

fn temp_output_path(prefix: &str, suffix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{}-{}{}",
        prefix,
        chrono::Utc::now().timestamp_millis(),
        suffix
    ))
}

// ---- Output parsing ----

pub fn strip_ansi_sequences(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b {
            i += 1;
            if i < bytes.len() && bytes[i] == b'[' {
                i += 1;
                while i < bytes.len() {
                    let b = bytes[i];
                    i += 1;
                    if (0x40..=0x7e).contains(&b) {
                        break;
                    }
                }
                continue;
            }
        }

        out.push(bytes[i] as char);
        i += 1;
    }

    out
}

fn extract_device_auth_url(line: &str) -> Option<String> {
    let trimmed = strip_ansi_sequences(line).trim().to_string();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Some(trimmed)
    } else {
        None
    }
}

fn extract_device_auth_code(line: &str) -> Option<String> {
    let trimmed = strip_ansi_sequences(line).trim().to_string();
    let looks_like_code = trimmed.len() >= 8
        && trimmed
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
        && trimmed.contains('-');
    if looks_like_code {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn cleaned_output_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(strip_ansi_sequences)
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn normalize_search_text(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn is_path_update_warning(line: &str) -> bool {
    normalize_search_text(line)
        .contains("warning: proceeding, even though we could not update path")
}

fn is_device_code_warning(line: &str) -> bool {
    let normalized = normalize_search_text(line);
    normalized.contains("device codes are a common phishing target")
        || normalized.contains("never share this code")
}

fn is_ignorable_codex_line(line: &str) -> bool {
    is_path_update_warning(line) || is_device_code_warning(line)
}

fn contains_chatgpt_login(lines: &[String]) -> bool {
    lines
        .iter()
        .any(|line| normalize_search_text(line).contains("logged in using chatgpt"))
}

fn contains_api_key_login(lines: &[String]) -> bool {
    lines
        .iter()
        .any(|line| normalize_search_text(line).contains("logged in using api key"))
}

fn summarize_codex_output(lines: &[String]) -> String {
    lines
        .iter()
        .filter(|line| !is_ignorable_codex_line(line))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

// ---- Public commands ----

/// Query `codex login status` and classify the result.
pub async fn codex_auth_status() -> Result<CodexAuthStatus, String> {
    let output = codex_command()
        .arg("login")
        .arg("status")
        .output()
        .await
        .map_err(|e| format!("Failed to run codex login status: {e}"))?;

    let stdout_lines = cleaned_output_lines(&output.stdout);
    let stderr_lines = cleaned_output_lines(&output.stderr);
    let mut all_lines = stdout_lines.clone();
    all_lines.extend(stderr_lines.clone());
    let combined = summarize_codex_output(&all_lines);

    if output.status.success() {
        let subscription_ready = contains_chatgpt_login(&all_lines);
        let api_key_only = contains_api_key_login(&all_lines);
        let status_text = if subscription_ready {
            "Ready".to_string()
        } else if api_key_only {
            "API Key Only".to_string()
        } else {
            "Logged In".to_string()
        };
        let detail = if subscription_ready {
            "Codex is signed in with ChatGPT and ready for subscription-backed Prompt Upgrade."
                .to_string()
        } else if api_key_only {
            "Codex is logged in with an API key. Run `codex login` and choose ChatGPT to use subscription access."
                .to_string()
        } else if !combined.is_empty() {
            combined
        } else {
            "Codex is logged in on this machine.".to_string()
        };

        Ok(CodexAuthStatus {
            logged_in: true,
            subscription_ready,
            status_text,
            detail,
        })
    } else {
        Ok(CodexAuthStatus {
            logged_in: false,
            subscription_ready: false,
            status_text: "Not Logged In".to_string(),
            detail: if combined.is_empty() {
                "Run `codex login` in a terminal to connect your ChatGPT account.".to_string()
            } else {
                combined
            },
        })
    }
}

/// Spawn the device-auth flow. Updates `progress` as lines stream from the
/// codex subprocess. If `open_browser` is true, shells out to open the
/// verification URL. Returns the initial progress snapshot; callers should
/// poll `progress` until `state` becomes "succeeded" or "failed".
pub async fn start_codex_device_auth(
    progress: Arc<Mutex<CodexDeviceAuthProgress>>,
    open_browser: bool,
) -> Result<CodexDeviceAuthProgress, String> {
    if let Ok(status) = codex_auth_status().await {
        if status.subscription_ready {
            let mut guard = progress.lock().await;
            *guard = CodexDeviceAuthProgress {
                state: "succeeded".to_string(),
                verification_url: String::new(),
                user_code: String::new(),
                message: "Codex is already signed in with ChatGPT.".to_string(),
                browser_opened: false,
            };
            return Ok(guard.clone());
        }
    }

    {
        let current = progress.lock().await.clone();
        if current.state == "pending" {
            return Ok(current);
        }
    }

    {
        let mut guard = progress.lock().await;
        *guard = CodexDeviceAuthProgress {
            state: "pending".to_string(),
            verification_url: String::new(),
            user_code: String::new(),
            message: "Starting Codex device authorization…".to_string(),
            browser_opened: false,
        };
    }

    let shared_progress = progress.clone();

    tokio::spawn(async move {
        let mut child = match codex_command()
            .arg("login")
            .arg("--device-auth")
            .env("NO_COLOR", "1")
            .env("TERM", "dumb")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                let mut guard = shared_progress.lock().await;
                *guard = CodexDeviceAuthProgress {
                    state: "failed".to_string(),
                    verification_url: String::new(),
                    user_code: String::new(),
                    message: format!("Failed to start Codex login: {err}"),
                    browser_opened: false,
                };
                return;
            }
        };

        let mut last_message = String::new();

        if let Some(stderr) = child.stderr.take() {
            let shared_progress = shared_progress.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let cleaned = strip_ansi_sequences(&line).trim().to_string();
                    if cleaned.is_empty() || is_ignorable_codex_line(&cleaned) {
                        continue;
                    }
                    let mut guard = shared_progress.lock().await;
                    guard.message = cleaned.clone();
                }
            });
        }

        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let cleaned = strip_ansi_sequences(&line).trim().to_string();
                if !cleaned.is_empty() && !is_ignorable_codex_line(&cleaned) {
                    last_message = cleaned.clone();
                }

                if let Some(url) = extract_device_auth_url(&line) {
                    let mut guard = shared_progress.lock().await;
                    guard.verification_url = url.clone();
                    guard.message =
                        "Open the login page, enter the one-time code, then finish sign-in."
                            .to_string();
                    if open_browser && !guard.browser_opened && open::that(&url).is_ok() {
                        guard.browser_opened = true;
                    }
                    continue;
                }

                if let Some(code) = extract_device_auth_code(&line) {
                    let mut guard = shared_progress.lock().await;
                    guard.user_code = code;
                    guard.message =
                        "Enter the one-time code in the browser to complete sign-in.".to_string();
                    continue;
                }
            }
        }

        let result = child.wait().await;
        let auth_status = codex_auth_status().await.ok();
        let mut guard = shared_progress.lock().await;
        let ready = auth_status
            .as_ref()
            .map(|s| s.subscription_ready)
            .unwrap_or(false);

        if ready {
            guard.state = "succeeded".to_string();
            guard.verification_url.clear();
            guard.user_code.clear();
            guard.message = "ChatGPT login complete. Prompt Upgrade is ready.".to_string();
            return;
        }

        match result {
            Ok(status) if status.success() => {
                guard.state = "failed".to_string();
                guard.message = auth_status.map(|s| s.detail).unwrap_or_else(|| {
                    "Codex login finished, but ChatGPT subscription was not detected.".to_string()
                });
            }
            Ok(_) => {
                guard.state = "failed".to_string();
                guard.message = if !last_message.is_empty() {
                    format!("Codex login failed: {last_message}")
                } else {
                    "Codex login was cancelled or failed.".to_string()
                };
            }
            Err(err) => {
                guard.state = "failed".to_string();
                guard.message = format!("Codex login failed: {err}");
            }
        }
    });

    Ok(progress.lock().await.clone())
}

/// Run the subscription-backed prompt upgrade via `codex exec`.
pub async fn run_codex_upgrade(model: String, raw_prompt: String) -> Result<String, String> {
    let status = codex_auth_status().await?;
    if !status.subscription_ready {
        return Err(status.detail);
    }

    let output_path = temp_output_path("q-codex-upgrade", ".txt");
    let prompt = format!(
        r#"{META_PROMPT}

Additional rules:
- Do not inspect local files.
- Do not run shell commands.
- Do not use tools.
- Answer directly with the improved prompt only.

Raw prompt:
{raw_prompt}"#
    );

    let mut child = codex_command()
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("read-only")
        .arg("--ephemeral")
        .arg("-c")
        .arg(r#"provider="openai""#)
        .arg("-m")
        .arg(model)
        .arg("-o")
        .arg(&output_path)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Failed to start Codex: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|e| format!("Failed to write prompt to Codex: {e}"))?;
    }

    let output = match timeout(Duration::from_secs(90), child.wait_with_output()).await {
        Ok(res) => res.map_err(|e| format!("Codex execution failed: {e}"))?,
        Err(_) => {
            return Err("Codex timed out after 90s".to_string());
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Codex execution failed".to_string()
        } else {
            stderr
        });
    }

    let text = tokio::fs::read_to_string(&output_path)
        .await
        .map_err(|e| format!("Failed to read Codex output: {e}"))?;
    let _ = tokio::fs::remove_file(&output_path).await;

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        Err("Codex returned empty output".to_string())
    } else {
        Ok(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_drops_csi_escapes() {
        let input = "\x1b[31mred\x1b[0m text";
        assert_eq!(strip_ansi_sequences(input), "red text");
    }

    #[test]
    fn extract_url_returns_http_lines() {
        assert_eq!(
            extract_device_auth_url("  https://example.com/login  "),
            Some("https://example.com/login".to_string())
        );
        assert_eq!(extract_device_auth_url("no url here"), None);
    }

    #[test]
    fn extract_code_matches_uppercase_with_dash() {
        assert_eq!(
            extract_device_auth_code(" ABCD-1234 "),
            Some("ABCD-1234".to_string())
        );
        assert_eq!(extract_device_auth_code("lowercase-nope"), None);
    }
}
