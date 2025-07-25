
use crate::modules::types::{RawTxn, Swap, NamedSwap};
use crate::modules::utils::get_named_swaps_path;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use serde_json::json;
use serde_json::Value;

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


pub fn filter_and_name_swaps(
    transactions: &Vec<RawTxn>,
) -> Result<Vec<NamedSwap>, Box<dyn std::error::Error>> {

    let settings = crate::modules::utils::load_config()?;
    let use_cached_swaps_raw = settings.use_cached_named_swaps.unwrap_or(true);
    let use_token_cache = settings.use_token_cache.unwrap_or(true);
    let use_jupiter_token_list = settings.use_jupiter_token_list.unwrap_or(true);
    let helius_api_key = settings.helius_api_key;
    let wallet = settings.wallet_address;
    let wallet_lower = wallet.to_lowercase();
    let swaps_path_raw = get_named_swaps_path(&wallet);
    let write_cache_files = settings.write_cache_files.unwrap_or(false);

    let swaps: Vec<NamedSwap> = if use_cached_swaps_raw && Path::new(&swaps_path_raw).exists() {
        println!("♻️  Using cached swaps from {}", swaps_path_raw);
        let file = fs::read_to_string(&swaps_path_raw)?;
        serde_json::from_str(&file)?
    } else {
        println!("🔍 Filtering swaps from {} transactions...", transactions.len());
        let mut raw_swaps = vec![];

        for tx in transactions.iter() {

            if tx.token_transfers.is_empty() {
                continue;
            }

            let sold = tx.token_transfers.iter()
                .find(|t| t.from_user_account.eq_ignore_ascii_case(&wallet_lower));

            let bought = tx.token_transfers.iter()
                .find(|t| t.to_user_account.eq_ignore_ascii_case(&wallet_lower));

            if let (Some(s), Some(b)) = (sold, bought) {
                raw_swaps.push(Swap {
                    timestamp: tx.timestamp.unwrap_or(0),
                    signature: tx.signature.clone(),
                    sold_mint: s.mint.clone(),
                    sold_amount: s.token_amount,
                    bought_mint: b.mint.clone(),
                    bought_amount: b.token_amount,
                });
            }

        }



        println!("🔎 Found {} swaps", raw_swaps.len());
        println!("🧠 Resolving token names for swaps...");

        let jupiter_token_map = if use_jupiter_token_list {
            load_jupiter_token_map()
        } else {
            println!("Skipping Jupiter token list (use_jupiter_token_list=false)");
            HashMap::new()
        };

        let cached_map = if use_token_cache {
            println!("Using cached token names...");
            load_cached_token_names()
        } else {
            HashMap::new()
        };

        let mut all_mints = HashSet::new();
        for s in &raw_swaps {
            all_mints.insert(s.sold_mint.clone());
            all_mints.insert(s.bought_mint.clone());
        }

        let mut mint_name_map: HashMap<String, String> = HashMap::new();
        let mut unknown_mints = vec![];
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
                        let token_data: Vec<serde_json::Value> = response.json()?;
                        for entry in &token_data {
                            let mint = entry.get("account").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let name = entry
                                .get("onChainMetadata")
                                .and_then(|m| m.get("metadata"))
                                .and_then(|m| m.get("data"))
                                .and_then(|d| d.get("name"))
                                .or_else(|| entry.get("tokenInfo").and_then(|t| t.get("name")))
                                .and_then(|n| n.as_str())
                                .unwrap_or("UNKNOWN")
                                .to_string();
                            mint_name_map.insert(mint, name);
                        }
                        let mut file = File::create("cache/token_names.json")?;
                        write!(file, "{}", serde_json::to_string_pretty(&token_data)?)?;
                        println!("✅ Token names written to cache/token_names.json");
                    } else {
                        println!("⚠️ Helius error: {}", response.status());
                    }
                }
                Err(e) => println!("⚠️ Error calling Helius: {}", e),
            }
        }

        let enriched: Vec<NamedSwap> = raw_swaps
            .into_iter()
            .map(|s| {
                let sold_token_name = mint_name_map.get(&s.sold_mint).cloned().unwrap_or_else(|| "UNKNOWN".to_string());
                let bought_token_name = mint_name_map.get(&s.bought_mint).cloned().unwrap_or_else(|| "UNKNOWN".to_string());
                NamedSwap {
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
        if write_cache_files {
            let mut file = File::create(&swaps_path_raw)?;
            write!(file, "{}", serde_json::to_string_pretty(&enriched)?)?;
            println!("✅ Enriched swaps written to {}", swaps_path_raw);
        }
        else {
            println!("Filtered and named {} swaps.", enriched.len());
        }
        enriched
    };

    Ok(swaps)
}
