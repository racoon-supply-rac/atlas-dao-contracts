use cosmwasm_std::{
    ensure_eq, entry_point, to_json_binary, Decimal, Deps, DepsMut, Empty, Env, MessageInfo,
    QueryResponse, StdResult, Uint128,
};
use sg_std::StargazeMsgWrapper;

use crate::error::ContractError;
use crate::execute::{
    execute_buy_tickets, execute_cancel_raffle, execute_claim, execute_create_raffle,
    execute_modify_raffle, execute_receive, execute_receive_nois, execute_update_randomness,
};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, RaffleResponse};
use crate::query::{query_all_raffles, query_all_tickets, query_config, query_ticket_number};
use crate::state::{
    get_raffle_state, load_raffle, Config, RandomnessParams, CONFIG, MINIMUM_CREATION_FEE_AMOUNT,
    MINIMUM_RAFFLE_DURATION, MINIMUM_RAFFLE_TIMEOUT, NOIS_RANDOMNESS, MINIMUM_CREATION_FEE_DENOM,
};
use cw2::set_contract_version;

pub type Response = cosmwasm_std::Response<StargazeMsgWrapper>;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let nois_proxy_addr = deps
        .api
        .addr_validate(&msg.nois_proxy_addr)
        .map_err(|_| ContractError::InvalidProxyAddress)?;
    NOIS_RANDOMNESS.save(
        deps.storage,
        &RandomnessParams {
            nois_randomness: None,
            requested: false,
        },
    )?;

    let creation_fee_amount = match msg.creation_fee_amount {
        Some(int) => int,
        None => MINIMUM_CREATION_FEE_AMOUNT.into(),
    };

    let creation_fee_denom = match msg.creation_fee_denom{
        Some(cfd) => cfd,
        None => MINIMUM_CREATION_FEE_DENOM.to_string(),
    };

    let config = Config {
        name: msg.name,
        owner: deps
            .api
            .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?,
        fee_addr: deps
            .api
            .addr_validate(&msg.fee_addr.unwrap_or_else(|| info.sender.to_string()))?,
        last_raffle_id: None,
        minimum_raffle_duration: msg
            .minimum_raffle_duration
            .unwrap_or(MINIMUM_RAFFLE_DURATION)
            .max(MINIMUM_RAFFLE_DURATION),
        minimum_raffle_timeout: msg
            .minimum_raffle_timeout
            .unwrap_or(MINIMUM_RAFFLE_TIMEOUT)
            .max(MINIMUM_RAFFLE_TIMEOUT),
        raffle_fee: msg.raffle_fee.unwrap_or(Decimal::zero()),
        creation_fee_denom,
        creation_fee_amount,
        // rand_fee: msg
        //     .rand_fee
        //     .unwrap_or(MINIMUM_RAND_FEE)
        //     .max(MINIMUM_RAND_FEE),
        lock: false,
        nois_proxy_addr,
        nois_proxy_denom: msg.nois_proxy_denom,
        nois_proxy_amount: msg.nois_proxy_amount,
    };

    // TODO: add fair-burn module?
    config.validate_fee()?;

    CONFIG.save(deps.storage, &config)?;
    set_contract_version(
        deps.storage,
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    )?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), ::cosmwasm_std::entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: Empty) -> StdResult<Response> {
    set_contract_version(
        deps.storage,
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    )?;
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            name,
            owner,
            fee_addr,
            minimum_raffle_duration,
            minimum_raffle_timeout,
            creation_fee_denom,
            creation_fee_amount,
            raffle_fee,
            nois_proxy_addr,
            nois_proxy_denom,
            nois_proxy_amount,
        } => execute_update_config(
            deps,
            env,
            info,
            name,
            owner,
            fee_addr,
            minimum_raffle_duration,
            minimum_raffle_timeout,
            creation_fee_denom,
            creation_fee_amount,
            raffle_fee,
            nois_proxy_addr,
            nois_proxy_denom,
            nois_proxy_amount,
        ),
        ExecuteMsg::CreateRaffle {
            owner,
            assets,
            raffle_options,
            raffle_ticket_price,
        } => execute_create_raffle(
            deps,
            env,
            info,
            owner,
            assets,
            raffle_ticket_price,
            raffle_options,
        ),
        ExecuteMsg::CancelRaffle { raffle_id } => execute_cancel_raffle(deps, env, info, raffle_id),
        ExecuteMsg::ModifyRaffle {
            raffle_id,
            raffle_ticket_price,
            raffle_options,
        } => execute_modify_raffle(
            deps,
            env,
            info,
            raffle_id,
            raffle_ticket_price,
            raffle_options,
        ),
        ExecuteMsg::BuyTicket {
            raffle_id,
            ticket_number,
            sent_assets,
        } => execute_buy_tickets(deps, env, info, raffle_id, ticket_number, sent_assets),
        ExecuteMsg::Receive(msg) => execute_receive(deps, env, info, msg),
        ExecuteMsg::ClaimNft { raffle_id } => execute_claim(deps, env, info, raffle_id),
        ExecuteMsg::UpdateRandomness { raffle_id } => {
            execute_update_randomness(deps, env, info, raffle_id)
        }
        ExecuteMsg::NoisReceive { callback } => execute_receive_nois(deps, env, info, callback),
        // Admin messages
        ExecuteMsg::ToggleLock { lock } => execute_toggle_lock(deps, env, info, lock),
    }
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    let response = match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?)?,
        QueryMsg::RaffleInfo { raffle_id } => {
            let raffle_info = load_raffle(deps.storage, raffle_id)?;
            to_json_binary(&RaffleResponse {
                raffle_id,
                raffle_state: get_raffle_state(env, raffle_info.clone()),
                raffle_info: Some(raffle_info),
            })?
        }
        QueryMsg::AllRaffles {
            start_after,
            limit,
            filters,
        } => to_json_binary(&query_all_raffles(deps, env, start_after, limit, filters)?)?,
        QueryMsg::AllTickets {
            raffle_id,
            start_after,
            limit,
        } => to_json_binary(&query_all_tickets(
            deps,
            env,
            raffle_id,
            start_after,
            limit,
        )?)?,
        QueryMsg::TicketNumber { owner, raffle_id } => {
            to_json_binary(&query_ticket_number(deps, env, raffle_id, owner)?)?
        }
    };
    Ok(response)
}

fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _name: Option<String>,
    owner: Option<String>,
    fee_addr: Option<String>,
    minimum_raffle_duration: Option<u64>,
    minimum_raffle_timeout: Option<u64>,
    creation_fee_denom: Option<String>,
    creation_fee_amount: Option<Uint128>,
    raffle_fee: Option<Decimal>,
    nois_proxy_addr: Option<String>,
    nois_proxy_denom: Option<String>,
    nois_proxy_amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    //TODO: let mut config
    let config = CONFIG.load(deps.storage)?;
    // ensure msg sender is admin
    ensure_eq!(info.sender, config.owner, ContractError::Unauthorized);
    // TODO: check if new value is_valid_name
    let name = config.name;
    let owner = match owner {
        Some(ow) => deps.api.addr_validate(&ow)?,
        None => config.owner,
    };
    let fee_addr = match fee_addr {
        Some(fea) => deps.api.addr_validate(&fea)?,
        None => config.fee_addr,
    };
    let minimum_raffle_duration = match minimum_raffle_duration {
        Some(mrd) => mrd.max(MINIMUM_RAFFLE_DURATION),
        None => config.minimum_raffle_duration,
    };
    let minimum_raffle_timeout = match minimum_raffle_timeout {
        Some(mrt) => mrt.max(MINIMUM_RAFFLE_TIMEOUT),
        None => config.minimum_raffle_timeout,
    };
    let raffle_fee = match raffle_fee {
        Some(rf) => rf,
        None => config.raffle_fee,
    };
    // let rand_fee = match rand_fee {
    //     Some(raf) => raf,
    //     None => config.rand_fee,
    // };
    let nois_proxy_addr = match nois_proxy_addr {
        Some(prx) => deps.api.addr_validate(&prx)?,
        None => config.nois_proxy_addr,
    };
    let nois_proxy_denom = match nois_proxy_denom {
        Some(npr) => npr,
        None => config.nois_proxy_denom,
    };
    let nois_proxy_amount = match nois_proxy_amount {
        Some(npa) => npa,
        None => config.nois_proxy_amount,
    };
    let creation_fee_denom = match creation_fee_denom {
        Some(crf) => crf,
        None => config.creation_fee_denom,
    };
    let creation_fee_amount = match creation_fee_amount {
        Some(crf) => crf,
        None => config.creation_fee_amount,
    };
    // we have a seperate function to lock a raffle, so we skip here
    let lock = config.lock;
    // we do not want to be able to manually update the last raffle id.
    let last_raffle_id = config.last_raffle_id;

    CONFIG.save(
        deps.storage,
        &Config {
            name,
            owner,
            fee_addr,
            last_raffle_id,
            minimum_raffle_duration,
            minimum_raffle_timeout,
            creation_fee_amount,
            creation_fee_denom,
            raffle_fee,
            // rand_fee,
            lock,
            nois_proxy_addr,
            nois_proxy_denom,
            nois_proxy_amount,
        },
    )?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Locking the contract (lock=true) means preventing the creation of new raffles
/// Tickets can still be bought and NFTs retrieved when a contract is locked
pub fn execute_toggle_lock(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lock: bool,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    // check the calling address is the authorised multisig
    ensure_eq!(info.sender, config.owner, ContractError::Unauthorized);

    config.lock = lock;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", "contract_lock")
        .add_attribute("value", lock.to_string()))
}
