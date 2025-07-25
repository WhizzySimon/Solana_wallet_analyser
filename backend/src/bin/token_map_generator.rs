use serde_json::{Value, json};
use std::{fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load the full Jupiter token list
    let raw = fs::read_to_string("data/jupiter-solana-token-list.json")?;
    let full_list: Vec<Value> = serde_json::from_str(&raw)?;

    // Extract only mint and name
    let simplified: Vec<Value> = full_list
        .into_iter()
        .filter_map(|token| {
            let mint = token.get("address")?.as_str()?;
            let name = token.get("name")?.as_str()?;
            Some(json!({ "mint": mint, "name": name }))
        })
        .collect();

    // Save to file
    fs::create_dir_all("data")?;
    fs::write(
        "data/jupiter_token_map.json",
        serde_json::to_string_pretty(&simplified)?
    )?;

    println!("✅ Created data/jupiter_token_map.json with {} entries", simplified.len());
    Ok(())
}
