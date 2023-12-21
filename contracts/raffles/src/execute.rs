use crate::query::is_nft_owner;
use crate::state::{
    assert_randomness_origin_and_order, can_buy_ticket, get_raffle_owner_finished_messages,
    get_raffle_owner_messages, get_raffle_state, get_raffle_winner, get_raffle_winner_messages,
    is_raffle_owner, ticket_cost, CONTRACT_INFO, RAFFLE_INFO, RAFFLE_TICKETS, USER_TICKETS,
};
// use anyhow::{anyhow, bail, Result};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    from_json, Addr, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,StdError
};

use crate::error::ContractError;
use raffles_export::state::{
    AssetInfo, RaffleInfo, RaffleOptions, RaffleOptionsMsg, RaffleState, Cw721Coin,
};

// use cw1155::Cw1155ExecuteMsg;
// use cw20::{Cw20ExecuteMsg};
use cw721::{Cw721ExecuteMsg, Cw721ReceiveMsg};
use raffles_export::msg::{into_cosmos_msg, DrandRandomness, ExecuteMsg};

/// Create a new raffle by depositing assets.
/// The raffle has many options, to make it most accessible.
/// Args :
///
/// `owner`: The address that will receive the funds when the raffle is ended. Default value : create raffle transaction sender
///
/// `asset` : The asset set up for auction. It can be a CW721 standard asset or a CW1155 standard asset.
/// This asset will be deposited with this function. Don't forget to pre-approve the contract for this asset to be able to create a raffle
/// ReceiveNFT or Receive_CW1155 is used for people that hate approvals
///
/// `raffle_start_timestamp` : Block Timestamp from which the users can buy tickets Default : current block time
///
/// `raffle_duration` : time in seconds from the raffle_start_timestamp during which users can buy tickets. Default : contract.minimum_raffle_duration
///
/// raffle_timeout : time in seconds from the end of the raffle duration during which users can add randomness. Default : contract.minimum_raffle_timeout
///
/// `comment`: A simple comment to add to the raffle (because we're not machines) : Default : ""
///
/// `raffle_ticket_price`: The needed tokens (native or CW20) needed to buy a raffle ticket
/// If you want to have free tickets, specify a 0 amount on a native token (any denom)
///
/// `max_participant_number`: maximum number of participants to the raffle. Default : contract_info.max_participant_number
#[allow(clippy::too_many_arguments)]
pub fn execute_create_raffle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    all_assets: Vec<AssetInfo>,
    raffle_ticket_price: AssetInfo,
    raffle_options: RaffleOptionsMsg,
) -> Result<Response, ContractError> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    if contract_info.lock {
        return Err(ContractError::ContractIsLocked {});
    }

    // First we validate at least one asset was provided to the raffle (or else this is useless, we want the raffles to include NFTs)
    if all_assets.is_empty(){
        return Err(ContractError::NoAssets {  })
    }


    // Then we physcially transfer all the assets
    let transfer_messages: Vec<CosmosMsg> = all_assets
        .iter()
        .map(|asset| match &asset {
            AssetInfo::Cw721Coin(token) => {

                // (Audit results)
                // Before transferring the NFT, we make sure the current NFT owner is indeed the borrower of funds
                // Otherwise, this would cause anyone to be able to create loans in the name of the owner if a bad approval was done
                is_nft_owner(deps.as_ref(), info.sender.clone(), token.address.to_string(), token.token_id.to_string())?;

                let message = Cw721ExecuteMsg::TransferNft {
                    recipient: env.contract.address.clone().into(),
                    token_id: token.token_id.clone(),
                };

                into_cosmos_msg(message, token.address.clone())
            }
            // TODO: AssetInfo::Sg721Base()

            // AssetInfo::Cw1155Coin(token) => {
            //     let message = Cw1155ExecuteMsg::SendFrom {
            //         from: info.sender.to_string(),
            //         to: env.contract.address.clone().into(),
            //         token_id: token.token_id.clone(),
            //         value: token.value,
            //         msg: None,
            //     };

            //     into_cosmos_msg(message, token.address.clone())
            // }
            _ => Err(StdError::generic_err("msg")),

        })
        .collect::<Result<Vec<CosmosMsg>, StdError>>()?;
    // Then we create the internal raffle structure
    let owner = owner.map(|x| deps.api.addr_validate(&x)).transpose()?;
    let raffle_id = _create_raffle(
        deps,
        env,
        owner.clone().unwrap_or_else(|| info.sender.clone()),
        all_assets,
        raffle_ticket_price,
        raffle_options,
    )?;

    Ok(Response::new()
        .add_messages(transfer_messages)
        .add_attribute("action", "create_raffle")
        .add_attribute("raffle_id", raffle_id.to_string())
        .add_attribute("owner", owner.unwrap_or_else(|| info.sender.clone())))
}

