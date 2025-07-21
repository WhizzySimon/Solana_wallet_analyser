
use wallet_analyzer::modules::transactions::get_transactions;
use wallet_analyzer::modules::swaps::filter_and_enrich_swaps;
use wallet_analyzer::modules::prices::run_prices;



fn main() -> Result<(), Box<dyn std::error::Error>> {

    let transactions = get_transactions()?;
    println!("Total transactions fetched/loaded: {}", transactions.len());

    let swaps_with_token_names = filter_and_enrich_swaps(&transactions)?;
    println!("Total swaps with token names filtered/loaded: {}", swaps_with_token_names.len());

    let _swaps_with_prices = run_prices(&swaps_with_token_names);

    Ok(())
}
