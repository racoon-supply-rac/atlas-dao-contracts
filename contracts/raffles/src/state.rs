use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, StdError, StdResult, Coin, Timestamp, Env, Storage, coin, Uint128};

use cw_storage_plus::{Item, Map};
use sg_std::NATIVE_DENOM;
use utils::state::AssetInfo;

//TODO: add to contract config
pub const ATLAS_DAO_STARGAZE_TREASURY: &str = "stars1jyg4j6t4kdptgsx6q55mu0f434zqcfppkx6ww9gs7p4x7clgfrjq29sgmc";
pub const NOIS_AMOUNT: u128 = 500000;
pub const MINIMUM_RAFFLE_DURATION: u64 = 1;
pub const MINIMUM_RAFFLE_TIMEOUT: u64 = 120; // The raffle timeout is a least 2 minutes
pub const DECIMAL_FRACTIONAL: u128 = 1_000_000_000_000_000_000u128; // 1*10**18
pub const MINIMUM_RAND_FEE: Decimal = Decimal::raw(DECIMAL_FRACTIONAL/10_000u128); // The randomness provider gets at least 1/10_000 of the total raffle price
pub const MINIMUM_CREATION_FEE_AMOUNT: u128 = 69;
pub const MINIMUM_CREATION_FEE_DENOM: &str = NATIVE_DENOM;


#[cw_serde]
pub struct Config {
    pub name: String,
    pub owner: Addr,
    pub fee_addr: Addr,
    pub last_raffle_id: Option<u64>,
    pub minimum_raffle_duration: u64, // The minimum interval in which users can buy raffle tickets
    pub minimum_raffle_timeout: u64, // The minimum interval during which users can provide entropy to the contract
    pub creation_fee_denom: String, // The static fee denom to create a new raffle.
    pub creation_fee_amount: Uint128, // The static fee amount to create a new raffle.
    pub raffle_fee: Decimal, // The percentage of the resulting ticket-tokens that will go to the treasury
    pub lock: bool,        // Wether the contract can accept new raffles
    pub nois_proxy_addr: Addr,
    pub nois_proxy_denom: String, // https://nois.network proxy address
    pub nois_proxy_amount: Uint128
}

impl Config{
    pub fn validate_fee(&self) -> Result<(), StdError>{
        // Check the fee distribution
        if self.raffle_fee >= Decimal::one(){
            return Err(StdError::generic_err(
                "The Total Fee rate should be lower than 1"
            ))
        }
        Ok(())
    }
}

#[cw_serde]
pub struct RandomnessParams {
    // The randomness beacon received from the proxy
    pub nois_randomness: Option<[u8; 32]>,
    // If the randomness has already been requested
    pub requested: bool,
}

#[cw_serde]
pub struct NoisProxy {
    // The price to pay the proxy for randomness
    pub price: Coin,
    // The address of the nois-proxy contract deployed onthe same chain as this contract
    pub address: Addr,
}

pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);
pub const RAFFLE_INFO: Map<u64, RaffleInfo> = Map::new("raffle_info");
pub const RAFFLE_TICKETS: Map<(u64, u32), Addr> = Map::new("raffle_tickets");
pub const USER_TICKETS: Map<(&Addr, u64), u32> = Map::new("user_tickets");
pub const NOIS_RANDOMNESS: Item<RandomnessParams> = Item::new("nois_randomness");


// RAFFLES

pub fn load_raffle(storage: &dyn Storage, raffle_id: u64) -> StdResult<RaffleInfo> {
    RAFFLE_INFO.load(storage, raffle_id)
}

#[cw_serde]
pub struct RaffleInfo {
    pub owner: Addr,
    pub assets: Vec<AssetInfo>,
    pub raffle_ticket_price: AssetInfo,
    pub number_of_tickets: u32,
    pub randomness: Option<RandomnessParams>,
    pub winner: Option<Addr>,
    pub is_cancelled: bool,
    pub raffle_options: RaffleOptions,
}


#[cw_serde]
pub enum RaffleState {
    Created,
    Started,
    Closed,
    Finished,
    Claimed,
    Cancelled,
}

impl std::fmt::Display for RaffleState {
    fn fmt(&self,f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RaffleState::Created => write!(f, "created"),
            RaffleState::Started => write!(f, "started"),
            RaffleState::Closed => write!(f, "closed"),
            RaffleState::Finished =>  write!(f, "finished"),
            RaffleState::Claimed =>  write!(f, "claimed"),
            RaffleState::Cancelled =>  write!(f, "cancelled"),

        }
    }
}

