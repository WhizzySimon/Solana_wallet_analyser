use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    path::Path,
    thread,
    time::Duration,
};

#[derive(Debug, Deserialize)]
struct Settings {
    helius_api_key: String,
    wallet_address: String,
    use_cached_txns: Option<bool>,
    use_cached_swaps: Option<bool>,
    use_token_cache: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SwapSummary {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_amount: f64,
    bought_mint: String,
    bought_amount: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings: Settings = config::Config::builder()
        .add_source(config::File::with_name("config/config"))
        .build()?
        .try_deserialize()?;

    let helius_api_key = settings.helius_api_key;
    let wallet = settings.wallet_address;
    let use_cached_txns = settings.use_cached_txns.unwrap_or(false);
    let use_cached_swaps = settings.use_cached_swaps.unwrap_or(false);
    let use_token_cache = settings.use_token_cache.unwrap_or(false);

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

    println!("Total transactions loaded: {}", transactions.len());

    let swaps_path = format!("cache/swaps_{}.json", wallet);
    let swaps: Vec<SwapSummary> = if use_cached_swaps && Path::new(&swaps_path).exists() {
        println!("Skipping swap filtering, using cached swaps.");
        let file = fs::read_to_string(&swaps_path)?;
        serde_json::from_str(&file)?
    } else {
        println!("Filtering swaps from transactions...");
        let mut swaps: Vec<SwapSummary> = Vec::new();
        for tx in &transactions {
            if let Some(token_transfers) = tx.get("tokenTransfers").and_then(|v| v.as_array()) {
                if token_transfers.len() == 2 {
                    let t1 = &token_transfers[0];
                    let t2 = &token_transfers[1];

                    let (sold, bought) = if t1.get("source").unwrap_or(&Value::Null) == wallet.as_str() {
                        (t1, t2)
                    } else {
                        (t2, t1)
                    };

                    swaps.push(SwapSummary {
                        timestamp: tx.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0),
                        signature: tx.get("signature").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        sold_mint: sold.get("mint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        sold_amount: sold.get("tokenAmount").and_then(|v| v.as_str()).unwrap_or("0").parse().unwrap_or(0.0),
                        bought_mint: bought.get("mint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        bought_amount: bought.get("tokenAmount").and_then(|v| v.as_str()).unwrap_or("0").parse().unwrap_or(0.0),
                    });
                }
            }
        }

        println!("Filtered {} swaps", swaps.len());
        let mut file = File::create(&swaps_path)?;
        write!(file, "{}", serde_json::to_string_pretty(&swaps)?)?;
        swaps
    };

    // fetch only from swaps
    let mut mint_name_map: HashMap<String, String> = HashMap::new();
    if !use_token_cache {
        println!("Fetching token names via Helius batch call...");

        let mut mints_set = HashSet::new();
        for swap in &swaps {
            mints_set.insert(swap.sold_mint.clone());
            mints_set.insert(swap.bought_mint.clone());
        }

        let mints: Vec<String> = mints_set.into_iter().collect();
        println!("Sending {} unique mint addresses to Helius", mints.len());

        let payload = json!({ "mintAccounts": mints });
        let client = reqwest::blocking::Client::new();
        let url = format!("https://api.helius.xyz/v0/token-metadata?api-key={}", helius_api_key);

        println!("Calling Helius: {}", url);
        let res = client.post(&url).json(&payload).send();

        match res {
            Ok(response) => {
                println!("Helius response status: {}", response.status());
                if response.status().is_success() {
                    let token_data: Vec<Value> = response.json()?;
                    for entry in &token_data {
                        let mint = entry.get("account").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let mut name = "UNKNOWN".to_string();
                        if let Some(meta) = entry.get("onChainMetadata") {
                            if let Some(meta_map) = meta.get("metadata") {
                                name = meta_map.get("name").and_then(|n| n.as_str()).unwrap_or("UNKNOWN").to_string();
                            }
                        }
                        mint_name_map.insert(mint, name);
                    }
                    fs::create_dir_all("cache")?;
                    let mut file = File::create("cache/token_names.json")?;
                    write!(file, "{}", serde_json::to_string_pretty(&token_data)?)?;
                    println!("Token names written to cache/token_names.json");
                } else {
                    println!("Failed to fetch token names. Status: {}", response.status());
                }
            }
            Err(err) => {
                println!("Error calling Helius: {}", err);
            }
        }
    } else {
        println!("Using cached token names (set use_token_cache=true)");
    }

    Ok(())
}
