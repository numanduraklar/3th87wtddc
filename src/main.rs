use std::collections::BTreeSet;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde::Serialize;

const API_BASE: &str = "https://api.hackerone.com/v1";

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Fetches assets (structured scopes) from HackerOne API"
)]
struct Cli {
    /// HackerOne API username. Falls back to H1_USERNAME.
    #[arg(long, env = "H1_USERNAME")]
    username: String,

    /// HackerOne API token. Falls back to H1_TOKEN.
    #[arg(long, env = "H1_TOKEN")]
    token: String,

    /// Restrict to one or more specific program handles.
    #[arg(short, long)]
    program: Vec<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    output: OutputFormat,
}

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
}

#[derive(Debug, Deserialize)]
struct ProgramsResponse {
    data: Vec<ProgramObject>,
    links: Option<Links>,
}

#[derive(Debug, Deserialize)]
struct ProgramObject {
    attributes: ProgramAttributes,
}

#[derive(Debug, Deserialize)]
struct ProgramAttributes {
    handle: String,
}

#[derive(Debug, Deserialize)]
struct ScopesResponse {
    data: Vec<ScopeObject>,
    links: Option<Links>,
}

#[derive(Debug, Deserialize)]
struct ScopeObject {
    attributes: ScopeAttributes,
}

#[derive(Debug, Deserialize)]
struct ScopeAttributes {
    asset_type: String,
    asset_identifier: String,
    #[serde(default)]
    eligible_for_bounty: Option<bool>,
    #[serde(default)]
    instruction: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Links {
    next: Option<String>,
}

#[derive(Debug, Serialize)]
struct AssetRecord {
    program: String,
    asset_type: String,
    asset_identifier: String,
    eligible_for_bounty: Option<bool>,
    instruction: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let client = Client::builder()
        .user_agent("h1-assets-fetcher/0.1")
        .build()
        .context("failed to initialize HTTP client")?;

    let handles = if cli.program.is_empty() {
        fetch_program_handles(&client, &cli.username, &cli.token).await?
    } else {
        cli.program.iter().cloned().collect::<BTreeSet<_>>()
    };

    if handles.is_empty() {
        println!("No program handle found.");
        return Ok(());
    }

    let mut assets = Vec::new();
    for handle in handles {
        let mut program_assets =
            fetch_assets_for_program(&client, &cli.username, &cli.token, &handle).await?;
        assets.append(&mut program_assets);
    }

    match cli.output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&assets)?);
        }
        OutputFormat::Table => print_table(&assets),
    }

    Ok(())
}

async fn fetch_program_handles(
    client: &Client,
    username: &str,
    token: &str,
) -> Result<BTreeSet<String>> {
    let mut next = Some(Url::parse(&format!(
        "{API_BASE}/hackers/programs?page[size]=100"
    ))?);
    let mut handles = BTreeSet::new();

    while let Some(url) = next {
        let response = client
            .get(url)
            .basic_auth(username, Some(token))
            .header("Accept", "application/json")
            .send()
            .await
            .context("failed to call GET /hackers/programs")?
            .error_for_status()
            .context("GET /hackers/programs returned an error status")?;

        let parsed: ProgramsResponse = response
            .json()
            .await
            .context("failed to parse programs response")?;

        handles.extend(parsed.data.into_iter().map(|p| p.attributes.handle));

        next = parsed
            .links
            .and_then(|l| l.next)
            .map(|raw| Url::parse(&raw))
            .transpose()
            .context("invalid pagination url in programs response")?;
    }

    Ok(handles)
}

async fn fetch_assets_for_program(
    client: &Client,
    username: &str,
    token: &str,
    handle: &str,
) -> Result<Vec<AssetRecord>> {
    let mut next = Some(Url::parse(&format!(
        "{API_BASE}/hackers/programs/{handle}/structured_scopes?page[size]=100"
    ))?);
    let mut assets = Vec::new();

    while let Some(url) = next {
        let response = client
            .get(url)
            .basic_auth(username, Some(token))
            .header("Accept", "application/json")
            .send()
            .await
            .with_context(|| format!("failed to fetch scopes for program '{handle}'"))?
            .error_for_status()
            .with_context(|| format!("non-success status while fetching scopes for '{handle}'"))?;

        let parsed: ScopesResponse = response
            .json()
            .await
            .with_context(|| format!("invalid structured_scopes JSON for '{handle}'"))?;

        for item in parsed.data {
            assets.push(AssetRecord {
                program: handle.to_string(),
                asset_type: item.attributes.asset_type,
                asset_identifier: item.attributes.asset_identifier,
                eligible_for_bounty: item.attributes.eligible_for_bounty,
                instruction: item.attributes.instruction,
            });
        }

        next = parsed
            .links
            .and_then(|l| l.next)
            .map(|raw| Url::parse(&raw))
            .transpose()
            .with_context(|| {
                format!("invalid pagination url in structured_scopes for '{handle}'")
            })?;
    }

    Ok(assets)
}

fn print_table(assets: &[AssetRecord]) {
    if assets.is_empty() {
        println!("No assets found.");
        return;
    }

    println!(
        "{:<24} {:<14} {:<8} {:<50}",
        "PROGRAM", "ASSET_TYPE", "BOUNTY", "ASSET_IDENTIFIER"
    );
    println!("{}", "-".repeat(104));

    for a in assets {
        let bounty = a
            .eligible_for_bounty
            .map(|v| if v { "yes" } else { "no" })
            .unwrap_or("n/a");

        println!(
            "{:<24} {:<14} {:<8} {:<50}",
            truncate(&a.program, 24),
            truncate(&a.asset_type, 14),
            bounty,
            truncate(&a.asset_identifier, 50),
        );
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        return s.to_string();
    }

    let mut out = s
        .chars()
        .take(max_len.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}