/// Queries the raffle state
/// This function depends on the block time to return the RaffleState.
/// As actions can only happen in certain time-periods, you have to be careful when testing off-chain
/// If the chains stops or the block time is not accurate we might get some errors (let's hope it never happens)
pub fn get_raffle_state(env: Env, raffle_info: RaffleInfo) -> RaffleState {
    if raffle_info.is_cancelled {
        RaffleState::Cancelled
    } else if env.block.time < raffle_info.raffle_options.raffle_start_timestamp {
        RaffleState::Created
    } else if env.block.time
        < raffle_info
            .raffle_options
            .raffle_start_timestamp
            .plus_seconds(raffle_info.raffle_options.raffle_duration)
    {
        RaffleState::Started
    } else if env.block.time
        < raffle_info
            .raffle_options
            .raffle_start_timestamp
            .plus_seconds(raffle_info.raffle_options.raffle_duration)
            .plus_seconds(raffle_info.raffle_options.raffle_timeout)
        || raffle_info.randomness.is_none()
    {
        RaffleState::Closed
    } else if raffle_info.winner.is_none() {
        RaffleState::Finished
    } else {
        RaffleState::Claimed
    }
}

#[cw_serde]
pub struct RaffleOptions {
    pub raffle_start_timestamp: Timestamp, // If not specified, starts immediately
    pub raffle_duration: u64,
    pub raffle_timeout: u64,
    pub comment: Option<String>,
    pub max_participant_number: Option<u32>,
    pub max_ticket_per_address: Option<u32>,
    pub raffle_preview: u32,
}

#[cw_serde]
pub struct RaffleOptionsMsg {
    pub raffle_start_timestamp: Option<Timestamp>, // If not specified, starts immediately
    pub raffle_duration: Option<u64>,
    pub raffle_timeout: Option<u64>,
    pub comment: Option<String>,
    pub max_participant_number: Option<u32>,
    pub max_ticket_per_address: Option<u32>,
    pub raffle_preview: Option<u32>,
}

impl RaffleOptions {
    pub fn new(
        env: Env,
        assets_len: usize,
        raffle_options: RaffleOptionsMsg,
        contract_info: Config,
    ) -> Self {
        Self {
            raffle_start_timestamp: raffle_options
                .raffle_start_timestamp
                .unwrap_or(env.block.time)
                .max(env.block.time),
            raffle_duration: raffle_options
                .raffle_duration
                .unwrap_or(contract_info.minimum_raffle_duration)
                .max(contract_info.minimum_raffle_duration),
            raffle_timeout: raffle_options
                .raffle_timeout
                .unwrap_or(contract_info.minimum_raffle_timeout)
                .max(contract_info.minimum_raffle_timeout),
            comment: raffle_options.comment,
            max_participant_number: raffle_options.max_participant_number,
            max_ticket_per_address: raffle_options.max_ticket_per_address,
            raffle_preview: raffle_options
                .raffle_preview
                .map(|preview| {
                    if preview >= assets_len.try_into().unwrap() {
                        0u32
                    } else {
                        preview
                    }
                })
                .unwrap_or(0u32),
        }
    }

    pub fn new_from(
        current_options: RaffleOptions,
        assets_len: usize,
        raffle_options: RaffleOptionsMsg,
        contract_info: Config,
    ) -> Self {
        Self {
            raffle_start_timestamp: raffle_options
                .raffle_start_timestamp
                .unwrap_or(current_options.raffle_start_timestamp)
                .max(current_options.raffle_start_timestamp),
            raffle_duration: raffle_options
                .raffle_duration
                .unwrap_or(current_options.raffle_duration)
                .max(contract_info.minimum_raffle_duration),
            raffle_timeout: raffle_options
                .raffle_timeout
                .unwrap_or(current_options.raffle_timeout)
                .max(contract_info.minimum_raffle_timeout),
            comment: raffle_options.comment.or(current_options.comment),
            max_participant_number: raffle_options
                .max_participant_number
                .or(current_options.max_participant_number),
            max_ticket_per_address: raffle_options
                .max_ticket_per_address
                .or(current_options.max_ticket_per_address),
            raffle_preview: raffle_options
                .raffle_preview
                .map(|preview| {
                    if preview >= assets_len.try_into().unwrap() {
                        0u32
                    } else {
                        preview
                    }
                })
                .unwrap_or(current_options.raffle_preview),
        }
    }
}








