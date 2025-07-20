use crate::modules::utils::load_config;
use crate::modules::types::{RawTxn};

use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::Path,
    thread,
    time::Duration,
};

/// Loads transactions from cache or fetches from Helius, and deserializes into RawTxn structs
pub fn get_transactions() -> Result<Vec<RawTxn>, Box<dyn std::error::Error>> {
    let settings = load_config()?;
    let helius_api_key = settings.helius_api_key;
    let wallet = settings.wallet_address;
    let use_cached_txns = settings.use_cached_txns.unwrap_or(false);

    let transactions_path = format!("cache/transactions_{}.json", wallet);
    let transactions: Vec<RawTxn> = if use_cached_txns && Path::new(&transactions_path).exists() {
        println!("♻️  Using cached transactions from {}", transactions_path);
        let file = File::open(&transactions_path)?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)?
    } else {
        println!("🌐 Fetching transactions for wallet: {}", wallet);
        let mut all = vec![];
        let mut before: Option<String> = None;
        let client = reqwest::blocking::Client::new();

        loop {
            let url = format!(
                "https://api.helius.xyz/v0/addresses/{}/transactions?api-key={}&limit=100{}",
                wallet,
                helius_api_key,
                before
                    .as_ref()
                    .map(|b| format!("&before={}", b))
                    .unwrap_or_default()
            );

            let response = client.get(&url).send()?;
            if !response.status().is_success() {
                println!("Helius error: {}", response.status());
                break;
            }

            let batch: Vec<RawTxn> = response.json()?;
            if batch.is_empty() {
                break;
            }

            before = batch.last().map(|tx| tx.signature.clone());
            all.extend(batch);
            thread::sleep(Duration::from_millis(300));
        }

        println!("💾 Saving transactions to cache...");
        fs::create_dir_all("cache")?;
        let mut file = File::create(&transactions_path)?;
        write!(file, "{}", serde_json::to_string_pretty(&all)?)?;
        all
    };

    Ok(transactions)
}
