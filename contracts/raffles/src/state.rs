use cosmwasm_std::{Coin, Empty, ensure};
use cw_storage_plus::{Item, Map};

use cosmwasm_std::{
    coins, Addr, BankMsg, CosmosMsg, Deps, Env, Response, Storage, SubMsg, Uint128,
};

use crate::error::ContractError;
use crate::rand::Prng;
use raffles_export::msg::{into_cosmos_msg, DrandRandomness, VerifierExecuteMsg};
use raffles_export::state::{AssetInfo, ContractInfo, RaffleInfo, RaffleState};
use cw721::Cw721ExecuteMsg;

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");
pub const RAFFLE_INFO: Map<u64, RaffleInfo> = Map::new("raffle_info");
pub const RAFFLE_TICKETS: Map<(u64, u32), Addr> = Map::new("raffle_tickets");
pub const USER_TICKETS: Map<(&Addr, u64), u32> = Map::new("user_tickets");

/// This function is largely inspired (and even directly copied) from https://github.com/LoTerra/terrand-contract-step1/
/// This function actually simply calls an external contract that checks the randomness origin
/// This architecture was chosen because the imported libraries needed to verify signatures are very heavy
/// and won't upload when combined with the current contract.
/// Separating into 2 contracts seems to help with that
/// For more info about randomness, visit : https://drand.love/
pub fn assert_randomness_origin_and_order(
    deps: Deps,
    owner: Addr,
    raffle_id: u64,
    randomness: DrandRandomness,
) -> Result<Response, ContractError> {
    let raffle_info = load_raffle(deps.storage, raffle_id)?;
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    if let Some(local_randomness) = raffle_info.randomness {
        if randomness.round <= local_randomness.randomness_round {
            return Err(ContractError::RandomnessNotAccepted {
                current_round: local_randomness.randomness_round
            });
        }
    }

    let msg = VerifierExecuteMsg::Verify {
        randomness,
        pubkey: contract_info.random_pubkey,
        raffle_id,
        owner: owner.to_string(),
    };
    let verify_message = into_cosmos_msg(msg, contract_info
        .verify_signature_contract.to_string())?;

    let msg = SubMsg::reply_on_success(verify_message, 0);
    Ok(Response::new().add_submessage(msg))
}

pub fn is_owner(storage: &dyn Storage, sender: Addr) -> Result<ContractInfo, ContractError> {
    let contract_info = CONTRACT_INFO.load(storage)?;
    if sender == contract_info.owner.owner {
        Ok(contract_info)
    } else {
        Err(ContractError::Unauthorized {})
    }
}

pub fn is_raffle_owner(
    storage: &dyn Storage,
    raffle_id: u64,
    sender: Addr,
) -> Result<RaffleInfo, ContractError> {
    let raffle_info = RAFFLE_INFO.load(storage, raffle_id)?;
    if sender == raffle_info.owner {
        Ok(raffle_info)
    } else {
        Err(ContractError::Unauthorized {})
    }
}