/// Create a new raffle and assign it a unique id
/// Internal function that doesn't check anything and creates a raffle.
/// The arguments are described on the create_raffle function above.
#[allow(clippy::too_many_arguments)]
pub fn _create_raffle(
    deps: DepsMut,
    env: Env,
    owner: Addr,
    all_assets: Vec<AssetInfo>,
    raffle_ticket_price: AssetInfo,
    raffle_options: RaffleOptionsMsg,
) -> Result<u64, ContractError> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    // We start by creating a new trade_id (simply incremented from the last id)
    let raffle_id: u64 = CONTRACT_INFO
        .update(deps.storage, |mut c| -> StdResult<_> {
            c.last_raffle_id = c.last_raffle_id.map_or(Some(0), |id| Some(id + 1));
            Ok(c)
        })?
        .last_raffle_id
        .unwrap(); // This is safe because of the function architecture just there

    RAFFLE_INFO.update(deps.storage, raffle_id, |trade| match trade {
        // If the trade id already exists, the contract is faulty
        // Or an external error happened, or whatever...
        // In that case, we emit an error
        // The priority is : We do not want to overwrite existing data
        Some(_) => Err(ContractError::ExistsInRaffleInfo {}),
        None => Ok(RaffleInfo {
            owner,
            assets: all_assets.clone(),
            raffle_ticket_price: raffle_ticket_price.clone(), // No checks for the assetInfo type, the worst thing that can happen is an error when trying to buy a raffle ticket
            number_of_tickets: 0u32,
            randomness: None,
            winner: None,
            is_cancelled: false,
            raffle_options: RaffleOptions::new(
                env,
                all_assets.len(),
                raffle_options,
                contract_info,
            ),
        }),
    })?;
    Ok(raffle_id)
}

/// Cancels a raffle
/// This function is only accessible if no raffle ticket was bought on the raffle
pub fn execute_cancel_raffle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    raffle_id: u64,
) -> Result<Response, ContractError> {
    let mut raffle_info = is_raffle_owner(deps.storage, raffle_id, info.sender)?;

    // The raffle can only be cancelled if it wasn't previously cancelled and it isn't finished
    let raffle_state = get_raffle_state(env.clone(), raffle_info.clone());

    if raffle_state != RaffleState::Created && 
        raffle_state != RaffleState::Started && 
        raffle_state != RaffleState::Closed && 
        raffle_state != RaffleState::Finished{
        return Err(ContractError::WrongStateForCancel { status: raffle_state })
    }

    // We then verify there are not tickets bought
    if raffle_info.number_of_tickets != 0 {
        return Err(ContractError::RaffleAlreadyStarted {});
    }

    // Then notify the raffle is ended
    raffle_info.is_cancelled = true;
    RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

    // Then we transfer the assets back to the owner
    let transfer_messages = get_raffle_owner_messages(env, raffle_info)?;
    Ok(Response::new()
        .add_messages(transfer_messages)
        .add_attribute("action", "cancel_raffle")
        .add_attribute("raffle_id", raffle_id.to_string()))
}

/// Modify the raffle characteristics
/// A parameter is only modified if it is specified in the called message
/// If None is provided, nothing changes for the parameter
/// This function is only accessible if no raffle ticket was bought on the raffle
pub fn execute_modify_raffle(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    raffle_id: u64,
    raffle_ticket_price: Option<AssetInfo>,
    raffle_options: RaffleOptionsMsg,
) -> Result<Response, ContractError> {
    let mut raffle_info = is_raffle_owner(deps.storage, raffle_id, info.sender)?;
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    // We then verify there are not tickets bought
    if raffle_info.number_of_tickets != 0 {
        return Err(ContractError::RaffleAlreadyStarted {});
    }

    // Then modify the raffle characteristics
    raffle_info.raffle_options = RaffleOptions::new_from(
        raffle_info.raffle_options,
        raffle_info.assets.len(),
        raffle_options,
        contract_info,
    );
    // Then modify the ticket price
    if let Some(raffle_ticket_price) = raffle_ticket_price {
        raffle_info.raffle_ticket_price = raffle_ticket_price;
    }
    RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_raffle")
        .add_attribute("raffle_id", raffle_id.to_string()))
}

