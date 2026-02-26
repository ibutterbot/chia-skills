use std::io::{Read, Write};

use anyhow::Result;
use chia_inspect_core::{
    ExplainLevel, inspect_bundle, load_block_spends_input, load_coin_spend_input,
    load_mempool_blob_input,
};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "chia-inspect",
    about = "Inspect Chia spend bundles and CLVM puzzle behavior"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    #[arg(long, value_enum, default_value_t = ExplainLevelArg::Deep)]
    explain_level: ExplainLevelArg,

    #[arg(long, default_value_t = false)]
    pretty: bool,

    #[arg(long, default_value = "-")]
    output: String,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect a mempool blob containing spend bundle data
    Mempool {
        #[arg(long)]
        blob_json: String,
    },
    /// Inspect block spend data from a coin spend list
    Block {
        #[arg(long)]
        spends_json: String,
    },
    /// Inspect a single coin spend payload
    Coin {
        #[arg(long)]
        coin_spend_json: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExplainLevelArg {
    Conditions,
    Deep,
}

impl From<ExplainLevelArg> for ExplainLevel {
    fn from(value: ExplainLevelArg) -> Self {
        match value {
            ExplainLevelArg::Conditions => ExplainLevel::Conditions,
            ExplainLevelArg::Deep => ExplainLevel::Deep,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let explain_level = ExplainLevel::from(cli.explain_level);

    let (source, bundle, notes) = match &cli.command {
        Command::Mempool { blob_json } => load_mempool_blob_input(&read_input(blob_json)?)?,
        Command::Block { spends_json } => load_block_spends_input(&read_input(spends_json)?)?,
        Command::Coin { coin_spend_json } => load_coin_spend_input(&read_input(coin_spend_json)?)?,
    };

    let output = inspect_bundle(source, bundle, notes, explain_level)?;
    let serialized = if cli.pretty {
        serde_json::to_string_pretty(&output)?
    } else {
        serde_json::to_string(&output)?
    };
    write_output(&cli.output, &serialized)?;
    Ok(())
}

fn read_input(path_or_stdin: &str) -> Result<String> {
    if path_or_stdin == "-" {
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        return Ok(input);
    }
    Ok(std::fs::read_to_string(path_or_stdin)?)
}

fn write_output(path_or_stdout: &str, data: &str) -> Result<()> {
    if path_or_stdout == "-" {
        let mut stdout = std::io::stdout();
        stdout.write_all(data.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
        return Ok(());
    }
    std::fs::write(path_or_stdout, format!("{data}\n"))?;
    Ok(())
}
