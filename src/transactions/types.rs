use ethers::types::{H160, U256};

#[derive(Debug)]
pub struct SwapData {
    pub token_in: Option<H160>,
    pub token_out: Option<H160>,
    pub amount_in: Option<U256>,
    pub amount_out_min: Option<U256>,
    pub protocol_fee: Option<U256>,
    pub bribe: U256,
    pub affiliate: U256,
    pub referrer: U256,
}
#[derive(Debug)]
pub struct SwapMultiCall {
    pub token_in_destination: H160,
    pub calls: Vec<(H160, String)>,
}
