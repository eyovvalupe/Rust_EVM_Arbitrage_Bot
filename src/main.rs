use std::{str::FromStr, sync::Arc};

use constants::{ETH, USDC, V3_QUOTER_ADDRESS, WETH};
use dotenv::dotenv;
use ethers::{
    providers::{Http, Provider},
    types::H160,
};

pub mod abi;
pub mod config;
pub mod constants;
pub mod error;
pub mod execution;
pub mod markets;
pub mod order;
pub mod tests;
pub mod transactions;

use crate::tests::{
//     simulate_swap,
    swap_calldata,
//     discover_erc_4626_vaults,
//     discover_factories,
};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt::init();

    let rpc_endpoint: String = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let ws_endpoint: String = std::env::var("ETHEREUM_WS_ENDPOINT")?;

    // Initialize a new configuration
    let configuration = config::Config::new(rpc_endpoint, ws_endpoint);
    let middleware = Arc::new(Provider::<Http>::try_from(configuration.http_endpoint.clone())?);

    let weth = H160::from_str(WETH)?;
    let usdc = H160::from_str(USDC).unwrap();
    let markets = markets::get_market(weth, usdc, &configuration.dexes, middleware).await?;

    match markets {
        Some(markets) => {
            println!("Found markets: {:?}", markets.keys());
        }
        None => {
            println!("No markets found!");
        }
    }

    // let v3_quoter_address: H160 = H160::from_str(V3_QUOTER_ADDRESS).unwrap();

    // simulate_swap::try_sample_swap_simulate(rpc_endpoint.clone()).await?;
    swap_calldata::try_swap_calldata(configuration.http_endpoint.clone()).await?;
    // discover_erc_4626_vaults::try_discorver_erc_4626_vaults(rpc_endpoint.clone()).await?;
    // discover_factories::try_discorver_factories(rpc_endpoint.clone()).await?;

    Ok(())
}
