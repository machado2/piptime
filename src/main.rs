use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use colored::*;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, args_conflicts_with_subcommands = true)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// The package manager to use
    #[arg(value_enum)]
    manager: Option<Manager>,

    /// The cutoff date (YYYY-MM-DD)
    date: Option<String>,

    /// List of packages to check
    packages: Vec<String>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Find versions that were "latest" during an anchor package-version window
    /// (from the anchor release time until the next release time).
    Overlap(OverlapArgs),
}

#[derive(Parser, Debug)]
struct OverlapArgs {
    /// The package manager to use
    #[arg(value_enum)]
    manager: Manager,

    /// Anchor spec in pip-style format: <package>==<version>
    anchor: String,

    /// List of packages to check
    #[arg(required = true)]
    packages: Vec<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Manager {
    Pip,
    Npm,
    Cargo,
    Gem,
    Composer,
}

struct PackageVersion {
    version: String,
    date: DateTime<Utc>,
}

struct WindowedVersion {
    version: String,
    overlap_start: DateTime<Utc>,
    overlap_end: DateTime<Utc>,
}

fn main() -> Result<()> {
    // Enable color support on Windows
    #[cfg(windows)]
    let _ = colored::control::set_virtual_terminal(true);

    let args = Args::parse();

    if let Some(command) = args.command {
        return match command {
            Command::Overlap(o) => run_overlap(o, args.verbose),
        };
    }

    let manager = args
        .manager
        .context("Missing MANAGER argument (or use a subcommand)")?;
    let date = args
        .date
        .context("Missing DATE argument (or use a subcommand)")?;

    if args.packages.is_empty() {
        return Err(anyhow::anyhow!(
            "Missing PACKAGES argument(s) (or use a subcommand)"
        ));
    }

    // Parse date
    let naive_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .context("Invalid date format. Use YYYY-MM-DD")?;
    // Set time to end of day to include releases on that day
    let target_date = naive_date.and_hms_opt(23, 59, 59).unwrap().and_utc();

    println!(
        "--- Searching for {} packages up to {} ---",
        format!("{:?}", manager).yellow(),
        target_date.date_naive().to_string().yellow()
    );

    let client = Client::builder()
        .user_agent("pkgtime/1.0 (pkgtime-tool)")
        .build()?;

    let mut install_cmds = Vec::new();
    let mut errors = Vec::new();

    for pkg in &args.packages {
        match find_version(&client, manager, pkg, target_date, args.verbose) {
            Ok(Some(v)) => {
                println!(
                    "✅ {}: {} (from {})",
                    pkg.green(),
                    v.version.bold(),
                    v.date.date_naive()
                );

                let cmd = match manager {
                    Manager::Pip => format!("{}=={}", pkg, v.version),
                    Manager::Npm => format!("{}@{}", pkg, v.version),
                    Manager::Cargo => format!("{} = \"={}\"", pkg, v.version),
                    Manager::Gem => format!("gem '{}', '{}'", pkg, v.version),
                    Manager::Composer => format!("{}:{}", pkg, v.version),
                };
                install_cmds.push(cmd);
            }
            Ok(None) => {
                let msg = "No version found before the specified date";
                println!("❌ {}: {}", pkg.red(), msg);
                errors.push(format!("{}: {}", pkg, msg));
            }
            Err(e) => {
                println!("❌ {}: {}", pkg.red(), e);
                errors.push(format!("{}: {}", pkg, e));
            }
        }
    }

    println!("{}", "-".repeat(60));

    if !install_cmds.is_empty() {
        print_install_instructions(manager, &install_cmds);
    }

    if !errors.is_empty() {
        println!("\n{}", "Attention to errors:".yellow());
        for err in errors {
            println!(" - {}", err);
        }
    }

    Ok(())
}

fn run_overlap(args: OverlapArgs, verbose: bool) -> Result<()> {
    if args.manager != Manager::Pip {
        return Err(anyhow::anyhow!(
            "The overlap command is currently only supported for 'pip'"
        ));
    }

    let (anchor_pkg, anchor_ver) = parse_pip_spec(&args.anchor)?;

    let client = Client::builder()
        .user_agent("pkgtime/1.0 (pkgtime-tool)")
        .build()?;

    let anchor_releases = fetch_pip_releases(&client, &anchor_pkg, verbose).with_context(|| {
        format!(
            "Failed to fetch releases for anchor package '{}'",
            anchor_pkg
        )
    })?;

    let (window_start, window_end) = pip_anchor_window(&anchor_pkg, &anchor_ver, &anchor_releases)?;

    println!(
        "--- Overlap window for {} ---",
        format!("{}=={}", anchor_pkg, anchor_ver).yellow()
    );
    println!(
        "Window: {} -> {}",
        window_start.to_rfc3339().yellow(),
        window_end.to_rfc3339().yellow()
    );
    println!("{}", "-".repeat(60));

    let mut errors = Vec::new();

    for pkg in &args.packages {
        match fetch_pip_releases(&client, pkg, verbose) {
            Ok(releases) => {
                let overlaps = versions_overlapping_window(&releases, window_start, window_end);
                if overlaps.is_empty() {
                    println!(
                        "{}: {}",
                        pkg.yellow(),
                        "no overlapping latest versions".dimmed()
                    );
                    continue;
                }

                let parts: Vec<String> = overlaps
                    .into_iter()
                    .map(|o| {
                        format!(
                            "{} ({}..{})",
                            o.version.bold(),
                            o.overlap_start.date_naive(),
                            o.overlap_end.date_naive()
                        )
                    })
                    .collect();

                println!("{}: {}", pkg.green(), parts.join(", "));
            }
            Err(e) => {
                println!("❌ {}: {}", pkg.red(), e);
                errors.push(format!("{}: {}", pkg, e));
            }
        }
    }

    if !errors.is_empty() {
        println!("\n{}", "Attention to errors:".yellow());
        for err in errors {
            println!(" - {}", err);
        }
    }

    Ok(())
}

fn print_install_instructions(manager: Manager, cmds: &[String]) {
    println!("Copy and paste into your configuration:");
    println!();
    match manager {
        Manager::Pip => println!("{}pip install {}{}", "\x1b[92m", cmds.join(" "), "\x1b[0m"),
        Manager::Npm => println!("{}npm install {}{}", "\x1b[92m", cmds.join(" "), "\x1b[0m"),
        Manager::Cargo => {
            println!("{}# Cargo.toml dependencies:{}", "\x1b[92m", "\x1b[0m");
            for cmd in cmds {
                println!("{}{}{}", "\x1b[92m", cmd, "\x1b[0m");
            }
        }
        Manager::Gem => {
            println!("{}# Gemfile:{}", "\x1b[92m", "\x1b[0m");
            for cmd in cmds {
                println!("{}{}{}", "\x1b[92m", cmd, "\x1b[0m");
            }
        }
        Manager::Composer => {
            println!(
                "{}composer require {}{}",
                "\x1b[92m",
                cmds.join(" "),
                "\x1b[0m"
            );
        }
    }
    println!();
}

fn find_version(
    client: &Client,
    manager: Manager,
    pkg: &str,
    target_date: DateTime<Utc>,
    verbose: bool,
) -> Result<Option<PackageVersion>> {
    match manager {
        Manager::Pip => find_pip(client, pkg, target_date, verbose),
        Manager::Npm => find_npm(client, pkg, target_date, verbose),
        Manager::Cargo => find_cargo(client, pkg, target_date, verbose),
        Manager::Gem => find_gem(client, pkg, target_date, verbose),
        Manager::Composer => find_composer(client, pkg, target_date, verbose),
    }
}

// --- PIP Strategy ---
#[derive(Deserialize)]
struct PipReleaseFile {
    #[serde(default)]
    upload_time: Option<String>,
    #[serde(default)]
    upload_time_iso_8601: Option<String>,
}
#[derive(Deserialize)]
struct PipData {
    releases: HashMap<String, Vec<PipReleaseFile>>,
}

fn parse_pip_spec(spec: &str) -> Result<(String, String)> {
    let (name, version) = spec
        .split_once("==")
        .context("Anchor must be in format <package>==<version>")?;

    if name.trim().is_empty() || version.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "Anchor must be in format <package>==<version>"
        ));
    }

    Ok((name.trim().to_string(), version.trim().to_string()))
}

