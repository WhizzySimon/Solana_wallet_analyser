use serde_json::Value;
use crate::modules::utils::load_config;
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    thread,
    time::Duration,
};

/// Fetches transactions from cache or Helius API and returns them as a Vec<Value>
pub fn get_transactions() -> Result<Vec<Value>, Box<dyn std::error::Error>> {

    // Load config
    let settings = load_config()?;

    let helius_api_key = settings.helius_api_key;
    let wallet = settings.wallet_address;
    let use_cached_txns = settings.use_cached_txns.unwrap_or(false);

    // Load or fetch transactions from Helius
    let transactions_path = format!("cache/transactions_{}.json", wallet);
    let transactions: Vec<Value> = if use_cached_txns && Path::new(&transactions_path).exists() {
        println!("Using cached transactions...");
        let file = fs::read_to_string(&transactions_path)?;
        serde_json::from_str(&file)?
    } else {
        println!("Fetching transactions for wallet: {}", wallet);
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

            let batch: Vec<Value> = response.json()?;
            if batch.is_empty() {
                break;
            }

            before = batch.last().and_then(|tx| tx.get("signature")?.as_str().map(String::from));
            all.extend(batch);
            thread::sleep(Duration::from_millis(300));
        }

        println!("Saving transactions to cache...");
        fs::create_dir_all("cache")?;
        let mut file = File::create(&transactions_path)?;
        write!(file, "{}", serde_json::to_string_pretty(&all)?)?;
        all
    };
    return Ok(transactions);
}