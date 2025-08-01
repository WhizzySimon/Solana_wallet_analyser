use std::{env, path::PathBuf};
use crate::modules::types::Config;
use std::collections::HashMap;

pub fn get_project_root() -> PathBuf {
    let mut path = env::current_exe().expect("Can't get current exe path");
    while !path.join("Cargo.toml").exists() {
        if !path.pop() {
            panic!("Could not locate project root");
        }
    }
    path
}

pub fn get_named_swaps_path(wallet: &str) -> String {
    format!("cache/swaps_named_{}.json", wallet)
}
pub fn get_priced_swaps_path(wallet: &str) -> String {
    format!("cache/swaps_priced_{}.json", wallet)
}

pub fn load_config () -> Result<Config, Box<dyn std::error::Error>> {
        // Load config
    let settings = config::Config::builder()
        .add_source(config::File::with_name("config/config"))
        .build()?
        .try_deserialize()?;
    Ok(settings)
}

pub fn build_decimals_map(path: &str) -> Result<HashMap<String, u8>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let tokens: Vec<serde_json::Value> = serde_json::from_reader(file)?;
    let mut map = HashMap::new();

    for token in tokens {
        if let (Some(mint), Some(decimals)) = (
            token.get("mint").and_then(|v| v.as_str()),
            token.get("decimals").and_then(|v| v.as_u64()),
        ) {
            map.insert(mint.to_string(), decimals as u8);
        }
    }

    Ok(map)
}