/// Buy a ticket for a specific raffle.
///
/// `raffle_id`: The id of the raffle you want to buy a ticket to/
///
/// `assets` : the assets you want to deposit against a raffle ticket.
/// These assets can either be a native coin or a CW20 token
/// These must correspond to the raffle_info.raffle_ticket_price exactly
/// This function needs the sender to approve token transfer (for CW20 tokens) priori to the transaction
/// The next function provides a receiver message implementation if you prefer
pub fn execute_buy_tickets(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    raffle_id: u64,
    ticket_number: u32,
    assets: AssetInfo,
) -> Result<Response, ContractError> {
    // First we physcially transfer the AssetInfo
    let transfer_messages = match &assets {
        AssetInfo::Cw721Coin(token) => {
            let message = Cw721ExecuteMsg::TransferNft {
                recipient: env.contract.address.clone().into(),
                token_id: token.token_id.clone(),
            };
            vec![into_cosmos_msg(message, token.address.clone())?]
        }

        // AssetInfo::Cw20Coin(token) => {
        //     let message = Cw20ExecuteMsg::Transfer {
        //         recipient: env.contract.address.clone().into(),
        //         amount: token.amount,
        //     };

        //     vec![into_cosmos_msg(message, token.address.clone())?]
        // }
        // or verify the sent coins match the message coins
        AssetInfo::Coin(coin) => {
            if coin.amount != Uint128::zero() && (info.funds.len() != 1 || info.funds[0].denom != coin.denom || info.funds[0].amount != coin.amount){
               return Err(ContractError::AssetMismatch {});
            }
            vec![]
        }
        // _ => return Err(ContractError::WrongAssetType {}),
    };

    // Then we verify the funds sent match the raffle conditions and we save the ticket that was bought
    _buy_tickets(
        deps,
        env,
        info.sender.clone(),
        raffle_id,
        ticket_number,
        assets,
    )?;

    Ok(Response::new()
        .add_messages(transfer_messages)
        .add_attribute("action", "buy_ticket")
        .add_attribute("raffle_id", raffle_id.to_string())
        .add_attribute("owner", info.sender))
}

/// Buy a ticket for a specific raffle
/// This function is used when sending an asset to the contract directly using a send_msg
/// This is used to buy a ticket using CW20 tokens
/// This function checks the sent message matches the sent assets and buys a ticket internally
pub fn execute_receive(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    wrapper: Cw721ReceiveMsg,
) -> Result<Response, ContractError> {
    let sender = deps.api.addr_validate(&wrapper.sender)?;
    match from_json(&wrapper.msg)? {
        ExecuteMsg::BuyTicket {
            raffle_id,
            ticket_number,
            sent_assets,
        } => {
            // First we make sure the received Asset is the one specified in the message
            match sent_assets.clone() {
                AssetInfo::Cw721Coin(Cw721Coin {
                    address: _address,
                    token_id,
                }) => {
                    if  token_id == wrapper.token_id
                    {
                        // The asset is a match, we can create the raffle object and return
                        _buy_tickets(
                            deps,
                            env,
                            sender.clone(),
                            raffle_id,
                            ticket_number,
                            sent_assets,
                        )?;

                        Ok(Response::new()
                            .add_attribute("action", "buy_ticket")
                            .add_attribute("raffle_id", raffle_id.to_string())
                            .add_attribute("owner", sender))
                    } else {
                        Err(ContractError::AssetMismatch {})
                    }
                }
                _ => Err(ContractError::AssetMismatch {}),
            }
        }
        _ => Err(ContractError::Unauthorized {}),
    }
}

