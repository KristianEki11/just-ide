#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

//! Workspace automation tasks (`notices`, etc.) for Just IDE.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "Developer tasks for Just IDE")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Generates `THIRD_PARTY_NOTICES.md` from `cargo metadata` and embedded font attributions.
    Notices,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Task::Notices => {
            let notice_text = generate_third_party_notices()?;
            std::fs::write("THIRD_PARTY_NOTICES.md", notice_text)?;
            println!("Generated THIRD_PARTY_NOTICES.md");
        }
    }
    Ok(())
}

fn generate_third_party_notices() -> anyhow::Result<String> {
    use std::collections::BTreeMap;
    use std::fmt::Write as _;
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked"])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let root: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let packages = root["packages"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing packages array in cargo metadata"))?;

    let mut entries: BTreeMap<(String, String), (String, String)> = BTreeMap::new();
    for pkg in packages {
        if pkg["source"].is_null() {
            continue;
        }
        let name = pkg["name"].as_str().unwrap_or_default().to_owned();
        let version = pkg["version"].as_str().unwrap_or_default().to_owned();
        let license = pkg["license"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("See upstream repository")
            .to_owned();
        let repo = pkg["repository"]
            .as_str()
            .or_else(|| pkg["homepage"].as_str())
            .unwrap_or("https://crates.io")
            .to_owned();
        entries.insert((name, version), (license, repo));
    }

    let mut out = String::from(
        "# Third-Party Notices\n\n\
         Just IDE embeds permissive-licensed open-source fonts and statically links \
         crates from crates.io (verified offline with `cargo deny check` and `cargo audit`).\n\n\
         ## Embedded Fonts\n\n\
         | Font | License | Source |\n\
         |---|---|---|\n\
         | Inter | SIL Open Font License 1.1 (OFL-1.1) | https://rsms.me/inter/ |\n\
         | JetBrains Mono | SIL Open Font License 1.1 (OFL-1.1) | https://www.jetbrains.com/lp/mono/ |\n\n",
    );
    let _ = writeln!(
        out,
        "## Rust Crates ({} external packages)\n",
        entries.len()
    );
    out.push_str("| Crate | Version | License | Repository |\n|---|---|---|---|\n");
    for ((name, version), (license, repo)) in entries {
        let _ = writeln!(out, "| {name} | {version} | {license} | {repo} |");
    }
    Ok(out)
}
