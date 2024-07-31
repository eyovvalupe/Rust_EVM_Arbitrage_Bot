use std::{str::FromStr, sync::{Arc, Mutex}, collections::HashMap};

use cfmms::pool::Pool;
use ethabi::Token;
use ethers::{
    abi::AbiEncode, providers::Middleware, types::{H160, I256, U256}
};
// use eyre::Ok;
use futures::{future::{BoxFuture, FutureExt}, executor::block_on};
use lazy_static::lazy_static;
use crate::{
    abi::IERC20_ABI,
    config::{self},
    constants::{FIFTH_WEB_MULTICALL, UNISWAP_V2_FEE, WETH, USDC, USDT},
    error::ExecutorError,
    routing::{find_best_a_to_b_route, find_a_to_b_markets_and_route, find_a_to_x_to_b_markets_and_route, find_best_a_to_x_to_b_route, find_all_markets},
};

pub(crate) mod types;

use types::{SwapData, SwapMultiCall};

lazy_static! {
    static ref GLOBAL_VEC: Arc<Mutex<Vec<Vec<H160>>>> = Arc::new(Mutex::new(Vec::new()));
}

// Async function example
async fn async_add_to_global_vec(vec: Vec<H160>) {
    let mut global_vec = GLOBAL_VEC.lock().unwrap();
    global_vec.push(vec);
}

fn compare_arc_option(
    arc_opt: Arc<Option<H160>>,
    opt: Option<H160>
) -> bool {
    let arc_value = &*arc_opt;
    arc_value == &opt
}

pub fn find_route(
    token_in: Arc<Option<H160>>,
    middle_tokens: Arc<HashMap<String, H160>>,
    middle_tokens_names: Arc<Vec<&'static str>>,
    deep: usize,
    temp_res: Arc<Mutex<Vec<H160>>>
) -> BoxFuture<'static, ()> {
    async move {
        let token_b = middle_tokens.get("TOKEN_B").cloned();
        let usdc = middle_tokens.get("USDC").cloned();
        let usdt = middle_tokens.get("USDT").cloned();
        let token_in_cloned = token_in.clone();

        let is_leaf = compare_arc_option(token_in_cloned, token_b);
        if let Some(token) = *token_in {
            temp_res.lock().unwrap().push(token);
        }
        if is_leaf && temp_res.lock().unwrap().to_vec().len() != 1 {
            let future = async_add_to_global_vec(temp_res.lock().unwrap().to_vec());
            block_on(future);
            return ;
        }
        for temp in &*middle_tokens_names.clone() {
            if deep == 3 {
                break;
            }
            let next_token = middle_tokens.get(*temp).cloned();
            let is_in_usdc = compare_arc_option(token_in.clone(), usdc);
            let is_out_usdc = next_token == usdc;
            let is_in_usdt = compare_arc_option(token_in.clone(), usdt);
            let is_out_usdt = next_token == usdt;
            if (is_in_usdc && is_out_usdt) || (is_in_usdt && is_out_usdc) || (*token_in == next_token) {
                continue;
            }
            find_route(
                Arc::new(next_token),
                middle_tokens.clone(),
                middle_tokens_names.clone(),
                deep+1,
                temp_res.clone()
            ).await;
            temp_res.lock().unwrap().pop();

            if deep == 2 && next_token == token_b {
                break;
            }
        }
    }.boxed()
}

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

    let middle_tokens_names = vec!["TOKEN_B", "WETH", "USDC", "USDT"];
    let mut middle_tokens = HashMap::new();

    middle_tokens.insert(String::from("TOKEN_B"), token_out);
    middle_tokens.insert(String::from("WETH"), H160::from_str(WETH).unwrap());
    middle_tokens.insert(String::from("USDC"), H160::from_str(USDC).unwrap());
    middle_tokens.insert(String::from("USDT"), H160::from_str(USDT).unwrap());

    let middle_tokens = Arc::new(middle_tokens);
    let middle_tokens_names = Arc::new(middle_tokens_names);
    let token_route_in = Arc::new(Some(token_in));
    let temp_res = Arc::new(Mutex::new(vec![]));

    find_route( token_route_in, Arc::clone(&middle_tokens), Arc::clone(&middle_tokens_names), 0, temp_res).await;
    let routes = GLOBAL_VEC.lock().unwrap();
    println!("==============================================this is the all routes from tree=====================================\n{:?}", routes);
    
    if token_in.is_zero() {
        amount_fixed_for_fee -= amount_fixed_for_fee / 100;
        protocol_fee = amount_in - amount_fixed_for_fee;
    }
    
    let mut tree_pool: Vec<Vec<Pool>> = Vec::new();
    let mut tree_amount: Vec<Vec<U256>> = Vec::new();
    tree_pool.resize(routes.len(), Vec::new());
    tree_amount.resize(routes.len(), Vec::new());
    let mut tree_best_route: Vec<Pool> = Vec::new();
    let mut tree_best_amount_out = U256::zero();

    // for i in 0..routes.len() {
    //     for j in 0..routes[i].len() {

    //         if j + 1 == routes[i].len() {
    //             break;
    //         }

    //         let mut temp_amount_in: U256 = U256::zero();

    //         if j == 0 {
    //             temp_amount_in = amount_fixed_for_fee;
    //         } else {
    //             temp_amount_in = tree_amount[i][j-1];
    //         }

    //         let markets =
    //             find_a_to_b_markets_and_route(routes[i][j], routes[i][j+1], configuration, middleware.clone()).await?;
    //         let (pool, amount_out) =
    //             find_best_a_to_b_route(markets, routes[i][j], temp_amount_in, middleware.clone()).await?;
    //         // println!("this is the token pair ===============> {:?}, {:?}\n", routes[i][j], routes[i][j+1]);
    //         // println!("this is the markets from finding markets ===============> {:?}\n", pool);

    //         tree_pool[i].push(pool);
    //         tree_amount[i].push(amount_out);
    //     }
    //     if tree_best_amount_out != U256::zero() && *tree_amount[i].last().unwrap() > tree_best_amount_out * 2 {
    //         continue;
    //     }
    //     if tree_best_amount_out < *tree_amount[i].last().unwrap()  {
    //         tree_best_amount_out = *tree_amount[i].last().unwrap();
    //         tree_best_route = tree_pool[i].clone();
    //     }
    //     // println!("============== this is the each route and pool of tree ============== \n{:?}\n{:?}\n{:?}\n", tree_amount[i], routes[i], tree_pool[i]);
    // }

    // println!("==============================================this is the best route and amount from tree=====================================\n{:?}\n{:?}\n", tree_best_amount_out, tree_best_route);

    // let all_markets = find_all_markets(token_in, token_out, configuration, middleware.clone()).await?;

    // let multi_markets = 
    //     find_a_to_x_to_b_markets_and_route(token_in, token_out, token_x, configuration, middleware.clone()).await?;
    
    // let (amounts_in, axb_amounts_out, axb_route) = 
    //     find_best_a_to_x_to_b_route(token_in, token_out, token_x, amount_in, &multi_markets, middleware.clone()).await?;
    
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

    best_amount_out = tree_best_amount_out;
    best_route = tree_best_route;

    ab_route.push(ab_pool);
    // if axb_amounts_out.last().unwrap() > &ab_amount_out {
    //     best_amount_out = *axb_amounts_out.last().unwrap();
    //     best_route = axb_route;
    // }
    // else {
        best_amount_out = ab_amount_out;
        best_route = ab_route;
    // }

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

    // println!("========================= this is the best amount and route from A-B and A-X-B ======================= \n{:?}, {:?}\n", best_amount_out, best_route);

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