fn pip_file_upload_time(file: &PipReleaseFile) -> Option<DateTime<Utc>> {
    if let Some(ts) = &file.upload_time_iso_8601 {
        if let Ok(date) = DateTime::parse_from_rfc3339(ts) {
            return Some(date.with_timezone(&Utc));
        }
    }

    if let Some(ts) = &file.upload_time {
        // "2019-05-16T17:21:44" (typically UTC but without timezone info)
        let layout = "%Y-%m-%dT%H:%M:%S";
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(ts, layout) {
            return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
        }
    }

    None
}

fn fetch_pip_releases(client: &Client, pkg: &str, verbose: bool) -> Result<Vec<PackageVersion>> {
    let url = format!("https://pypi.org/pypi/{}/json", pkg);
    if verbose {
        println!(" -> Fetching {}", url);
    }

    let resp = client.get(&url).send()?;
    if resp.status() == 404 {
        return Err(anyhow::anyhow!("Package not found on PyPI"));
    }
    let data: PipData = resp.json()?;

    let mut releases = Vec::new();

    for (version, files) in data.releases {
        let first_upload = files
            .into_iter()
            .filter_map(|f| pip_file_upload_time(&f))
            .min();
        if let Some(date) = first_upload {
            releases.push(PackageVersion { version, date });
        }
    }

    releases.sort_by_key(|v| v.date);
    Ok(releases)
}

