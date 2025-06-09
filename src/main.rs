#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::{
    fs::{File, create_dir_all},
    io::{Read, Write},
    path::Path,
};

#[derive(Debug, Deserialize, Serialize)]
struct SwapSummary {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_amount: f64,
    bought_mint: String,
    bought_amount: f64,
}

fn ensure_dir_exists<P: AsRef<Path>>(dir: P) {
    if let Err(e) = create_dir_all(&dir) {
        eprintln!("Failed to create directory {:?}: {:?}", dir.as_ref(), e);
    }
}

fn main() {
    ensure_dir_exists("output");

    // Read all transactions from output/transactions.json
    let tx_path = "output/transactions.json";
    let mut f = File::open(tx_path).expect("Cannot open output/transactions.json");
    let mut contents = String::new();
    f.read_to_string(&mut contents).expect("Failed to read file");
    let all_transactions: Vec<serde_json::Value> = serde_json::from_str(&contents).expect("Failed to parse JSON");

    // Filter swaps as before: tokenTransfers length at least 2
    let swaps: Vec<SwapSummary> = all_transactions
        .iter()
        .filter_map(|tx| {
            let token_transfers = tx.get("tokenTransfers")?.as_array()?;
            if token_transfers.len() >= 2 {
                let sold = &token_transfers[0];
                let bought = &token_transfers[1];
                Some(SwapSummary {
                    timestamp: tx.get("timestamp")?.as_u64()?,
                    signature: tx.get("signature")?.as_str()?.to_string(),
                    sold_mint: sold.get("mint")?.as_str()?.to_string(),
                    sold_amount: sold.get("tokenAmount")?.as_f64()?,
                    bought_mint: bought.get("mint")?.as_str()?.to_string(),
                    bought_amount: bought.get("tokenAmount")?.as_f64()?,
                })
            } else {
                None
            }
        })
        .collect();

    // Write to swaps_filtered.json
    let swaps_path = "output/swaps_filtered.json";
    let mut f = File::create(swaps_path).expect("Cannot create output/swaps_filtered.json");
    f.write_all(serde_json::to_string_pretty(&swaps).unwrap().as_bytes()).expect("Cannot write swaps_filtered.json");

    println!("Filtered {} swaps and wrote to {}", swaps.len(), swaps_path);
}
