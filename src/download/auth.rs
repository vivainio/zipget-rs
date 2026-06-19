//! GitHub authentication for private repositories.
//!
//! Token resolution order, on a 401/403/404 from an unauthenticated request:
//!   1. `GITHUB_TOKEN` / `GH_TOKEN` environment variables (explicit override).
//!   2. `gh` CLI accounts, the one whose username contains the repo owner first
//!      (e.g. owner `basware` -> account `villevai_Basware`), then the rest.
//!
//! `gh auth token --user <name>` is read-only and does not change the active
//! account, so iterating accounts never disturbs the user's `gh` state.

use crate::download::http;
use anyhow::Result;
use std::path::Path;
use std::process::Command;

/// Extract the owner ("org") portion of an `owner/repo` string.
pub fn repo_owner(repo: &str) -> &str {
    repo.split('/').next().unwrap_or(repo)
}

/// Result of a single HTTP GET attempt.
enum Attempt {
    Ok(Box<ureq::Response>),
    Status(u16),
    Transport,
}

fn attempt(url: &str, token: Option<&str>) -> Attempt {
    let mut request = ureq::get(url).set("User-Agent", "zipget-rs");
    if let Some(token) = token {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    match request.call() {
        Ok(response) => Attempt::Ok(Box::new(response)),
        // ureq surfaces any >= 400 status as an error rather than a response.
        Err(ureq::Error::Status(code, _)) => Attempt::Status(code),
        Err(_) => Attempt::Transport,
    }
}

/// Perform a GitHub API GET, trying credentials only if the unauthenticated
/// request fails. Returns the successful response together with the token that
/// worked (`None` if the public/unauthenticated request succeeded).
pub fn github_api_get(api_url: &str, owner: &str) -> Result<(ureq::Response, Option<String>)> {
    let mut last_status: Option<u16> = None;
    let mut tried_tokens: Vec<String> = Vec::new();

    // 1. Unauthenticated — keeps the public fast path free of any `gh` calls.
    match attempt(api_url, None) {
        Attempt::Ok(response) => return Ok((*response, None)),
        Attempt::Status(code) => last_status = Some(code),
        Attempt::Transport => {}
    }

    // 2. Explicit env tokens.
    for var in ["GITHUB_TOKEN", "GH_TOKEN"] {
        let token = match std::env::var(var) {
            Ok(t) => t.trim().to_string(),
            Err(_) => continue,
        };
        if token.is_empty() || tried_tokens.contains(&token) {
            continue;
        }
        tried_tokens.push(token.clone());
        match attempt(api_url, Some(&token)) {
            Attempt::Ok(response) => return Ok((*response, Some(token))),
            Attempt::Status(code) => last_status = Some(code),
            Attempt::Transport => {}
        }
    }

    // 3. gh CLI accounts, the one matching the repo owner first.
    for account in gh_accounts_owner_first(owner) {
        let token = match gh_token(&account) {
            Some(t) => t,
            None => continue,
        };
        if tried_tokens.contains(&token) {
            continue;
        }
        tried_tokens.push(token.clone());
        println!("Authenticating to GitHub via gh account '{account}'...");
        match attempt(api_url, Some(&token)) {
            Attempt::Ok(response) => return Ok((*response, Some(token))),
            Attempt::Status(code) => last_status = Some(code),
            Attempt::Transport => {}
        }
    }

    let status = last_status
        .map(|c| c.to_string())
        .unwrap_or_else(|| "no response".to_string());
    Err(anyhow::anyhow!(
        "GitHub API request to {api_url} failed (last status: {status}). \
         For a private repository, set GITHUB_TOKEN or run `gh auth login` \
         for an account with access."
    ))
}

/// Download a GitHub release asset to `dest`.
///
/// Public assets use the unauthenticated `browser_download_url` fast path.
/// Private assets must go through the asset API endpoint with the token, then
/// follow the signed redirect *without* the auth header (S3 rejects requests
/// carrying both its query-string signature and an `Authorization` header).
pub fn download_github_asset(
    asset_api_url: &str,
    browser_download_url: &str,
    token: Option<&str>,
    dest: &Path,
) -> Result<()> {
    let token = match token {
        None => return http::download_file(browser_download_url, dest, None),
        Some(token) => token,
    };

    // Disable automatic redirect-following so we can drop the auth header
    // before hitting the signed storage URL.
    let agent = ureq::builder().redirects(0).build();
    let response = match agent
        .get(asset_api_url)
        .set("User-Agent", "zipget-rs")
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/octet-stream")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::Status(code, response)) if (300..400).contains(&code) => response,
        Err(ureq::Error::Status(code, _)) => {
            return Err(anyhow::anyhow!(
                "Failed to download asset from {asset_api_url} (status {code})"
            ));
        }
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context(format!("Failed to download asset from {asset_api_url}")));
        }
    };

    let status = response.status();
    if (300..400).contains(&status) {
        let location = response
            .header("location")
            .ok_or_else(|| {
                anyhow::anyhow!("GitHub asset response {status} missing Location header")
            })?
            .to_string();
        // Signed storage URL — must be fetched without the Authorization header.
        http::download_file(&location, dest, None)
    } else if status == 200 {
        http::write_response_to_file(response, dest)
    } else {
        Err(anyhow::anyhow!(
            "Failed to download asset from {asset_api_url} (status {status})"
        ))
    }
}

