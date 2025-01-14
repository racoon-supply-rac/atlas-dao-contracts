use cosmwasm_std::{Deps, Coin, coin, WasmMsg, to_json_binary, Storage, Env, Uint128, coins, BankMsg, Addr, Empty, StdError, StdResult};
use cw721::Cw721ExecuteMsg;
use nois::{ProxyExecuteMsg, int_in_range};
use sg721::ExecuteMsg as Sg721ExecuteMsg;
use sg_std::{Response, CosmosMsg};
use utils::state::{AssetInfo, into_cosmos_msg};
use cw721_base::Extension;
use crate::{error::ContractError, state::{NOIS_AMOUNT, CONFIG, RaffleInfo, RandomnessParams, NOIS_RANDOMNESS, get_raffle_state, RAFFLE_TICKETS, ATLAS_DAO_STARGAZE_TREASURY, RAFFLE_INFO, RaffleState}};



pub fn get_nois_randomness(
    deps: Deps,
    raffle_id: u64,
) -> Result<Response, ContractError> {
    // let raffle_info = load_raffle(deps.storage, raffle_id)?;
    // let contract_info = CONFIG.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;
    let id = raffle_id.to_string();
    let nois_fee: Coin = coin(NOIS_AMOUNT, config.nois_proxy_denom);

    // TODO: if raffle already has randomness, error.


    let response = Response::new().add_message(WasmMsg::Execute {
        contract_addr: config.nois_proxy_addr.into_string(),
        // GetNextRandomness requests the randomness from the proxy
        // The job id is needed to know what randomness we are referring to upon reception in the callback.
        msg: to_json_binary(&ProxyExecuteMsg::GetNextRandomness {
            job_id: "raffle-".to_string() + id.as_str(), 
        })?,
        

        funds: vec![nois_fee], // Pay from the contract
    });
    Ok(response)
}   

/// Util to get the organizers and helpers messages to return when claiming a Raffle (returns the funds)
pub fn get_raffle_owner_finished_messages(
    storage: &dyn Storage,
    env: Env,
    raffle_info: RaffleInfo,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let contract_info = CONFIG.load(storage)?;

    // We start by splitting the fees between owner, treasury and radomness provider
    let total_paid = match raffle_info.raffle_ticket_price.clone() {
        AssetInfo::Coin(coin) => coin.amount,
        _ => return Err(ContractError::WrongFundsType {}),
    } * Uint128::from(raffle_info.number_of_tickets);
    let treasury_amount = total_paid * contract_info.raffle_fee;
    let owner_amount = total_paid  - treasury_amount;

    // Then we craft the messages needed for asset transfers
    match raffle_info.raffle_ticket_price {
        AssetInfo::Coin(coin) => {
            let mut messages: Vec<CosmosMsg> = vec![];
            // if rand_amount != Uint128::zero() {
            //     messages.push(
            //         BankMsg::Send { // TODO: Swap into $NOIS ?
            //             to_address: ATLAS_DAO_STARGAZE_TREASURY.to_string(),
            //             amount: coins(rand_amount.u128(), coin.denom.clone()),
            //         }
            //         .into(),
            //     );
            // };
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
                        to_address: ATLAS_DAO_STARGAZE_TREASURY.to_string(),
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

/// Picking the winner of the raffle
pub fn get_raffle_winner(
    deps: Deps,
    env: Env,
    raffle_id: u64,
    raffle_info: RaffleInfo,
) -> Result<Addr, ContractError> {
    let RandomnessParams {
        nois_randomness,
        requested: _,
    } = NOIS_RANDOMNESS.load(deps.storage)?;

    if nois_randomness.is_none() {
        return Err(ContractError::WrongStateForClaim {
            status: get_raffle_state(env, raffle_info),
        });
    }

    // TODO: get_nois_for_raffle(env, raffle_id)

    // We initiate the random number generator
    if raffle_info.randomness.is_none() {
        return Err(ContractError::WrongStateForClaim {
            status: get_raffle_state(env, raffle_info),
        });
    }
    // let mut rng: Prng = Prng::new(&raffle_info.randomness.unwrap().randomness);

    // We pick a winner id
    let winner_id = int_in_range(
        nois_randomness.expect("expect a value here"),
        0,
        raffle_info.number_of_tickets,
    );
    let winner = RAFFLE_TICKETS.load(deps.storage, (raffle_id, winner_id))?;

    Ok(winner)
}

/// Util to get the raffle creator messages to return when the Raffle is cancelled (returns the raffled asset)
pub fn get_raffle_owner_messages(env: Env, raffle_info: RaffleInfo) -> StdResult<Vec<CosmosMsg>> {
    let owner: Addr = raffle_info.owner.clone();
    _get_raffle_end_asset_messages(env, raffle_info, owner.to_string())
}

/// Util to get the assets back from a raffle
fn _get_raffle_end_asset_messages(
    _env: Env,
    raffle_info: RaffleInfo,
    receiver: String,
) -> StdResult<Vec<CosmosMsg>> {
    raffle_info
        .assets
        .iter()
        .map(|asset| match asset {
            AssetInfo::Cw721Coin(nft) => {
                let message = Cw721ExecuteMsg::TransferNft {
                    recipient: receiver.clone(),
                    token_id: nft.token_id.clone(),
                };
                into_cosmos_msg(message, nft.address.clone(),None,)
            }
            AssetInfo::Sg721Token(sg721_token) => {
                let message = Sg721ExecuteMsg::<Extension, Empty>::TransferNft {
                    recipient: receiver.clone(),
                    token_id: sg721_token.token_id.clone(),
                };
                into_cosmos_msg(message, sg721_token.address.clone(),None,)
            }
            _ => return Err(StdError::generic_err("unreachable")),
        })
        .collect()
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

/// Computes the ticket cost for multiple tickets bought together
pub fn ticket_cost(
    raffle_info: RaffleInfo,
    ticket_number: u32,
) -> Result<AssetInfo, ContractError> {
    Ok(match raffle_info.raffle_ticket_price {
        AssetInfo::Coin(x) => AssetInfo::Coin(Coin {
            denom: x.denom,
            amount: Uint128::from(ticket_number) * x.amount,
        }),
        // TODO: to set cost as Cw721Coin, we expect a possible
        // array of Cw721Coins as price cost.
        // AssetInfo::Sg721Token(x) => AssetInfo::Sg721Token(Sg721Token {
        //     address: x.address,
        //     amount: Uint128::from(ticket_number) * x.amount,
        //     token_id: todo!(),
        // }),
        _ => return Err(ContractError::WrongAssetType {}),
    })
}

/// Can only buy a ticket when the raffle has started and is not closed
pub fn can_buy_ticket(env: Env, raffle_info: RaffleInfo) -> Result<(), ContractError> {
    if get_raffle_state(env, raffle_info) == RaffleState::Started {
        Ok(())
    } else {
        return Err(ContractError::CantBuyTickets {});
    }
}

// RAFFLE WINNER 

/// Util to get the winner messages to return when claiming a Raffle (returns the raffled asset)
pub fn get_raffle_winner_messages(env: Env, raffle_info: RaffleInfo) -> StdResult<Vec<CosmosMsg>> {
    let winner: Addr = raffle_info.winner.clone().unwrap();
    _get_raffle_end_asset_messages(env, raffle_info, winner.to_string())
}