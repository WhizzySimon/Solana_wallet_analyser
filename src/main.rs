
use wallet_analyzer::modules::transactions::get_transactions;
use wallet_analyzer::modules::swaps::filter_and_enrich_swaps;



fn main() -> Result<(), Box<dyn std::error::Error>> {

    let transactions = get_transactions()?;
    println!("Total transactions loaded: {}", transactions.len());

    let swaps = filter_and_enrich_swaps(&transactions)?;
    println!("Total swaps loaded: {}", swaps.len());

    Ok(())
}