fn find_pip(
    client: &Client,
    pkg: &str,
    target_date: DateTime<Utc>,
    verbose: bool,
) -> Result<Option<PackageVersion>> {
    let releases = fetch_pip_releases(client, pkg, verbose)?;

    let candidates = releases
        .into_iter()
        .filter(|v| v.date <= target_date)
        .collect();
    Ok(select_champion(candidates))
}

// --- NPM Strategy ---
#[derive(Deserialize)]
struct NpmData {
    time: HashMap<String, String>,
}

fn find_npm(
    client: &Client,
    pkg: &str,
    target_date: DateTime<Utc>,
    verbose: bool,
) -> Result<Option<PackageVersion>> {
    let url = format!("https://registry.npmjs.org/{}", pkg);
    if verbose {
        println!(" -> Fetching {}", url);
    }

    let resp = client.get(&url).send()?;
    if resp.status() == 404 {
        return Err(anyhow::anyhow!("Package not found on NPM"));
    }
    let data: NpmData = resp.json()?;

    let mut candidates = Vec::new();

    for (version, time_str) in data.time {
        if version == "created" || version == "modified" {
            continue;
        }

        // npm dates are usually ISO 8601 with timezone (e.g. 2014-12-23T23:54:33.000Z)
        if let Ok(date) = DateTime::parse_from_rfc3339(&time_str) {
            let date_utc = date.with_timezone(&Utc);
            if date_utc <= target_date {
                candidates.push(PackageVersion {
                    version,
                    date: date_utc,
                });
            }
        }
    }

    Ok(select_champion(candidates))
}

// --- CARGO Strategy ---
#[derive(Deserialize)]
struct CargoVersion {
    num: String,
    created_at: String,
}
#[derive(Deserialize)]
struct CargoData {
    versions: Vec<CargoVersion>,
}

fn find_cargo(
    client: &Client,
    pkg: &str,
    target_date: DateTime<Utc>,
    verbose: bool,
) -> Result<Option<PackageVersion>> {
    let url = format!("https://crates.io/api/v1/crates/{}", pkg);
    if verbose {
        println!(" -> Fetching {}", url);
    }

    let resp = client.get(&url).send()?;
    if resp.status() == 404 {
        return Err(anyhow::anyhow!("Crate not found on Crates.io"));
    }
    let data: CargoData = resp.json()?;

    let mut candidates = Vec::new();

    for v in data.versions {
        // "2015-05-06T00:52:16.890333+00:00" - RFC3339 compatible
        if let Ok(date) = DateTime::parse_from_rfc3339(&v.created_at) {
            let date_utc = date.with_timezone(&Utc);
            if date_utc <= target_date {
                candidates.push(PackageVersion {
                    version: v.num,
                    date: date_utc,
                });
            }
        }
    }

    Ok(select_champion(candidates))
}

// --- GEM Strategy ---
#[derive(Deserialize)]
struct GemVersion {
    number: String,
    created_at: String,
}

fn find_gem(
    client: &Client,
    pkg: &str,
    target_date: DateTime<Utc>,
    verbose: bool,
) -> Result<Option<PackageVersion>> {
    let url = format!("https://rubygems.org/api/v1/versions/{}.json", pkg);
    if verbose {
        println!(" -> Fetching {}", url);
    }

    let resp = client.get(&url).send()?;
    if resp.status() == 404 {
        return Err(anyhow::anyhow!("Gem not found on RubyGems"));
    }
    // Response is an array of versions
    let versions: Vec<GemVersion> = resp.json()?;

    let mut candidates = Vec::new();

    for v in versions {
        // "2015-01-23T19:00:00.000Z"
        if let Ok(date) = DateTime::parse_from_rfc3339(&v.created_at) {
            let date_utc = date.with_timezone(&Utc);
            if date_utc <= target_date {
                candidates.push(PackageVersion {
                    version: v.number,
                    date: date_utc,
                });
            }
        }
    }

    Ok(select_champion(candidates))
}

// --- COMPOSER (Packagist) Strategy ---
#[derive(Deserialize)]
struct PackagistVersion {
    time: String,
}

#[derive(Deserialize)]
struct PackagistPackage {
    versions: HashMap<String, PackagistVersion>,
}

#[derive(Deserialize)]
struct PackagistWrapper {
    package: PackagistPackage,
}

