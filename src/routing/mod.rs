use cfmms::pool::{Pool, UniswapV2Pool, UniswapV3Pool};
use ethers::providers::Middleware;
use ethers::types::{H160, U256};
use ethers::contract::abigen;
use std::collections::HashMap;
use std::{str::FromStr, sync::Arc};

use crate::config::Config;
use crate::constants::{WETH, V3_QUOTER_ADDRESS};
use crate::error::ExecutorError;
use crate::markets;

abigen!(
    IUniswapV3Quoter,
    r#"[
        function quoteExactInputSingle(address tokenIn, address tokenOut, uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96) external returns (uint256 amountOut)
    ]"#,
);

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
) -> Result<(Pool, U256, Pool, U256), ExecutorError<M>> {
    let mut best_amount_out_v2 = U256::zero();
    let mut best_amount_out_v3 = U256::zero();
    let mut best_pool_v2 = Pool::UniswapV2(UniswapV2Pool::default());
    let mut best_pool_v3 = Pool::UniswapV3(UniswapV3Pool::default());
    let mut handles: Vec<Pool> = vec![];

    for pool in markets.values() {
        let pool = *pool;
        match pool {
            Pool::UniswapV2(_) => {
                let swap_amount_out = pool
                    .simulate_swap(token_in, amount, middleware.clone())
                    .await?;
                if swap_amount_out > best_amount_out_v2 {
                    best_amount_out_v2 = swap_amount_out;
                    best_pool_v2 = pool;
                }
            }

            Pool::UniswapV3(_uniswap_v3_pool) => {
                    let uniswap_v3_quoter = IUniswapV3Quoter::new(
                        H160::from_str(V3_QUOTER_ADDRESS).unwrap(),
                        middleware.clone(),
                    );

                    let (token_in, token_out) = if token_in == _uniswap_v3_pool.token_a {
                        (_uniswap_v3_pool.token_a, _uniswap_v3_pool.token_b)
                    } else {
                        (_uniswap_v3_pool.token_b, _uniswap_v3_pool.token_a)
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

                    if swap_amount_out > best_amount_out_v3 {
                        best_amount_out_v3 = swap_amount_out;
                        best_pool_v3 = pool;
                    }                        
            }
        };
    }

    Ok((best_pool_v2, best_amount_out_v2, best_pool_v3, best_amount_out_v3))
}
