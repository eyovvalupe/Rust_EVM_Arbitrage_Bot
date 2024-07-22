use serde::Deserialize;
use reqwest::Client;
use std::collections::HashMap;

use ethers::{
  types::H160,
};


#[derive(Deserialize)]
struct PoolInfo {
    id: String,
    token0: TokenInfo,
    token1: TokenInfo,
    liquidity: String,
    sqrtPrice: String,
    volume24h: String, // Assuming the API provides 24-hour volume
}

#[derive(Deserialize)]
struct TokenInfo {
    symbol: String,
    address: String,
}

fn filter_pools(pools: Vec<PoolInfo>, min_liquidity: u128, min_volume: u128) -> Vec<PoolInfo> {
    pools.into_iter()
        .filter(|pool| pool.liquidity.parse::<u128>().unwrap_or(0) >= min_liquidity)
        .filter(|pool| pool.volume24h.parse::<u128>().unwrap_or(0) >= min_volume)
        .collect()
}

pub fn get_pools(token_a: H160, token_b: H160) -> Vec<Pool> {
    let client = Client::new();
    
    // Example GraphQL query
    let query = r#"
    {
      pools(where: { token0: "TOKEN_A_ADDRESS", token1: "TOKEN_B_ADDRESS" }) {
        id
        token0 {
          symbol
        }
        token1 {
          symbol
        }
        liquidity
        sqrtPrice
        volume24h
      }
    }
    "#;

    // Fetch pool info
    let pools = get_pool_info(&client, "https://api.thegraph.com/subgraphs/name/uniswap/uniswap-v3", query).await.unwrap();
    
    // Define thresholds
    let min_liquidity = 1_000_000_u128; // Example threshold for liquidity
    let min_volume = 10_000_u128; // Example threshold for 24h volume
    
    // Filter pools by liquidity and recent activity
    let filtered_pools = filter_pools(pools, min_liquidity, min_volume);

    for pool in filtered_pools {
        println!("Pool ID: {}, Liquidity: {}, 24h Volume: {}", pool.id, pool.liquidity, pool.volume24h);
    }
}