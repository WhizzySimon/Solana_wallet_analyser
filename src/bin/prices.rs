use wallet_analyzer::modules::prices::run_prices;

fn main() {
    if let Err(e) = run_prices() {
        eprintln!("Error: {:?}", e);
    }
}
