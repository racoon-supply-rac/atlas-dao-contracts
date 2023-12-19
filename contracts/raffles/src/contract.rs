// use anyhow::{anyhow, bail, Result};
use cosmwasm_std::{entry_point, StdResult};
use cosmwasm_std::{
    to_json_binary, Binary, Deps, DepsMut, Env, Event, MessageInfo, Reply, Response, StdError,
    SubMsgResult, Decimal
};
#[cfg(not(feature = "library"))]
use std::convert::TryInto;
use std::num::ParseIntError;
use std::str::FromStr;

use cw2::set_contract_version;

use utils::state::OwnerStruct;

use raffles_export::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, RaffleResponse};
use raffles_export::state::{
    ContractInfo, Randomness, MINIMUM_RAFFLE_DURATION, MINIMUM_RAFFLE_TIMEOUT, MINIMUM_RAND_FEE,
};

use crate::error::ContractError;
use crate::execute::{
    execute_buy_tickets, execute_cancel_raffle, execute_claim, execute_create_raffle,
    execute_modify_raffle, execute_receive, execute_update_randomness,
};
use crate::query::{
    query_all_raffles, query_all_tickets, query_contract_info, query_ticket_number,
};
use crate::state::{get_raffle_state, is_owner, load_raffle, CONTRACT_INFO, RAFFLE_INFO};

const CONTRACT_NAME: &str = "illiquidlabs.io:raffles";
const CONTRACT_VERSION: &str = "0.1.0";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Verify the contract name
    msg.validate()?;
    // store token info
    let data = ContractInfo {
        name: msg.name,
        owner: OwnerStruct::new(deps
            .api
            .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?),
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
        rand_fee: msg
            .rand_fee
            .unwrap_or(MINIMUM_RAND_FEE)
            .max(MINIMUM_RAND_FEE),
        lock: false,
        drand_url: msg
            .drand_url
            .unwrap_or_else(|| "https://api.drand.sh/".to_string()),
        random_pubkey: Binary::from_base64(&msg.random_pubkey)?,
        verify_signature_contract: deps.api.addr_validate(&msg.verify_signature_contract)?,
    };

    // TODO: add fair-burn module?
    data.validate_fee()?;


    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default()
        .add_attribute("action", "init")
        .add_attribute("contract", "raffle")
        .add_attribute("owner", data.owner.owner))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateRaffle {
            owner,
            assets,
            raffle_ticket_price,
            raffle_options,
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
        ExecuteMsg::Receive (msg) => execute_receive(deps, env, info, msg),
        ExecuteMsg::ClaimNft { raffle_id } => execute_claim(deps, env, info, raffle_id),
        ExecuteMsg::UpdateRandomness {
            raffle_id,
            randomness,
        } => execute_update_randomness(deps, env, info, raffle_id, randomness),

        // Admin messages
        ExecuteMsg::ToggleLock { lock } => execute_toggle_lock(deps, env, info, lock),
        ExecuteMsg::ChangeParameter { parameter, value } => {
            execute_change_parameter(deps, env, info, parameter, value)
        }
        ExecuteMsg::ClaimOwnership { } => claim_ownership(deps, env, info),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // No state migrations performed, just returned a Response
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ContractInfo {} => to_json_binary(&query_contract_info(deps)?),
        QueryMsg::RaffleInfo { raffle_id } => {
            let raffle_info = load_raffle(deps.storage, raffle_id)?;
            to_json_binary(&RaffleResponse {
                raffle_id,
                raffle_state: get_raffle_state(env, raffle_info.clone()),
                raffle_info: Some(raffle_info),
            })
        }
        QueryMsg::AllRaffles {
            start_after,
            limit,
            filters,
        } => to_json_binary(&query_all_raffles(deps, env, start_after, limit, filters)?),
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
        )?),
        QueryMsg::TicketNumber { owner, raffle_id } => {
            to_json_binary(&query_ticket_number(
                deps,
                env,
                raffle_id,
                owner,
            )?)
        }
    }
}

/// Locking the contract (lock=true) means preventing the creation of new raffles
/// Tickets can still be bought and NFTs retrieved when a contract is locked
pub fn execute_toggle_lock(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lock: bool,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.lock = lock;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", "contract_lock")
        .add_attribute("value", lock.to_string()))
}

