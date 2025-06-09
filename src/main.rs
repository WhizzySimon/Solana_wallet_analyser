#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, fs::File, io::BufReader, env};
use reqwest::Client;
use tokio::time::{sleep, Duration};
use futures::future::join_all;

#[derive(Debug, Deserialize)]
struct Settings {
    helius_api_key: String,
    birdeye_api_key: String,
    wallet_address: String,
}

#[derive(Debug, Deserialize)]
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
    sold_token: String,
    sold_amount: f64,
    bought_mint: String,
    bought_token: String,
    bought_amount: f64,
}

#[derive(Debug, Deserialize)]
struct TokenList {
    tokens: Vec<TokenInfo>,
}

#[derive(Debug, Deserialize)]
struct TokenInfo {
    address: String,
    symbol: String,
}

fn load_token_map(path: &str) -> HashMap<String, String> {
    let file = File::open(path).unwrap_or_else(|e| panic!("Could not open token list file at '{}': {}", path, e));
    let reader = BufReader::new(file);
    let token_list: TokenList = serde_json::from_reader(reader).expect("Could not parse token list");
    token_list.tokens.into_iter()
        .map(|t| (t.address, t.symbol))
        .collect()
}

fn load_settings_from_env() -> Result<Settings, String> {
    let helius_api_key = env::var("HELIUS_API_KEY")
        .map_err(|_| "HELIUS_API_KEY environment variable not set".to_string())?;
    let birdeye_api_key = env::var("BIRDEYE_API_KEY")
        .map_err(|_| "BIRDEYE_API_KEY environment variable not set".to_string())?;
    let wallet_address = env::var("WALLET_ADDRESS")
        .map_err(|_| "WALLET_ADDRESS environment variable not set".to_string())?;

    Ok(Settings {
        helius_api_key,
        birdeye_api_key,
        wallet_address,
    })
}

async fn get_token_symbol_birdeye(client: &Client, mint: &str, api_key: &str) -> Option<String> {
    let url = format!("https://public-api.birdeye.so/public/token/{}", mint);
    let resp = client
        .get(&url)
        .header("X-API-KEY", api_key)
        .send()
        .await
        .ok()?;
    let json = resp.json::<serde_json::Value>().await.ok()?;
    json["data"]["symbol"].as_str().map(|s| s.to_string())
}

async fn get_token_symbol_dexscreener(client: &Client, mint: &str) -> Option<String> {
    let url = format!("https://api.dexscreener.com/latest/dex/tokens/{}", mint);
    let resp = client.get(&url).send().await.ok()?;
    let json = resp.json::<serde_json::Value>().await.ok()?;
    json["pairs"]
        .as_array()
        .and_then(|pairs| pairs.iter().find_map(|pair| pair["baseToken"]["symbol"].as_str()))
        .map(|s| s.to_string())
}

#[tokio::main]
async fn main() {
    // Load settings from environment variables
    let settings: Settings = match load_settings_from_env() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let client = Client::new();

    // Load token map
    let token_map = load_token_map("data/solana.tokenlist.json");

    // Load swaps
    let file = File::open("output/swaps_extracted.json").expect("Could not open output/swaps_extracted.json");
    let reader = BufReader::new(file);
    let swaps: Vec<SwapSummary> = serde_json::from_reader(reader).expect("Could not parse swaps");

    // Collect unknown mints
    let unknown_mints: HashSet<String> = swaps.iter()
        .flat_map(|s| vec![&s.sold_mint, &s.bought_mint])
        .filter(|mint| !token_map.contains_key(*mint))
        .map(|s| s.to_string())
        .collect();

    // Resolve unknown symbols in batches (Birdeye, then Dexscreener)
    let mut symbol_cache: HashMap<String, String> = HashMap::new();
    let mints: Vec<&String> = unknown_mints.iter().collect();
    const BIRDEYE_BATCH: usize = 25;
    for chunk in mints.chunks(BIRDEYE_BATCH) {
        let tasks: Vec<_> = chunk
            .iter()
            .map(|mint| {
                let client = client.clone();
                let mint = (*mint).clone();
                let birdeye_key = settings.birdeye_api_key.clone();
                tokio::spawn(async move {
                    let symbol = match get_token_symbol_birdeye(&client, &mint, &birdeye_key).await {
                        Some(sym) => Some(sym),
                        None => get_token_symbol_dexscreener(&client, &mint).await,
                    };
                    (mint, symbol)
                })
            })
            .collect();

        let batch_results = join_all(tasks).await;
        for res in batch_results {
            if let Ok((mint, Some(symbol))) = res {
                symbol_cache.insert(mint, symbol);
            }
        }
        sleep(Duration::from_secs(1)).await;
    }

    // Enrich swaps
    let mut enriched_swaps = Vec::new();
    for swap in swaps {
        let sold_symbol = token_map
            .get(&swap.sold_mint)
            .cloned()
            .or_else(|| symbol_cache.get(&swap.sold_mint).cloned())
            .unwrap_or("UNKNOWN".to_string());

        let bought_symbol = token_map
            .get(&swap.bought_mint)
            .cloned()
            .or_else(|| symbol_cache.get(&swap.bought_mint).cloned())
            .unwrap_or("UNKNOWN".to_string());

        enriched_swaps.push(EnrichedSwapSummary {
            timestamp: swap.timestamp,
            signature: swap.signature,
            sold_mint: swap.sold_mint,
            sold_token: sold_symbol,
            sold_amount: swap.sold_amount,
            bought_mint: swap.bought_mint,
            bought_token: bought_symbol,
            bought_amount: swap.bought_amount,
        });
    }

    // Write result
    std::fs::create_dir_all("output").ok();
    let outfile = File::create("output/enriched_swaps.json").expect("Could not create output/enriched_swaps.json");
    serde_json::to_writer_pretty(outfile, &enriched_swaps).expect("Could not write enriched_swaps.json");

    println!("Success: output/enriched_swaps.json written with {} swaps", enriched_swaps.len());
}
