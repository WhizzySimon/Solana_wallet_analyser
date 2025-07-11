use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fs, io::Write, path::Path, thread, time::Duration};

#[derive(Debug, Deserialize)]
struct Settings {
    helius_api_key: String,
    wallet_address: String,
}

#[derive(Debug, Serialize)]
struct SwapSummary {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_amount: f64,
    bought_mint: String,
    bought_amount: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- 1. Load config ---
    let settings: Settings = config::Config::builder()
        .add_source(config::File::with_name("config/config"))
        .build()?
        .try_deserialize()?;
    let helius_api_key = settings.helius_api_key;
    let wallet = settings.wallet_address;

    // --- 2. Fetch ALL transactions from Helius with pagination ---
    let mut all_transactions: Vec<Value> = Vec::new();
    let mut before: Option<String> = None;
    let mut call_count = 0;

    loop {
        let mut url = format!(
            "https://api.helius.xyz/v0/addresses/{}/transactions?api-key={}&limit=100",
            wallet, helius_api_key
        );
        if let Some(ref sig) = before {
            url.push_str(&format!("&before={}", sig));
        }
        println!("Fetching: {}", url);
        let response = reqwest::blocking::get(&url)?.text()?;
        let batch: Vec<Value> = serde_json::from_str(&response)?;

        if batch.is_empty() {
            println!("No more transactions.");
            break;
        }

        all_transactions.extend(batch.iter().cloned());
        call_count += 1;

        before = batch
            .last()
            .and_then(|tx| tx.get("signature"))
            .and_then(|sig| sig.as_str())
            .map(|s| s.to_string());

        thread::sleep(Duration::from_millis(300)); // Be nice to the API
    }

    println!("Fetched {} total transactions.", all_transactions.len());

    // --- 3. Write all transactions to file ---
    let tx_path = "data/transactions.json";
    if let Some(parent) = Path::new(tx_path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(tx_path, serde_json::to_string_pretty(&all_transactions)?)?;
    println!("All transactions saved to {}", tx_path);

    // --- 4. Extract swaps (bundled and single) ---
    let mut swaps = Vec::new();

    for tx in &all_transactions {
        let token_transfers = tx.get("tokenTransfers").and_then(|v| v.as_array());
        if let Some(tts) = token_transfers {
            if tts.len() >= 2 {
                let mut net: HashMap<String, f64> = HashMap::new();
                for t in tts {
                    let mint = t.get("mint").and_then(|v| v.as_str()).unwrap_or("");
                    let amount = t.get("tokenAmount").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let from = t.get("fromUserAccount").and_then(|v| v.as_str()).unwrap_or("");
                    let to = t.get("toUserAccount").and_then(|v| v.as_str()).unwrap_or("");
                    if from == wallet {
                        *net.entry(mint.to_string()).or_insert(0.0) -= amount;
                    }
                    if to == wallet {
                        *net.entry(mint.to_string()).or_insert(0.0) += amount;
                    }
                }
                // Find largest net negative (sold), and largest net positive (bought)
                let mut sold_mint = String::new();
                let mut sold_amount = 0.0;
                let mut bought_mint = String::new();
                let mut bought_amount = 0.0;
                for (mint, amt) in net.iter() {
                    if *amt < sold_amount {
                        sold_mint = mint.clone();
                        sold_amount = *amt;
                    }
                    if *amt > bought_amount {
                        bought_mint = mint.clone();
                        bought_amount = *amt;
                    }
                }
                if sold_amount < 0.0 && bought_amount > 0.0 {
                    swaps.push(SwapSummary {
                        timestamp: tx.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0),
                        signature: tx.get("signature").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        sold_mint,
                        sold_amount: -sold_amount,
                        bought_mint,
                        bought_amount,
                    });
                }
            }
        }
    }

    println!("Found {} swaps (bundled+simple).", swaps.len());

    // --- 5. Write swaps to JSON ---
    let swaps_path = "data/swaps_extracted.json";
    if let Some(parent) = Path::new(swaps_path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(swaps_path, serde_json::to_string_pretty(&swaps)?)?;
    println!("Done! Swaps written to {}", swaps_path);

    Ok(())
}