/// Change the different contract parameters
/// Admin only action
pub fn execute_change_parameter(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    parameter: String,
    value: String,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    match parameter.as_str() {
        "owner" => {
            let owner = deps.api.addr_validate(&value)?;
            contract_info.owner = contract_info.owner.propose_new_owner(owner);
        }
        "fee_addr" => {
            let addr = deps.api.addr_validate(&value)?;
            contract_info.fee_addr = addr;
        }
        "minimum_raffle_duration" => {
            let time = value.parse::<u64>()
            .map_err(|err: ParseIntError| StdError::generic_err(format!("minimum_raffle_duration Error: {:?}", err)))?;

            contract_info.minimum_raffle_duration = time.max(MINIMUM_RAFFLE_DURATION);
            
        }
        "minimum_raffle_timeout" => {
            let time = value.parse::<u64>()
            .map_err(|err: std::num::ParseIntError| StdError::generic_err(format!("minimum_raffle_timeout Error: {:?}", err)))?;

            
            contract_info.minimum_raffle_timeout = time.max(MINIMUM_RAFFLE_TIMEOUT);
        }
        "raffle_fee" => {
            let fee = Decimal::from_str(&value)?;
            contract_info.raffle_fee = fee;
        }
        "rand_fee" => {
            let fee = Decimal::from_str(&value)?;
            contract_info.rand_fee = fee.max(MINIMUM_RAND_FEE);
        }
        "drand_url" => {
            contract_info.drand_url = value.clone();
        }
        "verify_signature_contract" => {
            let addr = deps.api.addr_validate(&value)?;
            contract_info.verify_signature_contract = addr;
        }
        "random_pubkey" => {
            contract_info.random_pubkey = Binary::from_base64(&value)?;
        }
        _ => return Err(ContractError::ParameterNotFound {}),
    }

    contract_info.validate_fee()?;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", parameter)
        .add_attribute("value", value))
}

/// Claim ownership of the contract
pub fn claim_ownership(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {

    let mut contract_info = CONTRACT_INFO.load(deps.storage)?;

    contract_info.owner = contract_info.owner.validate_new_owner(info)?;

    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::default()
        .add_attribute("action", "claimed contract ownership")
        .add_attribute("new owner", contract_info.owner.owner))
}


/// Messages triggered after random validation.
/// We wrap the random validation in a message to make sure the transaction goes through.
/// This may require too much gas for query
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        0 => Ok(verify(deps, env, msg.result)?),
        _ => Err(ContractError::Unauthorized {}),
    }
}

/// This function is called after the randomness verifier has verified the current randomness
/// We used this architecture to make sure the verification passes (because a query may return early)
/// We verify the randomness provided matches the current state of the contract (good round, good raffle_id...)
/// We also save the new randomness in the contract
pub fn verify(deps: DepsMut, _env: Env, msg: SubMsgResult) -> Result<Response, ContractError> {
    match msg {
        SubMsgResult::Ok(subcall) => {
            let event: Event = subcall
                .events
                .into_iter()
                .find(|e| e.ty == "wasm")
                .ok_or_else(|| ContractError::NotFoundError("wasm results".to_string()))?;

            let round = event
                .attributes
                .clone()
                .into_iter()
                .find(|attr| attr.key == "round")
                .map_or(
                    Err(ContractError::NotFoundError("randomness round".to_string())),
                    |round| {
                        round
                            .value
                            .parse::<u64>()
                            .map_err(|_| ContractError::ParseError("randomness round".to_string()))
                    },
                )?;

            let randomness: String = event
                .attributes
                .clone()
                .into_iter()
                .find(|attr| attr.key == "randomness")
                .map(|rand| rand.value)
                .ok_or_else(|| ContractError::NotFoundError("randomness value".to_string()))?;

            let raffle_id: u64 = event
                .attributes
                .clone()
                .into_iter()
                .find(|attr| attr.key == "raffle_id")
                .map_or(
                    Err(ContractError::NotFoundError("raffle_id".to_string())),
                    |raffle_id| {
                        raffle_id
                            .value
                            .parse::<u64>()
                            .map_err(|_| ContractError::ParseError("raffle_id".to_string()))
                    },
                )?;

            let owner = deps.api.addr_validate(
                &event
                    .attributes
                    .into_iter()
                    .find(|attr| attr.key == "owner")
                    .map(|owner| owner.value)
                    .ok_or_else(|| ContractError::NotFoundError("randomness provider".to_string()))?,
            )?;

            let mut raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;
            raffle_info.randomness = Some(Randomness {
                randomness: Binary::from_base64(&randomness)?
                    .as_slice()
                    .try_into()
                    .map_err(|_| ContractError::ParseError("randomness".to_string()))?,
                randomness_round: round,
                randomness_owner: owner.clone(),
            });

            RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

            Ok(Response::new()
                .add_attribute("action", "update_randomness")
                .add_attribute("raffle_id", raffle_id.to_string())
                .add_attribute("sender", owner))
        }
        SubMsgResult::Err(_) => Err(ContractError::Std(StdError::generic_err("err"))),
    }
}
