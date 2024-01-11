use cosmwasm_std::{Decimal, Addr, Uint128};
use raffles::state::{Config as RaffleParams, MINIMUM_RAFFLE_TIMEOUT, MINIMUM_RAFFLE_DURATION};
use sg_std::NATIVE_DENOM;

const RAFFLE_FEE: u64 = 50; // 50%
const RAND_FEE: u64 = 5;

const NOIS_PROXY_ADDR: &str = "nois";
const FEE_ADDR: &str = "fee";
const OWNER_ADDR: &str = "fee";
const NAME: &str = "raffle param name";
const NOIS_AMOUNT: u128 = 50;

pub fn mock_params() -> RaffleParams {
    RaffleParams {
         name: NAME.to_string(),
         owner: Addr::unchecked(OWNER_ADDR),
         fee_addr: Addr::unchecked(FEE_ADDR),
         last_raffle_id: Some(0),
         minimum_raffle_duration: MINIMUM_RAFFLE_DURATION, 
         minimum_raffle_timeout: MINIMUM_RAFFLE_TIMEOUT, 
         raffle_fee: Decimal::percent(RAFFLE_FEE), 
         lock: false,        
         nois_proxy_addr: Addr::unchecked(NOIS_PROXY_ADDR),
         nois_proxy_denom: NATIVE_DENOM.to_owned(),
        creation_fee_denom: NATIVE_DENOM.to_owned(),
        creation_fee_amount: Uint128::new(NOIS_AMOUNT),
        nois_proxy_amount: NOIS_AMOUNT.into(),
    }
}