/// Creates new raffle tickets and assigns them to the sender
/// Internal function that doesn't check anything and buys multiple tickets
/// The arguments are described on the execute_buy_tickets function above.
pub fn _buy_tickets(
    deps: DepsMut,
    env: Env,
    owner: Addr,
    raffle_id: u64,
    ticket_number: u32,
    assets: AssetInfo,
) -> Result<(), ContractError> {
    let mut raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;

    // We first check the sent assets match the raffle assets
    if ticket_cost(raffle_info.clone(), ticket_number)? != assets {
        return Err(ContractError::PaiementNotSufficient {
            assets_wanted: raffle_info.raffle_ticket_price,
            assets_received: assets
        });
    }

    // We then check the raffle is in the right state
    can_buy_ticket(env, raffle_info.clone())?;

    // Then we check the user has the right to buy `ticket_number` more tickets
    if let Some(max_ticket_per_address) = raffle_info.raffle_options.max_ticket_per_address {
        let current_ticket_number = USER_TICKETS
            .load(deps.storage, (&owner, raffle_id))
            .unwrap_or(0);
        if current_ticket_number + ticket_number > max_ticket_per_address {
            return Err(ContractError::TooMuchTicketsForUser {
                max: max_ticket_per_address,
                nb_before: current_ticket_number,
                nb_after: current_ticket_number + ticket_number
            });
        }
    }

    // Then we check there are some ticket left to buy
    if let Some(max_participant_number) = raffle_info.raffle_options.max_participant_number {
        if raffle_info.number_of_tickets + ticket_number > max_participant_number {
            return Err(ContractError::TooMuchTickets {
                max: max_participant_number,
                nb_before: raffle_info.number_of_tickets,
                nb_after: raffle_info.number_of_tickets + ticket_number
            });
        }
    };

    // Then we save the sender to the bought tickets
    for n in 0..ticket_number {
        RAFFLE_TICKETS.save(
            deps.storage,
            (raffle_id, raffle_info.number_of_tickets + n),
            &owner,
        )?;
    }

    USER_TICKETS.update::<_, ContractError>(deps.storage, (&owner, raffle_id), |x| match x {
        Some(current_ticket_number) => Ok(current_ticket_number + ticket_number),
        None => Ok(ticket_number),
    })?;
    raffle_info.number_of_tickets += ticket_number;

    RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

    Ok(())
}

/// Update the randomness assigned to a raffle
/// The function receives and checks the randomness against the drand public_key registered with the account.
/// This allows trustless and un-predictable randomness to the raffle contract.
/// The randomness providers will get a small cut of the raffle tickets (to reimburse the tx fees and incentivize adding randomness)
pub fn execute_update_randomness(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    raffle_id: u64,
    randomness: DrandRandomness,
) -> Result<Response, ContractError> {
    // We check the raffle can receive randomness (good state)
    let raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;
    let raffle_state = get_raffle_state(env, raffle_info);
    if raffle_state != RaffleState::Closed {
        return Err(ContractError::WrongStateForRandmness {
            status: raffle_state
        });
    }
    // We assert the randomness is correct
    assert_randomness_origin_and_order(deps.as_ref(), info.sender, raffle_id, randomness)
}

/// Claim and end a raffle
/// This function can be called by anyone
/// This function has 4 purposes :
/// 1. Compute the winner of a raffle (using the last provided randomness) and save it in the contract
/// 2. Send the raffle assets to the winner
/// 3. Send the accumulated ticket prices to the raffle owner
/// 4. Send the fees (a cut of the accumulated ticket prices) to the treasury and the randomness provider
pub fn execute_claim(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    raffle_id: u64,
) -> Result<Response, ContractError> {
    // Loading the raffle object
    let mut raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;

    // We make sure the raffle is ended
    let raffle_state = get_raffle_state(env.clone(), raffle_info.clone());
    if raffle_state != RaffleState::Finished {
        return Err(ContractError::WrongStateForClaim {
            status: raffle_state
        });
    }

    // If there was no participant, the winner is the raffle owner and we pay no fees whatsoever
    if raffle_info.number_of_tickets == 0u32 {
        raffle_info.winner = Some(raffle_info.owner.clone());
    } else {
        // We get the winner of the raffle and save it to the contract. The raffle is now claimed !
        let winner = get_raffle_winner(deps.as_ref(), env.clone(), raffle_id, raffle_info.clone())?;
        raffle_info.winner = Some(winner);
    }
    RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

    // We send the assets to the winner
    let winner_transfer_messages = get_raffle_winner_messages(env.clone(), raffle_info.clone())?;
    let funds_transfer_messages =
        get_raffle_owner_finished_messages(deps.storage, env, raffle_info.clone())?;
    // We distribute the ticket prices to the owner and in part to the treasury
    Ok(Response::new()
        .add_messages(winner_transfer_messages)
        .add_messages(funds_transfer_messages)
        .add_attribute("action", "claim")
        .add_attribute("raffle_id", raffle_id.to_string())
        .add_attribute("winner", raffle_info.winner.unwrap()))
}
