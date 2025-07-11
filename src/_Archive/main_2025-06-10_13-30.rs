#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::{File, create_dir_all},
    io::{Read, Write},
    path::Path,
};

#[derive(Debug, Deserialize)]
struct Settings {
    helius_api_key: String,
    birdeye_api_key: String,
    wallet_address: String,
}

fn ensure_dir_exists<P: AsRef<Path>>(dir: P) {
    if let Err(e) = create_dir_all(&dir) {
        eprintln!("Failed to create directory {:?}: {:?}", dir.as_ref(), e);
    }
}

fn get_token_accounts(transactions: &[serde_json::Value], wallet: &str) -> HashSet<String> {
    let mut token_accounts = HashSet::new();

    for tx in transactions {
        // SPL tokens
        if let Some(token_transfers) = tx.get("tokenTransfers").and_then(|t| t.as_array()) {
            for t in token_transfers {
                // If you see "userAccountOwner" field in your debug print, re-enable the line below!
                // let owner = t.get("userAccountOwner").and_then(|v| v.as_str()).unwrap_or("");
                // if owner == wallet {
                    let from_user = t.get("fromUserAccount").and_then(|v| v.as_str()).unwrap_or("");
                    let to_user = t.get("toUserAccount").and_then(|v| v.as_str()).unwrap_or("");
                    if !from_user.is_empty() {
                        token_accounts.insert(from_user.to_string());
                    }
                    if !to_user.is_empty() {
                        token_accounts.insert(to_user.to_string());
                    }
                // }
            }
        }
        // Native SOL
        if let Some(native_transfers) = tx.get("nativeTransfers").and_then(|t| t.as_array()) {
            for t in native_transfers {
                let from_user = t.get("fromUserAccount").and_then(|v| v.as_str()).unwrap_or("");
                let to_user = t.get("toUserAccount").and_then(|v| v.as_str()).unwrap_or("");
                if from_user == wallet {
                    token_accounts.insert(from_user.to_string());
                }
                if to_user == wallet {
                    token_accounts.insert(to_user.to_string());
                }
            }
        }
    }
    token_accounts
}

#[tokio::main]
async fn main() {
    ensure_dir_exists("cache");
    ensure_dir_exists("data");
    ensure_dir_exists("output");

    // Load config/settings
    let settings_path = "config/config.toml";
    let settings: Settings = config::Config::builder()
        .add_source(config::File::with_name(settings_path))
        .build()
        .unwrap()
        .try_deserialize()
        .unwrap();

    // ---- Load transactions from cache (adjust as needed) ----
    let mut file = File::open("output/transactions.json").expect("Cannot open output/transactions.json");
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("Failed to read transactions.json");
    let transactions: Vec<serde_json::Value> = serde_json::from_str(&contents).expect("Failed to parse transactions.json");

    println!("Loaded {} transactions.", transactions.len());

    // --- DEBUG: Print structure of first few transactions ---
    let print_n = 3.min(transactions.len());
    let mut debug_file = File::create("output/transactions_debug.json").expect("Failed to create debug file");
    for i in 0..print_n {
        let entry = format!(
            "TRANSACTION {} STRUCTURE:\n{}\n\n",
            i + 1,
            serde_json::to_string_pretty(&transactions[i]).unwrap()
        );
        debug_file.write_all(entry.as_bytes()).expect("Failed to write debug info");
    }


    // ---- Extract token accounts by wallet (SOL and SPL) ----
    let my_token_accounts = get_token_accounts(&transactions, &settings.wallet_address);
    println!("Found {} possible token accounts related to wallet.", my_token_accounts.len());

    // --- Continue here with any swap extraction logic you'd like ---
    // For now, we're just debugging the data structure and collection logic!
}