/// Picking the winner of the raffle
/// This function was inspired by https://github.com/scrtlabs/secret-raffle/
/// We know the odds are not exactly perfect with this architecture
/// --> that's not how you select a true random number from an interval, but n/4_294_967_295 will stay quite small anyway
/// (with n the number of bought tickets)
pub fn get_raffle_winner(
    deps: Deps,
    env: Env,
    raffle_id: u64,
    raffle_info: RaffleInfo,
) -> Result<Addr, ContractError> {
    // We initiate the random number generator
    if raffle_info.randomness.is_none() {
        return Err(ContractError::WrongStateForClaim {
            status: get_raffle_state(env, raffle_info)
        });
    }
    let mut rng: Prng = Prng::new(&raffle_info.randomness.unwrap().randomness);

    // We pick a winner id
    let winner_id = rng.random_between(0u32, raffle_info.number_of_tickets - 1);
    let winner = RAFFLE_TICKETS.load(deps.storage, (raffle_id, winner_id))?;

    Ok(winner)
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

pub fn load_raffle(storage: &dyn Storage, raffle_id: u64) -> Result<RaffleInfo, ContractError> {
    RAFFLE_INFO
        .load(storage, raffle_id)
        .map_err(|_| ContractError::NotFoundInRaffleInfo {})
}

/// Can only buy a ticket when the raffle has started and is not closed
pub fn can_buy_ticket(env: Env, raffle_info: RaffleInfo) -> Result<(), ContractError> {
    if get_raffle_state(env, raffle_info) == RaffleState::Started {
        Ok(())
    } else {
        return Err(ContractError::CantBuyTickets {})
    }
}

/// Computes the ticket cost for multiple tickets bought together
pub fn ticket_cost(raffle_info: RaffleInfo, ticket_number: u32) -> Result<AssetInfo, ContractError> {
    Ok(match raffle_info.raffle_ticket_price {
        AssetInfo::Coin(x) => AssetInfo::Coin(Coin {
            denom: x.denom,
            amount: Uint128::from(ticket_number) * x.amount,
        }),
        // AssetInfo::Cw20Coin(x) => AssetInfo::Cw20Coin(Cw20Coin {
        //     address: x.address,
        //     amount: Uint128::from(ticket_number) * x.amount,
        // }),
        _ => return Err(ContractError::WrongAssetType {}),
    })
}

/// Util to get the winner messages to return when claiming a Raffle (returns the raffled asset)
pub fn get_raffle_winner_messages(env: Env, raffle_info: RaffleInfo) -> Result<CosmosMsg, ContractError> {
    let winner: Addr = raffle_info.winner.clone().unwrap();
    _get_raffle_end_asset_messages(env, raffle_info, winner.to_string())
}

/// Util to get the raffle creator messages to return when the Raffle is cancelled (returns the raffled asset)
pub fn get_raffle_owner_messages(env: Env, raffle_info: RaffleInfo) -> Result<CosmosMsg, ContractError> {
    let owner: Addr = raffle_info.owner.clone();
    _get_raffle_end_asset_messages(env, raffle_info, owner.to_string())
}

/// Util to get the assets back from a raffle
fn _get_raffle_end_asset_messages(
    env: Env,
    raffle_info: RaffleInfo,
    receiver: String,
) -> Result<Vec<CosmosMsg>> {
    raffle_info
        .assets
        .iter()
        .map(|asset| match asset {
            AssetInfo::Cw721Coin(nft) => {
                let message = Cw721ExecuteMsg::TransferNft {
                    recipient: receiver.clone(),
                    token_id: nft.token_id.clone(),
                };
                into_cosmos_msg(message, nft.address.clone())
            }
            AssetInfo::Cw1155Coin(cw1155) => {
                let message = Cw1155ExecuteMsg::SendFrom {
                    from: env.contract.address.to_string(),
                    to: receiver.clone(),
                    token_id: cw1155.token_id.clone(),
                    value: cw1155.value,
                    msg: None,
                };
                into_cosmos_msg(message, cw1155.address.clone())
            }
            _ => bail!(ContractError::Unreachable {}),
        })
        .collect()
}

/// Util to get the organizers and helpers messages to return when claiming a Raffle (returns the funds)
pub fn get_raffle_owner_finished_messages(
    storage: &dyn Storage,
    _env: Env,
    raffle_info: RaffleInfo,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let contract_info = CONTRACT_INFO.load(storage)?;

    // We start by splitting the fees between owner, treasury and radomness provider
    let total_paid = match raffle_info.raffle_ticket_price.clone() {
        // AssetInfo::Cw20Coin(coin) => coin.amount,
        AssetInfo::Coin(coin) => coin.amount,
        _ => return Err(ContractError::WrongFundsType {}),
    } * Uint128::from(raffle_info.number_of_tickets);
    let rand_amount = total_paid * contract_info.rand_fee;
    let treasury_amount = total_paid * contract_info.raffle_fee;
    let owner_amount = total_paid - rand_amount - treasury_amount;

    // Then we craft the messages needed for asset transfers
    match raffle_info.raffle_ticket_price {
        // AssetInfo::Cw20Coin(coin) => {
        //     let mut messages: Vec<CosmosMsg> = vec![];
        //     if rand_amount != Uint128::zero() {
        //         messages.push(into_cosmos_msg(
        //             Cw20ExecuteMsg::Transfer {
        //                 recipient: raffle_info.randomness.unwrap().randomness_owner.to_string(),
        //                 amount: rand_amount,
        //             },
        //             coin.address.clone(),
        //         )?);
        //     };
        //     if treasury_amount != Uint128::zero() {
        //         messages.push(into_cosmos_msg(
        //             Cw20ExecuteMsg::Transfer {
        //                 recipient: contract_info.fee_addr.to_string(),
        //                 amount: treasury_amount,
        //             },
        //             coin.address.clone(),
        //         )?);
        //     };
        //     if owner_amount != Uint128::zero() {
        //         messages.push(into_cosmos_msg(
        //             Cw20ExecuteMsg::Transfer {
        //                 recipient: raffle_info.owner.to_string(),
        //                 amount: owner_amount,
        //             },
        //             coin.address,
        //         )?);
        //     };
        //     Ok(messages)
        // }
        AssetInfo::Coin(coin) => {
            let mut messages: Vec<CosmosMsg> = vec![];
            if rand_amount != Uint128::zero() {
                messages.push(
                    BankMsg::Send {
                        to_address: raffle_info.randomness.unwrap().randomness_owner.to_string(),
                        amount: coins(rand_amount.u128(), coin.denom.clone()),
                    }
                    .into(),
                );
            };
            if treasury_amount != Uint128::zero() {
                messages.push(
                    BankMsg::Send {
                        to_address: contract_info.fee_addr.to_string(),
                        amount: coins(treasury_amount.u128(), coin.denom.clone()),
                    }
                    .into(),
                );
            };
            if owner_amount != Uint128::zero() {
                messages.push(
                    BankMsg::Send {
                        to_address: raffle_info.owner.to_string(),
                        amount: coins(owner_amount.u128(), coin.denom),
                    }
                    .into(),
                );
            };

            Ok(messages)
        }
        _ => Err(ContractError::WrongFundsType {}),
    }
}
