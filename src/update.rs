//! Self-update: at startup, compare this version with the latest release and, if that
//! is newer, download its binary over this one and restart.
//!
//! Only copies the user installed themselves update this way. A build run from a source
//! checkout, or a copy owned by a package manager (Homebrew, pacman), is left alone.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::store::{checkout_root, real_exe};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The commit this binary was built from, for `--version`; empty if unknown, `-dirty`
/// if the tree had local changes.
pub const COMMIT: &str = env!("WORDL_COMMIT");

/// Where the latest release's files are: `VERSION`, `SHA256SUMS` and one binary per
/// platform. No API is involved, so there is no rate limit to run into.
const RELEASES: &str = "https://github.com/anwarahmed/wordl/releases/latest/download";
/// Points the updater somewhere else; `file://` works, which is how it is tested.
const URL_ENV: &str = "WORDL_RELEASE_URL";
/// Set on the restarted process so a failed or raced update can't loop; also the
/// switch to skip the check for one run.
const SKIP_ENV: &str = "WORDL_NO_UPDATE";
/// A package that owns its copy installs this file, relative to the directory of the
/// binary, naming itself and how to upgrade. Homebrew's formula and the AUR package
/// both do; the game then leaves updating to them.
const MANAGED_BY: &str = "../share/wordl/managed-by";
/// Starting the game must not hang on a bad connection.
const CHECK_TIMEOUT: Duration = Duration::from_secs(3);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

type Version = (u32, u32, u32);

/// Parses `1.2.3` or `v1.2.3`.
fn parse_version(s: &str) -> Option<Version> {
    let mut parts = s.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u32>().ok());
    match (parts.next()??, parts.next()??, parts.next()??, parts.next()) {
        (a, b, c, None) => Some((a, b, c)),
        _ => None,
    }
}

/// The release asset built for this platform, if there is one.
fn asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("wordl-x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => Some("wordl-aarch64-unknown-linux-musl"),
        ("macos", "aarch64") => Some("wordl-aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("wordl-x86_64-apple-darwin"),
        _ => None,
    }
}

fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".wordl-write-test-{}", std::process::id()));
    let ok = fs::write(&probe, b"").is_ok();
    let _ = fs::remove_file(probe);
    ok
}

/// The package that owns the copy at `exe`, as its marker file words it
/// ("Homebrew; use brew upgrade wordl"), if one does.
fn managed_by(exe: &Path) -> Option<String> {
    let marker = fs::read_to_string(exe.parent()?.join(MANAGED_BY)).ok()?;
    marker.lines().next().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string)
}

/// Why this copy doesn't update itself, if it doesn't. `asked` is for `wordl update`:
/// asking explicitly overrides the two off switches, and nothing else.
fn skip_reason(enabled: bool, asked: bool, exe: &Path) -> Option<String> {
    let fixed = |s: &str| Some(s.to_string());
    if !asked && std::env::var_os(SKIP_ENV).is_some_and(|v| !v.is_empty()) {
        fixed("WORDL_NO_UPDATE is set")
    } else if !asked && !enabled {
        fixed("turned off; 'wordl update on' turns it back on")
    } else if checkout_root().is_some() {
        fixed("running from a source checkout; use git pull and cargo build")
    } else if let Some(owner) = managed_by(exe) {
        Some(format!("installed with {owner}"))
    } else if exe.components().any(|c| c.as_os_str() == "Cellar") {
        // In case a formula ever ships without the marker. `exe` has symlinks resolved,
        // so this sees through the link Homebrew puts in its bin directory.
        fixed("installed with Homebrew; use brew upgrade wordl")
    } else if asset_name().is_none() {
        fixed("no prebuilt binary for this platform; rebuild from source to update")
    } else if !exe.parent().is_some_and(writable) {
        fixed("its directory is not writable, so a package manager probably owns it; update it the way you installed it")
    } else {
        None
    }
}

fn base() -> String {
    std::env::var(URL_ENV).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| RELEASES.to_string())
}

fn get(url: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    if let Some(path) = url.strip_prefix("file://") {
        return fs::read(path).map_err(|e| e.to_string());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(timeout)).build().into();
    let mut res = agent.get(url).header("User-Agent", "wordl").call().map_err(|e| e.to_string())?;
    res.body_mut().with_config().limit(64 << 20).read_to_vec().map_err(|e| e.to_string())
}

/// The newest released version, as numbers and as published.
fn latest_release() -> Result<(Version, String), String> {
    let body = get(&format!("{}/VERSION", base()), CHECK_TIMEOUT)?;
    let text = String::from_utf8_lossy(&body).trim().trim_start_matches('v').to_string();
    Ok((parse_version(&text).ok_or_else(|| format!("unrecognized release version {text:?}"))?, text))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Finds a file's checksum in `sha256sum` output (`<hex>  <name>` per line).
fn expected_sum<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|l| {
        let (sum, file) = l.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then_some(sum)
    })
}

