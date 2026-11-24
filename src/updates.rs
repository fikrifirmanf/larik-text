use anyhow::{Context as _, Result, ensure};
use gpui::{App, PromptLevel};
use semver::Version;
use sha2::{Digest as _, Sha256};
use std::{collections::HashMap, io::Read as _, path::Path, time::Duration};
use url::Url;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(serde::Deserialize)]
struct Release {
    version: String,
    url: String,
    /// Optional per-platform packages keyed like `macos-aarch64`, `windows-x86_64`, `linux-aarch64`.
    #[serde(default)]
    downloads: HashMap<String, Package>,
}

#[derive(serde::Deserialize)]
struct Package {
    url: String,
    sha256: String,
}

pub struct Update {
    version: Version,
    page: Url,
    download: Option<Download>,
}

struct Download {
    url: Url,
    sha256: String,
}

enum Handoff {
    /// A helper waits for this process to exit, swaps the app, and relaunches it.
    #[cfg_attr(windows, allow(dead_code))]
    Restart,
    /// The package was handed to the system installer.
    #[cfg_attr(not(windows), allow(dead_code))]
    Opened,
}

pub fn about(cx: &mut App) {
    notice(
        cx,
        "Larik Text",
        &format!("Version {VERSION}\nA native tabbed editor for text, Markdown, and code."),
    );
}

fn feed_url() -> Option<String> {
    std::env::var("LARIK_TEXT_UPDATE_FEED")
        .ok()
        .or_else(|| option_env!("LARIK_TEXT_UPDATE_FEED").map(str::to_owned))
        .filter(|feed| !feed.is_empty())
}

/// Checks quietly a few seconds after launch and only speaks up when an update exists.
pub fn check_on_launch(cx: &mut App) {
    let Some(feed) = feed_url() else { return };
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs(4)).await;
        let task = cx.background_executor().spawn(async move { latest(&feed) });
        if let Ok(Some(update)) = task.await {
            let _ = cx.update(|cx| offer(cx, update));
        }
    })
    .detach();
}

pub fn check(cx: &mut App) {
    let Some(feed) = feed_url() else {
        notice(
            cx,
            "No update feed configured",
            "This local build has no release server to check.",
        );
        return;
    };

    let task = cx.background_executor().spawn(async move { latest(&feed) });
    cx.spawn(async move |cx| {
        let result = task.await;
        let _ = cx.update(|cx| match result {
            Ok(Some(update)) => offer(cx, update),
            Ok(None) => notice(
                cx,
                "Larik Text is up to date",
                &format!("You have version {VERSION}."),
            ),
            Err(error) => notice(cx, "Couldn’t check for updates", &error.to_string()),
        });
    })
    .detach();
}

fn offer(cx: &mut App, update: Update) {
    let title = format!("Larik Text {} is available", update.version);
    let can_install = update.download.is_some();
    let detail = if can_install {
        format!(
            "You have version {VERSION}. Download and install the update now, and Larik Text will restart. Your open tabs and drafts are kept."
        )
    } else {
        format!("You have version {VERSION}. Open the release page to download it.")
    };
    let Some(handle) = crate::dialog_window(None, cx) else {
        return;
    };
    let _ = handle.update(cx, |_, window, cx| {
        let buttons: &[&str] = if can_install {
            &["Download and install", "Later"]
        } else {
            &["Open release page", "Later"]
        };
        let response = window.prompt(PromptLevel::Info, &title, Some(&detail), buttons, cx);
        window
            .spawn(cx, async move |cx| {
                if response.await != Ok(0) {
                    return;
                }
                match update.download {
                    None => {
                        let _ = cx.update(|_, cx| cx.open_url(update.page.as_str()));
                    }
                    Some(download) => {
                        let task = cx
                            .background_executor()
                            .spawn(async move { install(&download) });
                        match task.await {
                            Ok(Handoff::Restart) => {
                                let _ = cx.update(|_, cx| crate::quit(cx));
                            }
                            Ok(Handoff::Opened) => {}
                            Err(error) => {
                                let _ = cx.update(|_, cx| {
                                    notice(cx, "Couldn’t install the update", &format!("{error:#}"))
                                });
                            }
                        }
                    }
                }
            })
            .detach();
    });
}

