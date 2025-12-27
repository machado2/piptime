use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use clap::{Parser, ValueEnum};
use colored::*;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The package manager to use
    #[arg(value_enum)]
    manager: Manager,

    /// The cutoff date (YYYY-MM-DD)
    date: String,

    /// List of packages to check
    #[arg(required = true)]
    packages: Vec<String>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
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

fn main() -> Result<()> {
    // Enable color support on Windows
    #[cfg(windows)]
    let _ = colored::control::set_virtual_terminal(true);

    let args = Args::parse();

    // Parse date
    let naive_date = NaiveDate::parse_from_str(&args.date, "%Y-%m-%d")
        .context("Invalid date format. Use YYYY-MM-DD")?;
    // Set time to end of day to include releases on that day
    let target_date = naive_date.and_hms_opt(23, 59, 59).unwrap().and_utc();

    println!(
        "--- Searching for {} packages up to {} ---",
        format!("{:?}", args.manager).yellow(),
        target_date.date_naive().to_string().yellow()
    );

    let client = Client::builder()
        .user_agent("pkgtime/1.0 (pkgtime-tool)")
        .build()?;

    let mut install_cmds = Vec::new();
    let mut errors = Vec::new();

    for pkg in &args.packages {
        match find_version(&client, args.manager, pkg, target_date, args.verbose) {
            Ok(Some(v)) => {
                println!(
                    "✅ {}: {} (from {})",
                    pkg.green(),
                    v.version.bold(),
                    v.date.date_naive()
                );

                let cmd = match args.manager {
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
        print_install_instructions(args.manager, &install_cmds);
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
    upload_time: String,
}
#[derive(Deserialize)]
struct PipData {
    releases: HashMap<String, Vec<PipReleaseFile>>,
}

fn find_pip(
    client: &Client,
    pkg: &str,
    target_date: DateTime<Utc>,
    verbose: bool,
) -> Result<Option<PackageVersion>> {
    let url = format!("https://pypi.org/pypi/{}/json", pkg);
    if verbose {
        println!(" -> Fetching {}", url);
    }

    let resp = client.get(&url).send()?;
    if resp.status() == 404 {
        return Err(anyhow::anyhow!("Package not found on PyPI"));
    }
    let data: PipData = resp.json()?;

    let mut candidates = Vec::new();

    for (version, files) in data.releases {
        if let Some(first_file) = files.first() {
            // "2019-05-16T17:21:44"
            let layout = "%Y-%m-%dT%H:%M:%S";
            // Pip timestamps are typically UTC but without timezone info in string
            if let Ok(naive) =
                chrono::NaiveDateTime::parse_from_str(&first_file.upload_time, layout)
            {
                let date = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
                if date <= target_date {
                    candidates.push(PackageVersion { version, date });
                }
            }
        }
    }

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
