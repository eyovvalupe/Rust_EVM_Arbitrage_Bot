use cfmms::pool::{Pool, UniswapV2Pool};
use ethers::providers::Middleware;
use ethers::types::{H160, U256};
use std::collections::HashMap;
use std::{str::FromStr, sync::Arc};
use crate::{
    config::Config,
    constants::WETH,
    markets,
    abi::IUniswapV3Quoter,
    error::ExecutorError,
};

pub async fn find_markets_and_route<M: 'static + Middleware>(
    token_in: H160,
    token_out: H160,
    configuration: &Config,
    middleware: Arc<M>,
) -> Result<HashMap<H160, Pool>, ExecutorError<M>> {
    let markets = markets::get_market(
        match token_in.is_zero() {
            true => H160::from_str(WETH).unwrap(),
            false => token_in,
        },
        match token_out.is_zero() {
            true => H160::from_str(WETH).unwrap(),
            false => token_out,
        },
        &configuration.dexes,
        middleware,
    )
    .await?;

    match markets {
        Some(markets) => {
            println!("Found markets: {:?}", markets.keys());
            Ok(markets)
        }
        None => {
            println!("No markets found!");
            Err(ExecutorError::MarketDoesNotExistForPair(
                token_in, token_out,
            ))
        }
    }
}

pub async fn find_best_route<M: 'static + Middleware>(
    markets: HashMap<H160, Pool>,
    token_in: H160,
    amount: U256,
    middleware: Arc<M>,
) -> Result<(Pool, U256), ExecutorError<M>> {
    pub const V3_QUOTER_ADDRESS: H160 = H160([
        178, 115, 8, 249, 249, 13, 96, 116, 99, 187, 51, 234, 27, 235, 180, 28, 39, 206, 90, 182,
    ]);
    let mut best_amount_out = U256::zero();
    let mut best_pool = Pool::UniswapV2(UniswapV2Pool::default());

    for pool in markets.values() {
        let pool = *pool;
        match pool {
            Pool::UniswapV2(_) => {
                let swap_amount_out = pool
                    .simulate_swap(token_in, amount, middleware.clone())
                    .await?;
                if swap_amount_out > best_amount_out {
                    best_amount_out = swap_amount_out;
                    best_pool = pool;
                }
            }

            Pool::UniswapV3(uniswap_v3_pool) => {
                let uniswap_v3_quoter =
                    IUniswapV3Quoter::new(V3_QUOTER_ADDRESS, middleware.clone());

                let (token_in, token_out) = if token_in == uniswap_v3_pool.token_a {
                    (uniswap_v3_pool.token_a, uniswap_v3_pool.token_b)
                } else {
                    (uniswap_v3_pool.token_b, uniswap_v3_pool.token_a)
                };

                let swap_amount_out = uniswap_v3_quoter
                    .quote_exact_input_single(
                        token_in,
                        token_out,
                        pool.fee(),
                        amount,
                        U256::zero(),
                    )
                    .call()
                    .await?;

                if swap_amount_out > best_amount_out {
                    best_amount_out = swap_amount_out;
                    best_pool = pool;
                }
            }
        };
    }

    Ok((best_pool, best_amount_out))
}