fn https_url(value: &str) -> Result<Url> {
    let url = Url::parse(value).context("The update URL is invalid.")?;
    ensure!(
        url.scheme() == "https"
            && url.host().is_some()
            && url.username().is_empty()
            && url.password().is_none(),
        "The update URL must use HTTPS without credentials."
    );
    Ok(url)
}

/// GitHub serves release assets through redirects; follow a few, but only over HTTPS.
fn https_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.url().scheme() == "https" && attempt.previous().len() < 5 {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

fn latest(feed: &str) -> Result<Option<Update>> {
    let feed = https_url(feed)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(https_redirects())
        .build()?;
    let response = client
        .get(feed)
        .send()
        .map_err(|_| anyhow::anyhow!("The update feed could not be reached."))?;
    ensure!(
        response.status().is_success(),
        "The update feed returned HTTP {}.",
        response.status().as_u16()
    );
    let mut body = Vec::new();
    response.take(65_537).read_to_end(&mut body)?;
    ensure!(body.len() <= 65_536, "The update feed is too large.");
    parse_release(&body)
}

fn parse_release(body: &[u8]) -> Result<Option<Update>> {
    let release: Release =
        serde_json::from_slice(body).context("The update feed did not describe a release.")?;
    let version = Version::parse(&release.version).context("The release version is invalid.")?;
    let page = https_url(&release.url)?;
    if version <= Version::parse(VERSION)? {
        return Ok(None);
    }
    // A package without a usable checksum is ignored, so the update falls back to the release page.
    let download = release.downloads.get(&platform()).and_then(|package| {
        let sha256 = package.sha256.to_ascii_lowercase();
        (sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| https_url(&package.url).ok())
            .flatten()
            .map(|url| Download { url, sha256 })
    });
    Ok(Some(Update {
        version,
        page,
        download,
    }))
}

fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn install(download: &Download) -> Result<Handoff> {
    let stage = dirs::cache_dir()
        .context("There is no cache folder for the download.")?
        .join("Larik Text/update");
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage)?;
    let name = download
        .url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|name| !name.is_empty() && !name.contains(['/', '\\']))
        .unwrap_or("larik-text-update")
        .to_owned();
    let package = stage.join(&name);
    fetch(download, &package)?;
    apply(&package, &stage)
}

fn fetch(download: &Download, destination: &Path) -> Result<()> {
    const LIMIT: u64 = 400 * 1024 * 1024;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .redirect(https_redirects())
        .build()?;
    let response = client
        .get(download.url.clone())
        .send()
        .map_err(|_| anyhow::anyhow!("The download could not be started."))?;
    ensure!(
        response.status().is_success(),
        "The download server returned HTTP {}.",
        response.status().as_u16()
    );
    let mut reader = response.take(LIMIT + 1);
    let mut file = std::fs::File::create(destination)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    let mut total = 0u64;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        ensure!(total <= LIMIT, "The download is too large.");
        hasher.update(&buffer[..read]);
        std::io::Write::write_all(&mut file, &buffer[..read])?;
    }
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    ensure!(
        actual == download.sha256,
        "The downloaded file did not match its checksum, so it was not installed."
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn apply(package: &Path, stage: &Path) -> Result<Handoff> {
    use std::process::Command;
    let exe = std::env::current_exe()?;
    let bundle = exe
        .ancestors()
        .find(|path| path.extension().is_some_and(|ext| ext == "app"))
        .context("Larik Text isn’t running from an app bundle, so it can’t replace itself.")?
        .to_owned();
    let unpacked = stage.join("unpacked");
    let status = Command::new("/usr/bin/ditto")
        .args(["-x", "-k"])
        .arg(package)
        .arg(&unpacked)
        .status()?;
    ensure!(status.success(), "The update package couldn’t be unpacked.");
    let new_bundle = std::fs::read_dir(&unpacked)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "app"))
        .context("The update package doesn’t contain an app.")?;
    let script = r#"
        while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
        old="$2.previous"
        rm -rf "$old"
        if mv "$2" "$old"; then
            if mv "$3" "$2"; then rm -rf "$old"; else mv "$old" "$2"; fi
        fi
        open "$2"
    "#;
    Command::new("/bin/sh")
        .args(["-c", script, "sh"])
        .arg(std::process::id().to_string())
        .arg(&bundle)
        .arg(&new_bundle)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("The installer helper couldn’t start.")?;
    Ok(Handoff::Restart)
}