fn find_composer(
    client: &Client,
    pkg: &str,
    target_date: DateTime<Utc>,
    verbose: bool,
) -> Result<Option<PackageVersion>> {
    let url = format!("https://packagist.org/packages/{}.json", pkg);
    if verbose {
        println!(" -> Fetching {}", url);
    }

    let resp = client.get(&url).send()?;
    if resp.status() == 404 {
        return Err(anyhow::anyhow!(
            "Package not found on Packagist (ensure 'vendor/package' format)"
        ));
    }

    let wrapper: PackagistWrapper = resp.json()?;

    let mut candidates = Vec::new();

    for (version, data) in wrapper.package.versions {
        // Filter out dev versions if necessary, but key here is just time
        // "2021-02-16T14:36:00+00:00"
        if let Ok(date) = DateTime::parse_from_rfc3339(&data.time) {
            let date_utc = date.with_timezone(&Utc);
            if date_utc <= target_date {
                candidates.push(PackageVersion {
                    version,
                    date: date_utc,
                });
            }
        }
    }

    Ok(select_champion(candidates))
}

fn select_champion(mut candidates: Vec<PackageVersion>) -> Option<PackageVersion> {
    // Sort by date ascending
    candidates.sort_by_key(|v| v.date);
    // Return the last one (most recent before cutoff)
    candidates.pop()
}

fn pip_anchor_window(
    pkg: &str,
    version: &str,
    releases: &[PackageVersion],
) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let idx = releases
        .iter()
        .position(|v| v.version == version)
        .with_context(|| format!("Version '{}' not found for '{}'", version, pkg))?;

    let start = releases[idx].date;
    let end = releases
        .get(idx + 1)
        .map(|v| v.date)
        .unwrap_or_else(Utc::now);

    Ok((start, end))
}

fn versions_overlapping_window(
    releases: &[PackageVersion],
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> Vec<WindowedVersion> {
    if releases.is_empty() || window_start >= window_end {
        return Vec::new();
    }

    let mut out = Vec::new();

    let mut idx = releases
        .iter()
        .rposition(|v| v.date <= window_start)
        .unwrap_or(0);

    loop {
        let version = &releases[idx];
        let next_date = releases.get(idx + 1).map(|v| v.date).unwrap_or(window_end);

        let overlap_start = std::cmp::max(version.date, window_start);
        let overlap_end = std::cmp::min(next_date, window_end);

        if overlap_start < overlap_end {
            out.push(WindowedVersion {
                version: version.version.clone(),
                overlap_start,
                overlap_end,
            });
        }

        idx += 1;
        if idx >= releases.len() {
            break;
        }
        if releases[idx].date >= window_end {
            break;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn parse_pip_spec_ok() {
        let (n, v) = parse_pip_spec("requests==2.31.0").unwrap();
        assert_eq!(n, "requests");
        assert_eq!(v, "2.31.0");
    }

    #[test]
    fn parse_pip_spec_rejects_invalid() {
        assert!(parse_pip_spec("requests").is_err());
        assert!(parse_pip_spec("==1.0.0").is_err());
        assert!(parse_pip_spec("requests==").is_err());
    }

    fn pv(version: &str, y: i32, m: u32, d: u32) -> PackageVersion {
        PackageVersion {
            version: version.to_string(),
            date: Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap(),
        }
    }

    #[test]
    fn versions_overlapping_window_spans_multiple_versions() {
        let releases = vec![
            pv("1.0.0", 2020, 1, 1),
            pv("1.1.0", 2020, 2, 1),
            pv("2.0.0", 2020, 3, 1),
        ];
        let window_start = Utc.with_ymd_and_hms(2020, 1, 15, 0, 0, 0).unwrap();
        let window_end = Utc.with_ymd_and_hms(2020, 2, 15, 0, 0, 0).unwrap();

        let got = versions_overlapping_window(&releases, window_start, window_end);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].version, "1.0.0");
        assert_eq!(got[0].overlap_start, window_start);
        assert_eq!(got[0].overlap_end, releases[1].date);
        assert_eq!(got[1].version, "1.1.0");
        assert_eq!(got[1].overlap_start, releases[1].date);
        assert_eq!(got[1].overlap_end, window_end);
    }

    #[test]
    fn versions_overlapping_window_start_before_first_release() {
        let releases = vec![pv("0.1.0", 2020, 2, 1)];
        let window_start = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let window_end = Utc.with_ymd_and_hms(2020, 3, 1, 0, 0, 0).unwrap();

        let got = versions_overlapping_window(&releases, window_start, window_end);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].version, "0.1.0");
        assert_eq!(got[0].overlap_start, releases[0].date);
        assert_eq!(got[0].overlap_end, window_end);
    }
}
