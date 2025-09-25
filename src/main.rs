mod config;
mod scrubber;

use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

use config::ScrubberConfig;
use scrubber::{ScrubStats, Scrubber};

#[derive(Parser, Debug)]
#[command(
    name = "clinical-scrubber",
    about = "CLI tool that redacts common PHI elements from clinical notes.",
    version,
    author = ""
)]
struct Args {
    /// Optional input file. Use '-' to read from STDIN.
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Optional output file. Use '-' to write to STDOUT.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Path to JSON config that augments the default dictionaries.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Redaction categories to skip (e.g. --skip person --skip date).
    #[arg(long, value_enum)]
    skip: Vec<Category>,

    /// Suppress redaction summary.
    #[arg(long)]
    quiet: bool,

    /// Emit redaction stats as JSON to stderr.
    #[arg(long)]
    stats_json: bool,

    /// Enable additional HIPAA Safe Harbor redactions (IDs, licenses, IPs, etc.).
    #[arg(long)]
    safe_harbor: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, ValueEnum)]
enum Category {
    Email,
    Phone,
    Date,
    RelativeDate,
    Ssn,
    Mrn,
    Zip,
    Person,
    Facility,
    Address,
    Coordinate,
    Url,
    Insurance,
    License,
    Vehicle,
    Device,
    Ip,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let skip: HashSet<Category> = args.skip.into_iter().collect();

    let config = load_config(args.config.as_ref())?;
    let scrubber = Scrubber::new(config, args.safe_harbor)?;

    let input = read_input(args.input.as_ref())?;
    let (scrubbed, stats) = scrubber.scrub(&input, &skip);
    write_output(args.output.as_ref(), &scrubbed)?;

    if !args.quiet {
        report_stats(&stats, args.stats_json)?;
    }

    Ok(())
}

fn read_input(path: Option<&PathBuf>) -> Result<String> {
    match path {
        Some(p) if p == std::path::Path::new("-") => read_from_stdin(),
        Some(p) => fs::read_to_string(p)
            .with_context(|| format!("failed to read input file: {}", p.display())),
        None => read_from_stdin(),
    }
}

fn read_from_stdin() -> Result<String> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .context("failed to read from STDIN")?;
    Ok(buffer)
}

fn write_output(path: Option<&PathBuf>, contents: &str) -> Result<()> {
    match path {
        Some(p) if p == std::path::Path::new("-") => {
            io::stdout()
                .write_all(contents.as_bytes())
                .context("failed to write to STDOUT")?;
        }
        Some(p) => {
            let mut file = fs::File::create(p)
                .with_context(|| format!("failed to create output file: {}", p.display()))?;
            file.write_all(contents.as_bytes())
                .with_context(|| format!("failed to write output file: {}", p.display()))?;
        }
        None => {
            io::stdout()
                .write_all(contents.as_bytes())
                .context("failed to write to STDOUT")?;
        }
    }

    Ok(())
}

fn load_config(path: Option<&PathBuf>) -> Result<ScrubberConfig> {
    match path {
        Some(p) => {
            let raw = fs::read_to_string(p)
                .with_context(|| format!("failed to read config file: {}", p.display()))?;
            let config: ScrubberConfig = serde_json::from_str(&raw)
                .with_context(|| format!("failed to parse config JSON: {}", p.display()))?;
            Ok(config)
        }
        None => Ok(ScrubberConfig::default()),
    }
}

fn report_stats(stats: &ScrubStats, as_json: bool) -> Result<()> {
    if as_json {
        let payload = serde_json::to_string_pretty(stats).context("failed to serialize stats")?;
        eprintln!("{}", payload);
    } else {
        eprintln!("Redactions applied: {}", stats.total());
        if stats.emails > 0 {
            eprintln!("  emails   : {}", stats.emails);
        }
        if stats.phones > 0 {
            eprintln!("  phones   : {}", stats.phones);
        }
        if stats.dates > 0 {
            eprintln!("  dates    : {}", stats.dates);
        }
        if stats.ssn > 0 {
            eprintln!("  ssn          : {}", stats.ssn);
        }
        if stats.mrn > 0 {
            eprintln!("  mrn          : {}", stats.mrn);
        }
        if stats.zip_codes > 0 {
            eprintln!("  zip codes    : {}", stats.zip_codes);
        }
        if stats.persons > 0 {
            eprintln!("  persons      : {}", stats.persons);
        }
        if stats.facilities > 0 {
            eprintln!("  facilities   : {}", stats.facilities);
        }
        if stats.addresses > 0 {
            eprintln!("  addresses    : {}", stats.addresses);
        }
        if stats.coordinates > 0 {
            eprintln!("  coordinates  : {}", stats.coordinates);
        }
        if stats.urls > 0 {
            eprintln!("  urls         : {}", stats.urls);
        }
        if stats.insurance_ids > 0 {
            eprintln!("  insurance    : {}", stats.insurance_ids);
        }
        if stats.licenses > 0 {
            eprintln!("  licenses     : {}", stats.licenses);
        }
        if stats.vehicles > 0 {
            eprintln!("  vehicles     : {}", stats.vehicles);
        }
        if stats.devices > 0 {
            eprintln!("  devices      : {}", stats.devices);
        }
        if stats.ip_addresses > 0 {
            eprintln!("  ip addresses : {}", stats.ip_addresses);
        }
        if stats.relative_dates > 0 {
            eprintln!("  relative dates: {}", stats.relative_dates);
        }
    }
    Ok(())
}
