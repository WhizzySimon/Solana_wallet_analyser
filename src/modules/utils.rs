use std::{env, path::PathBuf};

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