/// Finds the `larik-text` binary at the top of the archive or one folder down.
#[cfg(target_os = "linux")]
fn find_executable(dir: &Path, depth: u8) -> Option<std::path::PathBuf> {
    let entries: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .collect();
    entries
        .iter()
        .map(|entry| entry.path())
        .find(|path| path.is_file() && path.file_name().is_some_and(|name| name == "larik-text"))
        .or_else(|| {
            (depth < 1)
                .then(|| {
                    entries
                        .iter()
                        .map(|entry| entry.path())
                        .filter(|path| path.is_dir())
                        .find_map(|path| find_executable(&path, depth + 1))
                })
                .flatten()
        })
}

#[cfg(target_os = "linux")]
fn apply(package: &Path, stage: &Path) -> Result<Handoff> {
    use std::{os::unix::fs::PermissionsExt as _, process::Command};
    let exe = std::env::current_exe()?;
    let unpacked = stage.join("unpacked");
    std::fs::create_dir_all(&unpacked)?;
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(package)
        .arg("-C")
        .arg(&unpacked)
        .status()?;
    ensure!(status.success(), "The update package couldn’t be unpacked.");
    let new_exe =
        find_executable(&unpacked, 0).context("The update package doesn’t contain Larik Text.")?;
    std::fs::set_permissions(&new_exe, std::fs::Permissions::from_mode(0o755))?;
    let script = r#"
        while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
        cp "$3" "$2.new" && mv "$2.new" "$2"
        exec "$2"
    "#;
    Command::new("/bin/sh")
        .args(["-c", script, "sh"])
        .arg(std::process::id().to_string())
        .arg(&exe)
        .arg(&new_exe)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("The installer helper couldn’t start.")?;
    Ok(Handoff::Restart)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn apply(package: &Path, _stage: &Path) -> Result<Handoff> {
    // Windows: hand the downloaded package to the shell, which runs the installer or opens the archive.
    std::process::Command::new("explorer")
        .arg(package)
        .spawn()
        .context("The downloaded package couldn’t be opened.")?;
    Ok(Handoff::Opened)
}

fn notice(cx: &mut App, title: &str, detail: &str) {
    if let Some(handle) = crate::dialog_window(None, cx) {
        let _ = handle.update(cx, |_, window, cx| {
            let response = window.prompt(PromptLevel::Info, title, Some(detail), &["OK"], cx);
            window
                .spawn(cx, async move |_| {
                    let _ = response.await;
                })
                .detach();
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn compares_versions_and_requires_https_release_links() {
        let update =
            super::parse_release(br#"{"version":"2.10.0","url":"https://example.com/release"}"#)
                .unwrap()
                .unwrap();
        assert_eq!(update.version.to_string(), "2.10.0");
        assert!(update.download.is_none());
        assert!(
            super::parse_release(br#"{"version":"0.1.0","url":"https://example.com/old"}"#)
                .unwrap()
                .is_none()
        );
        assert!(
            super::parse_release(br#"{"version":"2.10.0","url":"http://example.com/unsafe"}"#)
                .is_err()
        );
    }

    #[test]
    fn installs_only_checksummed_https_packages_for_this_platform() {
        let key = super::platform();
        let sha = "a".repeat(64);
        let feed = |package: &str| {
            format!(
                r#"{{"version":"9.0.0","url":"https://example.com/r","downloads":{{"{key}":{package},"other-os":{{"url":"https://example.com/o","sha256":"{sha}"}}}}}}"#
            )
        };
        let good = feed(&format!(
            r#"{{"url":"https://example.com/app.zip","sha256":"{sha}"}}"#
        ));
        let update = super::parse_release(good.as_bytes()).unwrap().unwrap();
        assert_eq!(
            update.download.unwrap().url.as_str(),
            "https://example.com/app.zip"
        );

        let no_checksum = feed(r#"{"url":"https://example.com/app.zip","sha256":"short"}"#);
        assert!(
            super::parse_release(no_checksum.as_bytes())
                .unwrap()
                .unwrap()
                .download
                .is_none()
        );
        let insecure = feed(&format!(
            r#"{{"url":"http://example.com/app.zip","sha256":"{sha}"}}"#
        ));
        assert!(
            super::parse_release(insecure.as_bytes())
                .unwrap()
                .unwrap()
                .download
                .is_none()
        );
    }
}
