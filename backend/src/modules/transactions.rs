use crate::modules::types::{RawTxn, Settings};

use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::Path,
};

use tokio::time::{sleep, Duration};
use reqwest::Client;
use chrono::{Utc, Duration as ChronoDuration};

/// Convenience error type alias
pub type AnyError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub async fn get_transactions(settings: &Settings) -> Result<Vec<RawTxn>, AnyError> {
    let helius_api_key = &settings.helius_api_key;
    let use_cached_txns = settings.config.use_cached_txns.unwrap_or(false);
    let write_cache_files = settings.config.write_cache_files.unwrap_or(false);

    let transactions_path = format!("cache/transactions_{}.json", settings.wallet_address);

    let transactions: Vec<RawTxn> = if use_cached_txns && Path::new(&transactions_path).exists() {
        println!("♻️  Using cached transactions from {}", transactions_path);
        let file = File::open(&transactions_path)
            .map_err(|e| format!("Failed to open cache file: {}", e))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .map_err(|e| format!("Failed to parse cached JSON: {}", e))?
    } else {
        println!("🌐 Fetching transactions for wallet: {}", settings.wallet_address);

        let client = Client::new();
        let mut all = Vec::new();
        let mut before: Option<String> = None;

        let thirty_days_ago = Utc::now().timestamp() - ChronoDuration::days(30).num_seconds();

        loop {
            let url = format!(
                "https://api.helius.xyz/v0/addresses/{}/transactions?api-key={}{}",
                settings.wallet_address,
                helius_api_key,
                before
                    .as_ref()
                    .map(|b| format!("&before={}", b))
                    .unwrap_or_default()
            );

            let response = client.get(&url).send().await
                .map_err(|e| format!("HTTP error from Helius: {}", e))?;

            if !response.status().is_success() {
                return Err(format!("Helius returned status {}", response.status()).into());
            }

            let batch: Vec<RawTxn> = response.json().await
                .map_err(|e| format!("Failed to deserialize transaction batch: {}", e))?;

            let filtered_batch: Vec<RawTxn> = batch
                .into_iter()
                .filter(|tx| tx.timestamp.unwrap_or(0) as i64 >= thirty_days_ago)
                .collect();


            if filtered_batch.is_empty() {
                println!("⏹️  Stopped: no transactions in last 30 days.");
                break;
            }

            before = filtered_batch.last().map(|tx| tx.signature.clone());
            all.extend(filtered_batch);

            sleep(Duration::from_millis(300)).await;
        }

        if write_cache_files {
            println!("💾 Saving transactions to cache...");
            fs::create_dir_all("cache")
                .map_err(|e| format!("Failed to create cache dir: {}", e))?;
            let mut file = File::create(&transactions_path)
                .map_err(|e| format!("Failed to create cache file: {}", e))?;
            write!(file, "{}", serde_json::to_string_pretty(&all)
                .map_err(|e| format!("Failed to serialize JSON: {}", e))?)
                .map_err(|e| format!("Failed to write to cache file: {}", e))?;
        } else {
            println!("Fetched {} transactions.", all.len());
        }

        all
    };

    Ok(transactions)
}
