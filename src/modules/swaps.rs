use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    path::Path,
};
use crate::modules::types::{Swap, SwapWithTokenNames};
use crate::modules::utils::load_config;


/// Loads Jupiter token map (mint → token name)
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

/// Loads resolved token names from cache (Helius results)
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
                        .and_then(|m| m.get("data"))
                        .and_then(|d| d.get("name"))
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

/// Extracts token amount using rawTokenAmount.decimals and tokenAmount
fn extract_token_amount(obj: &Value) -> f64 {
    if let Some(n) = obj.get("tokenAmount") {
        if let Some(as_f64) = n.as_f64() {
            return as_f64;
        }
        if let Some(as_str) = n.as_str() {
            return as_str.parse().unwrap_or(0.0);
        }
    }
    0.0
}


pub fn load_or_filter_and_enrich_swaps (transactions:&Vec<Value>) -> Result<Vec<Swap>, Box<dyn std::error::Error>> {

        // Load config
    let settings = load_config()?;

    let helius_api_key = settings.helius_api_key;
    let wallet = settings.wallet_address;
    let use_cached_swaps = settings.use_cached_swaps.unwrap_or(false);
    let use_token_cache = settings.use_token_cache.unwrap_or(false);
    let use_jupiter_token_list = settings.use_jupiter_token_list.unwrap_or(true);

    // Load Jupiter token map if enabled
    let jupiter_token_map: HashMap<String, String> = if use_jupiter_token_list {
        load_jupiter_token_map()
    } else {
        println!("Skipping Jupiter token list (use_jupiter_token_list=false)");
        HashMap::new()
    };

    // Load or filter/enrich swaps
    let swaps_path = format!("cache/swaps_{}.json", wallet);
    let swaps: Vec<Swap> = if use_cached_swaps && Path::new(&swaps_path).exists() {
        println!("♻️  Using cached swaps from {}", swaps_path);
        let file = fs::read_to_string(&swaps_path)?;
        serde_json::from_str(&file)?
    } else {
        // Step 1: Extract swap-like token transfers from raw txs
        println!("Filtering swaps from transactions...");
        let mut swaps = vec![];
        for tx in transactions {
            let wallet_lower = wallet.to_lowercase();
            if let Some(transfers) = tx.get("tokenTransfers").and_then(|v| v.as_array()) {
                let sold = transfers.iter().find(|tt| {
                    tt.get("fromUserAccount")
                        .and_then(|v| v.as_str())
                        .map(|s| s.eq_ignore_ascii_case(&wallet_lower))
                        .unwrap_or(false)
                });

                let bought = transfers.iter().find(|tt| {
                    tt.get("toUserAccount")
                        .and_then(|v| v.as_str())
                        .map(|s| s.eq_ignore_ascii_case(&wallet_lower))
                        .unwrap_or(false)
                });

                if let (Some(s), Some(b)) = (sold, bought) {
                    let sold_amt = extract_token_amount(s);
                    let bought_amt = extract_token_amount(b);

                    swaps.push(Swap {
                        timestamp: tx.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0),
                        signature: tx.get("signature").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        sold_mint: s.get("mint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        sold_amount: sold_amt,
                        bought_mint: b.get("mint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        bought_amount: bought_amt,
                    });

                }

            }
        }

        println!("🔎 Found {} swaps", swaps.len());
        println!("🧠 Resolving token names for swaps...");

        // Step 2: Resolve mint names using Jupiter, cache, and Helius fallback
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

        // Step 3: Call Helius to resolve remaining unknown mints
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

        // Step 4: Enrich swaps with token names
        let enriched_swaps: Vec<SwapWithTokenNames> = swaps
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

                SwapWithTokenNames {
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

        // Step 5: Write enriched swaps to cache
        let mut file = File::create(&swaps_path)?;
        write!(file, "{}", serde_json::to_string_pretty(&enriched_swaps)?)?;
        println!("✅ Enriched swaps written to {}", swaps_path);

        // Return simplified struct so rest of program can use it uniformly
        enriched_swaps.iter().map(|e| Swap {
            timestamp: e.timestamp,
            signature: e.signature.clone(),
            sold_mint: e.sold_mint.clone(),
            sold_amount: e.sold_amount,
            bought_mint: e.bought_mint.clone(),
            bought_amount: e.bought_amount,
        }).collect()
    };
    Ok(swaps)
}