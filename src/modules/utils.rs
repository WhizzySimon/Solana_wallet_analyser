use std::{env, path::PathBuf};
use crate::modules::types::Settings;


pub fn get_project_root() -> PathBuf {
    let mut path = env::current_exe().expect("Can't get current exe path");
    while !path.join("Cargo.toml").exists() {
        if !path.pop() {
            panic!("Could not locate project root");
        }
    }
    path
}

pub fn get_swaps_path(wallet: &str) -> String {
    format!("cache/swaps_{}.json", wallet)
}

pub fn load_config () -> Result<Settings, Box<dyn std::error::Error>> {
        // Load config
    let settings = config::Config::builder()
        .add_source(config::File::with_name("config/config"))
        .build()?
        .try_deserialize()?;
    Ok(settings)
}