use std::{str::FromStr, sync::Arc};

use cfmms::pool::Pool;
use ethabi::Token;
use ethers::{
    abi::AbiEncode, providers::Middleware, types::{H160, I256, U256}
};

use crate::{
    abi::IERC20_ABI,
    config::{self},
    constants::{FIFTH_WEB_MULTICALL, UNISWAP_V2_FEE, WETH},
    error::ExecutorError,
    routing::{find_best_a_to_b_route, find_a_to_b_markets_and_route, find_a_to_x_to_b_markets_and_route, find_best_a_to_x_to_b_route, find_all_markets},
};

pub(crate) mod types;

use types::{SwapData, SwapMultiCall};

//Construct a final swap transaction calldata
pub async fn swap_transaction_calldata<M: 'static + Middleware>(
    configuration: &config::Config,
    token_in: H160,
    token_out: H160,
    token_x: H160,
    amount_in: U256,
    slippage: u32,
    receiver: H160,
    middleware: Arc<M>,
) -> Result<(SwapData, SwapMultiCall), ExecutorError<M>> {
    let mut amount_fixed_for_fee = amount_in;
    let mut protocol_fee = U256::zero();
    let bribe = U256::zero();
    let to = H160::from_str(FIFTH_WEB_MULTICALL).unwrap();

    if token_in.is_zero() {
        amount_fixed_for_fee -= amount_fixed_for_fee / 100;
        protocol_fee = amount_in - amount_fixed_for_fee;
    }

    let all_margets = find_all_markets(token_in, token_out, configuration, middleware.clone()).await?;

    let multi_markets = 
        find_a_to_x_to_b_markets_and_route(token_in, token_out, token_x, configuration, middleware.clone()).await?;
    
    let (amounts_in, axb_amounts_out, axb_route) = 
        find_best_a_to_x_to_b_route(token_in, token_out, token_x, amount_in, &multi_markets, middleware.clone()).await?;
    
    let markets =
        find_a_to_b_markets_and_route(token_in, token_out, configuration, middleware.clone()).await?;

    let (ab_pool, ab_amount_out) =
        find_best_a_to_b_route(markets, token_in, amount_fixed_for_fee, middleware.clone()).await?;

    // Construct SwapCallData
    let mut swap_data: SwapData = SwapData {
        token_in: None,
        token_out: None,
        amount_in: None,
        amount_out_min: None,
        protocol_fee: None,
        bribe,
        affiliate: U256::zero(),
        referrer: U256::zero(),
    };

    let slippage_used = match slippage {
        0 => 95 * 100,
        _ => slippage,
    };
    let mut ab_route = vec![];
    let mut best_route = vec![];
    let mut best_amount_out = U256::zero();

    ab_route.push(ab_pool);
    if axb_amounts_out.last().unwrap() > &ab_amount_out {
        best_amount_out = *axb_amounts_out.last().unwrap();
        best_route = axb_route;
    }
    else {
        best_amount_out = ab_amount_out;
        best_route = ab_route;
    }

    let amount_out_min = best_amount_out - best_amount_out * slippage_used / 10000;

    if token_in.is_zero() {
        swap_data.token_out = Some(token_out);
        swap_data.amount_out_min = Some(amount_out_min);
        swap_data.protocol_fee = Some(protocol_fee);
    } else if token_out.is_zero() {
        swap_data.token_in = Some(token_in);
        swap_data.amount_in = Some(amount_fixed_for_fee);
        swap_data.amount_out_min = Some(amount_out_min);
    } else {
        swap_data.token_in = Some(token_in);
        swap_data.token_out = Some(token_out);
        swap_data.amount_in = Some(amount_fixed_for_fee);
        swap_data.amount_out_min = Some(amount_out_min);
    };

    // Construct SwapMultiCall
    let mut swap_multicall: SwapMultiCall = SwapMultiCall {
        token_in_destination: to,
        calls: vec![],
    };

    for best_pool in best_route {
        match best_pool {
            Pool::UniswapV2(uniswapv2_pool) => {
                let mut swap_bytes: Vec<u8> = vec![];

                if token_in.is_zero() {
                    swap_bytes.extend(&H160::from_str(WETH).unwrap().encode());
                    swap_bytes.extend(&U256::from(UNISWAP_V2_FEE).encode());
                } else if token_out.is_zero() {
                } else {
                    swap_bytes.push(0);
                }
                let swap_calldata = uniswapv2_pool.swap_calldata(amount_fixed_for_fee, U256::zero(), to, swap_bytes);

                let mut hex_calldata = hex::encode(swap_calldata.clone());
                hex_calldata.insert_str(0, "0x");

                // Push V2 swap call
                swap_multicall
                    .calls
                    .push((uniswapv2_pool.address, hex_calldata));

                if token_in.is_zero() {
                    swap_multicall.token_in_destination = to;

                    // Push Erc20 transfer call
                    let transfer_input = vec![Token::Address(receiver), Token::Uint(best_amount_out)];
                    let transfer_calldata = IERC20_ABI
                        .function("transfer")?
                        .encode_input(&transfer_input)
                        .unwrap();
                    let mut hex_calldata = hex::encode(transfer_calldata);
                    hex_calldata.insert_str(0, "0x");
                    swap_multicall.calls.push((token_out, hex_calldata));
                } else if token_out.is_zero() {
                    swap_multicall.token_in_destination = uniswapv2_pool.address;
                }
            }
            Pool::UniswapV3(_uniswapv3_pool) => {
                let mut swap_bytes: Vec<u8> = vec![];

                if token_in.is_zero() {
                    swap_bytes.extend(&H160::from_str(WETH).unwrap().encode());
                    swap_bytes.extend(&U256::from(UNISWAP_V2_FEE).encode());
                } else if token_out.is_zero() {
                } else {
                    swap_bytes.push(0);
                }

                let swap_calldata = _uniswapv3_pool.swap_calldata(to, true, I256::zero(), amount_fixed_for_fee, swap_bytes);

                let mut hex_calldata = hex::encode(swap_calldata.clone());
                hex_calldata.insert_str(0, "0x");

                // Push V2 swap call
                swap_multicall
                    .calls
                    .push((_uniswapv3_pool.address, hex_calldata));

                if token_in.is_zero() {
                    swap_multicall.token_in_destination = to;

                    // Push Erc20 transfer call
                    let transfer_input = vec![Token::Address(receiver), Token::Uint(best_amount_out)];
                    let transfer_calldata = IERC20_ABI
                        .function("transfer")?
                        .encode_input(&transfer_input)
                        .unwrap();
                    let mut hex_calldata = hex::encode(transfer_calldata);
                    hex_calldata.insert_str(0, "0x");
                    swap_multicall.calls.push((token_out, hex_calldata));
                } else if token_out.is_zero() {
                    swap_multicall.token_in_destination = _uniswapv3_pool.address;
                }
            }
        }
    }
    Ok((swap_data, swap_multicall))
}