/// Downloads the latest release's binary for this platform, checks it against the
/// release's checksums, and swaps it in for `exe`.
fn install(exe: &Path) -> Result<(), String> {
    let name = asset_name().ok_or("no prebuilt binary for this platform")?;
    let base = base();
    let sums = get(&format!("{base}/SHA256SUMS"), DOWNLOAD_TIMEOUT).map_err(|e| format!("could not download checksums: {e}"))?;
    let sums = String::from_utf8_lossy(&sums);
    let want = expected_sum(&sums, name).ok_or_else(|| format!("the release has no checksum for {name}"))?;
    let binary = get(&format!("{base}/{name}"), DOWNLOAD_TIMEOUT).map_err(|e| format!("could not download {name}: {e}"))?;
    let got = sha256_hex(&binary);
    if !got.eq_ignore_ascii_case(want) {
        return Err(format!("checksum mismatch for {name} (expected {want}, got {got})"));
    }
    // Written beside the target and renamed over it: the swap is atomic, and replacing
    // a running program's file this way is safe.
    let staged = exe.with_extension("new");
    fs::write(&staged, &binary).and_then(|()| fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))).and_then(|()| fs::rename(&staged, exe)).map_err(
        |e| {
            let _ = fs::remove_file(&staged);
            format!("could not replace {}: {e}", exe.display())
        },
    )
}

/// Resolved before any replacement; afterwards the running image has no path. The real
/// file, not a link to it: the checks in `skip_reason` are about where it is installed,
/// and replacing a link would leave the installed file behind (and break `brew upgrade`).
fn current_exe() -> Result<PathBuf, String> {
    real_exe().map_err(|e| format!("cannot tell where wordl is installed: {e}"))
}

/// Called before the game starts. Updates and restarts when a newer release exists;
/// otherwise, or on any failure, returns so the current version runs.
pub fn before_start(enabled: bool) {
    let Ok(exe) = current_exe() else { return };
    if skip_reason(enabled, false, &exe).is_some() {
        return;
    }
    // A failure here is usually just being offline: not worth a word on screen.
    let Ok((latest, version)) = latest_release() else { return };
    if parse_version(VERSION).is_none_or(|current| latest <= current) {
        return;
    }
    println!("Updating wordl {VERSION} -> {version}");
    match install(&exe) {
        Ok(()) => {
            let err = Command::new(&exe).args(std::env::args_os().skip(1)).env(SKIP_ENV, "1").exec();
            eprintln!("wordl was updated; start it again to use the new version. ({err})");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("wordl: update failed ({e}); starting the current version.");
            std::thread::sleep(Duration::from_secs(2));
        }
    }
}

/// `wordl update`: the same check on demand, reporting what happened.
pub fn command() -> Result<(), String> {
    let exe = current_exe()?;
    if let Some(reason) = skip_reason(true, true, &exe) {
        return Err(format!("this copy can't update itself: {reason}"));
    }
    let (latest, version) = latest_release().map_err(|e| format!("could not check for updates: {e}"))?;
    if parse_version(VERSION).is_none_or(|current| latest <= current) {
        println!("wordl {VERSION} is up to date (latest release is {version}).");
        return Ok(());
    }
    println!("Updating wordl {VERSION} -> {version}");
    install(&exe).map_err(|e| format!("update failed: {e}"))?;
    println!("Updated to {version}.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_orders_versions() {
        assert_eq!(parse_version("v0.2.0"), Some((0, 2, 0)));
        assert_eq!(parse_version("1.10.3\n"), Some((1, 10, 3)));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("v1.2.x"), None);
        assert!(parse_version("0.10.0") > parse_version("0.9.9"));
        assert!(parse_version(VERSION).is_some());
    }

    #[test]
    fn finds_checksum_by_exact_name() {
        let sums = "aaa  wordl-x86_64-unknown-linux-musl\nbbb *wordl-aarch64-apple-darwin\n";
        assert_eq!(expected_sum(sums, "wordl-aarch64-apple-darwin"), Some("bbb"));
        assert_eq!(expected_sum(sums, "wordl-x86_64-unknown-linux-musl"), Some("aaa"));
        assert_eq!(expected_sum(sums, "wordl-x86_64"), None);
    }

    #[test]
    fn hashes_like_sha256sum() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn a_marker_file_names_the_package_that_owns_a_copy() {
        let root = std::env::temp_dir().join(format!("wordl-test-{}-managed", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("bin")).unwrap();
        let exe = root.join("bin/wordl");
        assert_eq!(managed_by(&exe), None);
        fs::create_dir_all(root.join("share/wordl")).unwrap();
        fs::write(root.join("share/wordl/managed-by"), "Homebrew; use brew upgrade wordl\n").unwrap();
        assert_eq!(managed_by(&exe).as_deref(), Some("Homebrew; use brew upgrade wordl"));
        fs::write(root.join("share/wordl/managed-by"), "\n").unwrap();
        assert_eq!(managed_by(&exe), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn the_off_switch_stops_the_startup_check_but_not_a_request() {
        let root = std::env::temp_dir().join(format!("wordl-test-{}-switch", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("share/wordl")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("share/wordl/managed-by"), "pacman; update the wordl-bin package\n").unwrap();
        let exe = root.join("bin/wordl");
        assert_eq!(skip_reason(false, false, &exe).as_deref(), Some("turned off; 'wordl update on' turns it back on"));
        // Asked for explicitly, the switch is ignored and the real reason is given.
        assert_eq!(skip_reason(false, true, &exe).as_deref(), Some("installed with pacman; update the wordl-bin package"));
        let _ = fs::remove_dir_all(root);
    }
}
