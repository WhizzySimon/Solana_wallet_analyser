use serde::Deserialize;
use std::{collections::HashMap, fs, path::Path};
use tokio;

const PAIRS_URL: &str = "https://api.dexscreener.com/latest/dex/pairs/solana";
const CANDLE_API: &str = "https://api.dexscreener.com/latest/dex/candles/solana";

#[derive(Debug, Deserialize)]
struct Swap {
    timestamp: u64,
    sold_mint: String,
    bought_mint: String,
}

#[derive(Debug, Deserialize)]
struct DexPairsResponse {
    pairs: Vec<DexPair>,
}

#[derive(Debug, Deserialize)]
struct DexPair {
    pair_address: String,
    #[serde(rename = "baseToken")]
    base_token: DexToken,
    #[serde(rename = "quoteToken")]
    quote_token: DexToken,
    liquidity: Option<Liquidity>,
}

#[derive(Debug, Deserialize)]
struct DexToken {
    address: String,
    symbol: String,
}

#[derive(Debug, Deserialize)]
struct Liquidity {
    usd: f64,
}

#[derive(Debug, Deserialize)]
struct Candle {
    t: u64,
    c: String,
}

#[derive(Debug, Deserialize)]
struct CandleResponse {
    pairs: Vec<Candle>,
}

fn find_first_swap_file() -> Option<String> {
    let dir = Path::new("cache");
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name.starts_with("swaps_") && name.ends_with(".json") {
            return Some(format!("cache/{}", name));
        }
    }
    None
}

async fn find_best_usd_or_sol_pair(token_mint: &str) -> Option<String> {
    let res = reqwest::get(PAIRS_URL).await.ok()?;
    let json: DexPairsResponse = res.json().await.ok()?;

    let mut usd_candidates: Vec<&DexPair> = json
        .pairs
        .iter()
        .filter(|pair| {
            pair.base_token.address == token_mint
                && pair.quote_token.symbol.to_uppercase().contains("USD")
        })
        .collect();

    usd_candidates.sort_by(|a, b| {
        let a_liq = a.liquidity.as_ref().map_or(0.0, |l| l.usd);
        let b_liq = b.liquidity.as_ref().map_or(0.0, |l| l.usd);
        b_liq.partial_cmp(&a_liq).unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some(best) = usd_candidates.first() {
        return Some(best.pair_address.clone());
    }

    let mut sol_candidates: Vec<&DexPair> = json
        .pairs
        .iter()
        .filter(|pair| {
            pair.base_token.address == token_mint
                && pair.quote_token.symbol.to_uppercase() == "SOL"
        })
        .collect();

    sol_candidates.sort_by(|a, b| {
        let a_liq = a.liquidity.as_ref().map_or(0.0, |l| l.usd);
        let b_liq = b.liquidity.as_ref().map_or(0.0, |l| l.usd);
        b_liq.partial_cmp(&a_liq).unwrap_or(std::cmp::Ordering::Equal)
    });

    sol_candidates.first().map(|p| p.pair_address.clone())
}

async fn fetch_candles_for_timestamps(
    pair_address: &str,
    timestamps: &[u64],
) -> Result<HashMap<u64, f64>, Box<dyn std::error::Error>> {
    if timestamps.is_empty() {
        return Ok(HashMap::new());
    }

    let min_ts = *timestamps.iter().min().unwrap_or(&0);
    let max_ts = *timestamps.iter().max().unwrap_or(&0) + 60;

    let url = format!("{}/{}?interval=1m&from={}&to={}", CANDLE_API, pair_address, min_ts, max_ts);
    let res = reqwest::get(&url).await?;
    let candle_resp: CandleResponse = res.json().await?;

    let mut out = HashMap::new();
    for ts in timestamps {
        if let Some(candle) = candle_resp.pairs.iter().find(|c| c.t == *ts) {
            let price: f64 = candle.c.parse()?;
            out.insert(*ts, price);
        }
    }

    Ok(out)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(swaps_path) = find_first_swap_file() else {
        eprintln!("❌ No swaps_*.json file found in cache/");
        return Ok(());
    };

    let content = fs::read_to_string(&swaps_path)?;
    let swaps: Vec<Swap> = serde_json::from_str(&content)?;

    let mut token_times: HashMap<String, Vec<u64>> = HashMap::new();
    for swap in swaps.iter() {
        token_times
            .entry(swap.sold_mint.clone())
            .or_default()
            .push(swap.timestamp);
        token_times
            .entry(swap.bought_mint.clone())
            .or_default()
            .push(swap.timestamp);
    }

    let mut result: HashMap<String, HashMap<u64, f64>> = HashMap::new();

    for (mint, timestamps) in token_times {
        println!("🔍 Looking up best pair for: {}", mint);
        if let Some(pair_address) = find_best_usd_or_sol_pair(&mint).await {
            println!("✅ Found pair: {}", pair_address);
            let prices = fetch_candles_for_timestamps(&pair_address, &timestamps).await?;
            result.insert(mint, prices);
        } else {
            println!("❌ No pair found for: {}", mint);
        }
    }

    fs::create_dir_all("output")?;
    let json = serde_json::to_string_pretty(&result)?;
    fs::write("output/prices.json", json)?;
    println!("✅ Prices saved to output/prices.json");
    Ok(())
}
