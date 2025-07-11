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
    use_jupiter_token_list: Option<bool>, // ✅ Added flag
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

#[derive(Debug, Serialize)]
struct EnrichedSwapSummary {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_token_name: String,
    sold_amount: f64,
    bought_mint: String,
    bought_token_name: String,
    bought_amount: f64,
}

fn load_jupiter_token_map() -> HashMap<String, String> {
    let path = "data/jupiter_token_map.json";
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(entries) = serde_json::from_str::<Vec<Value>>(&content) {
            return entries
                .into_iter()
                .filter_map(|v| {
                    Some((
                        v.get("mint")?.as_str()?.to_string(),
                        v.get("name")?.as_str()?.to_string(),
                    ))
                })
                .collect();
        }
    }
    println!("⚠️  Could not load Jupiter token map from {}", path);
    HashMap::new()
}

fn load_cached_token_names() -> HashMap<String, String> {
    let path = "cache/token_names.json";
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(entries) = serde_json::from_str::<Vec<Value>>(&content) {
            return entries
                .into_iter()
                .filter_map(|v| {
                    let mint = v.get("account")?.as_str()?.to_string();
                    let name = v
                        .get("onChainMetadata")
                        .and_then(|m| m.get("metadata"))
                        .and_then(|m| m.get("name"))
                        .or_else(|| v.get("tokenInfo").and_then(|t| t.get("name")))
                        .and_then(|n| n.as_str())
                        .unwrap_or("UNKNOWN")
                        .to_string();
                    Some((mint, name))
                })
                .collect();
        }
    }
    HashMap::new()
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
    let use_jupiter_token_list = settings.use_jupiter_token_list.unwrap_or(true); // ✅ Use flag

    // --- Load Jupiter Token Map if enabled ---
    let jupiter_token_map: HashMap<String, String> = if use_jupiter_token_list {
        load_jupiter_token_map()
    } else {
        println!("Skipping Jupiter token list (use_jupiter_token_list=false)");
        HashMap::new()
    };

    // 👇 Everything else is untouched...
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
        let mut swaps = vec![];
        for tx in &transactions {
            if let Some(token_transfers) = tx.get("tokenTransfers").and_then(|v| v.as_array()) {
                if token_transfers.len() >= 2 {
                    let t1 = &token_transfers[0];
                    let t2 = &token_transfers[1];

                    let (sold, bought) = if t1.get("source").unwrap_or(&Value::Null) == wallet.as_str()
                        || t1.get("destination").unwrap_or(&Value::Null) == wallet.as_str()
                    {
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

    let cached_map = if use_token_cache {
        println!("Using cached token names...");
        load_cached_token_names()
    } else {
        HashMap::new()
    };

    let mut all_mints: HashSet<String> = HashSet::new();
    for swap in &swaps {
        all_mints.insert(swap.sold_mint.clone());
        all_mints.insert(swap.bought_mint.clone());
    }

    let mut mint_name_map: HashMap<String, String> = HashMap::new();
    let mut unknown_mints: Vec<String> = vec![];

    for mint in &all_mints {
        if let Some(name) = jupiter_token_map.get(mint) {
            mint_name_map.insert(mint.clone(), name.clone());
        } else if let Some(name) = cached_map.get(mint) {
            mint_name_map.insert(mint.clone(), name.clone());
        } else {
            unknown_mints.push(mint.clone());
        }
    }

    if !unknown_mints.is_empty() {
        println!("Querying {} unknown mints via Helius...", unknown_mints.len());
        let payload = json!({ "mintAccounts": unknown_mints });
        let url = format!("https://api.helius.xyz/v0/token-metadata?api-key={}", helius_api_key);
        let client = reqwest::blocking::Client::new();
        let res = client.post(&url).json(&payload).send();

        match res {
            Ok(response) => {
                if response.status().is_success() {
                    let token_data: Vec<Value> = response.json()?;
                    for entry in &token_data {
                        let mint = entry.get("account").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let name = entry
                            .get("onChainMetadata")
                            .and_then(|m| m.get("metadata"))
                            .and_then(|m| m.get("name"))
                            .or_else(|| entry.get("tokenInfo").and_then(|t| t.get("name")))
                            .and_then(|n| n.as_str())
                            .unwrap_or("UNKNOWN")
                            .to_string();
                        mint_name_map.insert(mint, name);
                    }
                    let mut file = File::create("cache/token_names.json")?;
                    write!(file, "{}", serde_json::to_string_pretty(&token_data)?)?;
                    println!("Token names written to cache/token_names.json");
                } else {
                    println!("Helius error: {}", response.status());
                }
            }
            Err(e) => println!("Error calling Helius: {}", e),
        }
    }

    let enriched_swaps: Vec<EnrichedSwapSummary> = swaps
        .into_iter()
        .map(|s| {
            let sold_token_name = jupiter_token_map
                .get(&s.sold_mint)
                .cloned()
                .or_else(|| mint_name_map.get(&s.sold_mint).cloned())
                .unwrap_or_else(|| "UNKNOWN".to_string());

            let bought_token_name = jupiter_token_map
                .get(&s.bought_mint)
                .cloned()
                .or_else(|| mint_name_map.get(&s.bought_mint).cloned())
                .unwrap_or_else(|| "UNKNOWN".to_string());

            EnrichedSwapSummary {
                timestamp: s.timestamp,
                signature: s.signature,
                sold_mint: s.sold_mint,
                sold_token_name,
                sold_amount: s.sold_amount,
                bought_mint: s.bought_mint,
                bought_token_name,
                bought_amount: s.bought_amount,
            }
        })
        .collect();

    let mut file = File::create(&swaps_path)?;
    write!(file, "{}", serde_json::to_string_pretty(&enriched_swaps)?)?;
    println!("✅ Updated {} with enriched swap data", swaps_path);

    Ok(())
}
