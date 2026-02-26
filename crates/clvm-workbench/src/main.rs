use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use chialisp::classic::clvm::OPERATORS_LATEST_VERSION;
use chialisp::classic::clvm_tools::binutils::{assemble, disassemble};
use chialisp::classic::clvm_tools::cmds;
use clvmr::allocator::Allocator;
use clvmr::serde::{node_from_bytes_backrefs, node_to_bytes};

#[derive(Debug, Parser)]
#[command(
    name = "clvm-workbench",
    about = "CLVM utility wrapper for opd/opc/brun workflows"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Decode CLVM bytes to readable CLVM
    Opd {
        input: String,
    },
    /// Encode readable CLVM to bytes
    Opc {
        input: String,
    },
    /// Run CLVM program with environment
    Run {
        #[arg(long)]
        program: String,
        #[arg(long, default_value = "()")]
        env: String,
        #[arg(long, default_value_t = false)]
        cost: bool,
        #[arg(long, default_value_t = false)]
        verbose: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Opd { input } => {
            let bytes = decode_hex_input(&input)?;
            let mut allocator = Allocator::new();
            let node = node_from_bytes_backrefs(&mut allocator, &bytes)?;
            println!(
                "{}",
                disassemble(&allocator, node, Some(OPERATORS_LATEST_VERSION))
            );
        }
        Command::Opc { input } => {
            let mut allocator = Allocator::new();
            let node = assemble(&mut allocator, &input)
                .map_err(|e| anyhow::anyhow!("failed to assemble CLVM: {e}"))?;
            let bytes = node_to_bytes(&allocator, node)?;
            println!("0x{}", hex::encode(bytes));
        }
        Command::Run {
            program,
            env,
            cost,
            verbose,
        } => {
            let mut args = vec!["brun".to_string()];
            if cost {
                args.push("--cost".to_string());
            }
            if verbose {
                args.push("--verbose".to_string());
            }
            args.push(normalize_program_input(&program)?);
            args.push(normalize_program_input(&env)?);
            cmds::brun(&args);
        }
    }
    Ok(())
}

fn normalize_program_input(input: &str) -> Result<String> {
    if looks_like_hex(input) {
        let bytes = decode_hex_input(input)?;
        let mut allocator = Allocator::new();
        let node = node_from_bytes_backrefs(&mut allocator, &bytes)?;
        return Ok(disassemble(&allocator, node, Some(OPERATORS_LATEST_VERSION)));
    }
    Ok(input.to_string())
}

fn decode_hex_input(input: &str) -> Result<Vec<u8>> {
    let raw = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .unwrap_or(input)
        .trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    if raw.len() % 2 != 0 {
        bail!("hex input must have even length");
    }
    hex::decode(raw).with_context(|| "failed to decode hex input")
}

fn looks_like_hex(input: &str) -> bool {
    let raw = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .unwrap_or(input);
    !raw.is_empty()
        && raw.len() % 2 == 0
        && raw
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
}