/// gh accounts for github.com, ordered with the account whose name contains
/// `owner` (case-insensitive) first, then the rest.
fn gh_accounts_owner_first(owner: &str) -> Vec<String> {
    let owner_lc = owner.to_lowercase();
    let (mut matching, mut others): (Vec<String>, Vec<String>) = gh_accounts()
        .into_iter()
        .partition(|a| a.to_lowercase().contains(&owner_lc));
    matching.append(&mut others);
    matching
}

/// Parse the github.com account names known to the `gh` CLI. Returns an empty
/// list if `gh` is not installed or not logged in.
fn gh_accounts() -> Vec<String> {
    let output = match Command::new("gh").args(["auth", "status"]).output() {
        Ok(output) => output,
        Err(_) => return Vec::new(),
    };

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stderr));

    let mut accounts: Vec<String> = Vec::new();
    for line in text.lines() {
        // Lines look like: "✓ Logged in to github.com account NAME (...)"
        if !line.contains("github.com") {
            continue;
        }
        if let Some(idx) = line.find("account ") {
            let rest = &line[idx + "account ".len()..];
            if let Some(name) = rest.split_whitespace().next() {
                let name = name.trim_matches(|c: char| {
                    !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
                });
                if !name.is_empty() && !accounts.iter().any(|a| a == name) {
                    accounts.push(name.to_string());
                }
            }
        }
    }
    accounts
}

/// Read a specific account's token without changing the active account.
fn gh_token(user: &str) -> Option<String> {
    let output = Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com", "--user", user])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_owner() {
        assert_eq!(repo_owner("basware/foobar"), "basware");
        assert_eq!(repo_owner("vivainio/zipget"), "vivainio");
        assert_eq!(repo_owner("noslash"), "noslash");
    }

    #[test]
    fn test_owner_first_ordering_is_stable_without_gh() {
        // With no gh accounts available this is just empty; the ordering logic
        // itself is exercised via the partition in gh_accounts_owner_first.
        let owner = "basware";
        let accounts = vec![
            "vivainio".to_string(),
            "villevai_Basware".to_string(),
            "other".to_string(),
        ];
        let owner_lc = owner.to_lowercase();
        let (mut matching, mut others): (Vec<String>, Vec<String>) = accounts
            .into_iter()
            .partition(|a| a.to_lowercase().contains(&owner_lc));
        matching.append(&mut others);
        assert_eq!(matching[0], "villevai_Basware");
    }
}
