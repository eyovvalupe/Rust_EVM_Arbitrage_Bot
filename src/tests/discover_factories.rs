use amms::discovery::factory::{discover_factories, DiscoverableFactory};
use ethers::providers::{Http, Provider};
use std::sync::Arc;

pub async fn try_discorver_factories(rpc_endpoint: String) -> eyre::Result<()> {
    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

    // Find all UniswapV2 and UniswapV3 compatible factories and filter out matches with less than 1000 AMMs
    let number_of_amms_threshold = 1000;
    let factories = discover_factories(
        vec![
            DiscoverableFactory::UniswapV2Factory,
            DiscoverableFactory::UniswapV3Factory,
        ],
        number_of_amms_threshold,
        provider,
        50000,
    )
    .await?;

    println!("Factories: {:?}", factories);

    Ok(())
}
