use crate::client::ScrapedPages;
use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Persist the raw modem pages for a structurally invalid scrape so firmware
/// changes can be diagnosed later. Files are written with restrictive
/// permissions and token-like values are redacted.
pub fn write_failed_scrape(dir: &Path, pages: &ScrapedPages, error: &str) -> Result<PathBuf> {
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create debug directory {}", dir.display()))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let out_dir = dir.join(format!("failed-scrape-{}", stamp));
    create_private_dir(&out_dir)?;

    for (name, body) in [
        ("status_cgi.html", &pages.status),
        ("vers_cgi.html", &pages.vers),
        ("dhcp_cgi.html", &pages.dhcp),
        ("qos_cgi.html", &pages.qos),
        ("cm_state_cgi.html", &pages.cm_state),
        ("event_cgi.html", &pages.event),
        ("config_params_cgi.html", &pages.config_params),
        ("product_cgi.html", &pages.product),
        ("dhcp_cmLog_cgi.html", &pages.dhcp_log),
        ("dsx_cgi.html", &pages.dsx),
    ] {
        write_private(&out_dir.join(name), &redact(body))?;
    }
    write_private(&out_dir.join("parse-error.txt"), &redact(error))?;
    prune_old_bundles(dir, 10)?;
    Ok(out_dir)
}

fn prune_old_bundles(dir: &Path, keep: usize) -> Result<()> {
    let mut bundles: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("failed-scrape-"))
        })
        .collect();
    bundles.sort();
    let remove_count = bundles.len().saturating_sub(keep);
    for path in bundles.into_iter().take(remove_count) {
        let _ = fs::remove_dir_all(path);
    }
    Ok(())
}

pub fn redact(input: &str) -> String {
    let patterns = [
        (
            r#"(?i)(csrf_token["'\s:=]+)([A-Za-z0-9._~+/=-]+)"#,
            "${1}[REDACTED]",
        ),
        (r#"(?i)(password["'\s:=]+)([^&\s<"']+)"#, "${1}[REDACTED]"),
        (
            r#"(?i)((?:session|sid|token)[A-Za-z0-9_-]*["'\s:=]+)([A-Za-z0-9._~+/=-]+)"#,
            "${1}[REDACTED]",
        ),
    ];

    let mut output = input.to_string();
    for (pattern, replacement) in patterns {
        if let Ok(re) = Regex::new(pattern) {
            output = re.replace_all(&output, replacement).into_owned();
        }
    }
    output
}

fn write_private(path: &Path, contents: &str) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn create_private_dir(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_tokens_and_passwords() {
        let html = r#"csrf_token="abc123"; password=hunter2 session_id=deadbeef"#;
        let redacted = redact(html);
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("hunter2"));
        assert!(!redacted.contains("deadbeef"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn writes_restricted_debug_bundle() {
        let pages = ScrapedPages {
            status: "csrf_token=secret".into(),
            vers: "MODEL: CM3500B".into(),
            dhcp: String::new(),
            qos: String::new(),
            cm_state: String::new(),
            event: String::new(),
            config_params: String::new(),
            product: String::new(),
            dhcp_log: String::new(),
            dsx: String::new(),
        };
        let root = std::env::temp_dir().join(format!("cm3500-debug-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let out = write_failed_scrape(&root, &pages, "status missing").unwrap();
        assert!(out.join("status_cgi.html").exists());
        assert!(out.join("parse-error.txt").exists());
        let status = fs::read_to_string(out.join("status_cgi.html")).unwrap();
        assert!(!status.contains("secret"));
        let _ = fs::remove_dir_all(&root);
    }
}
