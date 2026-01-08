use std::io::{self, Read};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};

const BTC_CLI: &str = "bitcoin-cli";

#[derive(Parser)]
#[command(name = "sign-txs")]
#[command(about = "Sign Bitcoin transactions from a JSON file or stdin")]
struct Args {
    /// Input JSON file containing transactions (reads from stdin if not provided)
    input_file: Option<String>,

    /// Docker container ID running bitcoind with the wallet (uses local bitcoin-cli if not provided)
    #[arg(long, env = "BITCOIND_CONTAINER")]
    bitcoind_container: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TxEntry {
    bitcoin: String,
}

#[derive(Debug, Deserialize)]
struct DecodeResult {
    vin: Vec<VinEntry>,
}

#[derive(Debug, Deserialize)]
struct VinEntry {
    txid: String,
    vout: u32,
    txinwitness: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct TxInfo {
    vout: Vec<VoutEntry>,
}

#[derive(Debug, Deserialize)]
struct VoutEntry {
    value: f64,
    #[serde(rename = "scriptPubKey")]
    script_pubkey: ScriptPubKey,
}

#[derive(Debug, Deserialize)]
struct ScriptPubKey {
    hex: String,
}

#[derive(Debug, Serialize)]
struct PrevOut {
    txid: String,
    vout: u32,
    amount: f64,
    #[serde(rename = "scriptPubKey")]
    script_pubkey: String,
}

#[derive(Debug, Deserialize)]
struct SignResult {
    hex: String,
    complete: bool,
    errors: Option<Vec<serde_json::Value>>,
}

fn run_btc_cli(args: &[&str]) -> Result<String> {
    let output = Command::new(BTC_CLI)
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute {}", BTC_CLI))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{} failed: {}", BTC_CLI, stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_docker_btc(container: &str, args: &[&str]) -> Result<String> {
    let mut cmd_args = vec!["exec", container, BTC_CLI];
    cmd_args.extend(args);

    let output = Command::new("docker")
        .args(&cmd_args)
        .output()
        .context("Failed to execute docker")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("docker exec {BTC_CLI} failed: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn decode_transaction(raw_tx: &str) -> Result<DecodeResult> {
    let output = run_btc_cli(&["decoderawtransaction", raw_tx])?;
    serde_json::from_str(&output).context("Failed to parse decoded transaction")
}

fn get_prevout_info(txid: &str, vout: u32) -> Result<Option<(f64, String)>> {
    let output = run_btc_cli(&["getrawtransaction", txid, "true"])?;
    let tx_info: TxInfo =
        serde_json::from_str(&output).context("Failed to parse transaction info")?;

    if let Some(vout_entry) = tx_info.vout.get(vout as usize) {
        Ok(Some((
            vout_entry.value,
            vout_entry.script_pubkey.hex.clone(),
        )))
    } else {
        Ok(None)
    }
}

fn sign_transaction(container: Option<&str>, raw_tx: &str, tx_index: usize) -> Result<String> {
    eprintln!("\nProcessing transaction {}...", tx_index + 1);

    // Decode the transaction to get inputs
    let decoded = decode_transaction(raw_tx)?;

    // Build prevouts array for all inputs that need signing
    let mut prevouts: Vec<PrevOut> = Vec::new();

    for (i, input) in decoded.vin.iter().enumerate() {
        // Check if this input has witness data (already signed)
        if input.txinwitness.is_some() {
            eprintln!("  Input {}: already signed, skipping", i);
            continue;
        }

        eprintln!(
            "  Input {}: {}:{} - fetching prevout info...",
            i, input.txid, input.vout
        );

        // Get the previous output info from the remote node
        match get_prevout_info(&input.txid, input.vout)? {
            Some((amount, script_pubkey)) => {
                eprintln!(
                    "  Input {}: amount={}, scriptPubKey={}",
                    i, amount, script_pubkey
                );
                prevouts.push(PrevOut {
                    txid: input.txid.clone(),
                    vout: input.vout,
                    amount,
                    script_pubkey,
                });
            }
            None => {
                eprintln!(
                    "  Input {}: prevout not found on chain, may be from earlier tx in batch",
                    i
                );
            }
        }
    }

    if prevouts.is_empty() {
        eprintln!("  No inputs to sign, returning original transaction");
        return Ok(raw_tx.to_string());
    }

    eprintln!("  Signing {} input(s) with wallet...", prevouts.len());

    // Sign with wallet (either via Docker or local bitcoin-cli)
    let prevouts_json = serde_json::to_string(&prevouts)?;
    let sign_output = match container {
        Some(c) => run_docker_btc(c, &["signrawtransactionwithwallet", raw_tx, &prevouts_json])?,
        None => run_btc_cli(&["signrawtransactionwithwallet", raw_tx, &prevouts_json])?,
    };

    let sign_result: SignResult =
        serde_json::from_str(&sign_output).context("Failed to parse sign result")?;

    if sign_result.complete {
        eprintln!("  Transaction fully signed");
    } else if let Some(errors) = &sign_result.errors {
        eprintln!(
            "  Warning: Transaction not fully signed. Errors: {}",
            serde_json::to_string_pretty(errors)?
        );
    }

    Ok(sign_result.hex)
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Read input from file or stdin
    let (content, source) = match &args.input_file {
        Some(path) => {
            let content = std::fs::read_to_string(path).context("Failed to read input file")?;
            (content, path.as_str())
        }
        None => {
            let mut content = String::new();
            io::stdin()
                .read_to_string(&mut content)
                .context("Failed to read from stdin")?;
            (content, "stdin")
        }
    };
    let txs: Vec<TxEntry> = serde_json::from_str(&content).context("Failed to parse input JSON")?;

    eprintln!("Reading transactions from: {}", source);
    eprintln!("Found {} transaction(s) to process", txs.len());

    // Process each transaction
    let mut signed_txs: Vec<TxEntry> = Vec::new();

    for (i, tx) in txs.iter().enumerate() {
        let signed_hex = sign_transaction(args.bitcoind_container.as_deref(), &tx.bitcoin, i)?;
        signed_txs.push(TxEntry {
            bitcoin: signed_hex,
        });
    }

    eprintln!("\nAll transactions processed. Output:\n");

    // Output signed transactions
    println!("{}", serde_json::to_string_pretty(&signed_txs)?);

    Ok(())
}
