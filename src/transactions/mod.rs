use std::{str::FromStr, sync::Arc};

use cfmms::pool::Pool;
use ethabi::Token;
use ethers::{
    abi::AbiEncode,
    providers::Middleware,
    types::{H160, I256, U256},
};

use crate::{
    abi::IERC20_ABI,
    config::{self},
    constants::{FIFTH_WEB_MULTICALL, UNISWAP_V2_FEE, WETH},
    error::ExecutorError,
    routing::{find_best_route, find_markets_and_route},
};

pub(crate) mod types;

use types::{SwapData, SwapMultiCall};

//Construct a final swap transaction calldata
pub async fn swap_transaction_calldata<M: 'static + Middleware>(
    configuration: &config::Config,
    token_in: H160,
    token_out: H160,
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

    let markets =
        find_markets_and_route(token_in, token_out, configuration, middleware.clone()).await?;

    let (best_pool, best_amount_out) =
        find_best_route(markets, token_in, amount_fixed_for_fee, middleware.clone()).await?;

    println!("this is the best result from routing =========> {:?} \n {:?}", best_pool, best_amount_out);
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
    Ok((swap_data, swap_multicall))
}
