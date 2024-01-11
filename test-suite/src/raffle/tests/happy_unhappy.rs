// We are happy if these contract calls fail!

use cosmwasm_std::{
    coins,
    testing::{mock_dependencies_with_balance, mock_env, mock_info},
    Api, Uint128, Coin,
};
use raffles::{
    contract::{instantiate, execute},
    msg::{ExecuteMsg, InstantiateMsg}, state::{RaffleOptions, RaffleOptionsMsg},
};
use sg_std::NATIVE_DENOM;
use utils::state::{AssetInfo, Cw721Coin, Sg721Token};

const INITIAL_BALANCE: u128 = 2_000_000_000;
const MANAGER: &str = "creator";
const NAME: &str = "good-name";
const AMOUNT: Uint128 = Uint128::new(50);
const NOIS_PROXY: &str = "nois";

#[test]
fn initialization() {
    let mut deps: cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::testing::MockQuerier,
    > = mock_dependencies_with_balance(&coins(2, "token"));

    // Invalid nois_proxy returns error
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));

    let msg = InstantiateMsg {
        owner: Some(MANAGER.to_string()),
        name: NAME.to_string(),
        nois_proxy_addr: "".to_string(),
        nois_proxy_denom: "ibc/717352A5277F3DE916E8FD6B87F4CA6A51F2FBA9CF04ABCFF2DF7202F8A8BC50"
            .to_string(),
        nois_proxy_amount: AMOUNT.into(),
        fee_addr: None,
        minimum_raffle_duration: None,
        minimum_raffle_timeout: None,
        max_participant_number: None,
        raffle_fee: None,
        rand_fee: None,
        creation_fee_denom: Some(NATIVE_DENOM.to_string()),
        creation_fee_amount: AMOUNT.into(),
    };

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
}

#[test]
fn execution() {
    // Invalid TicketPrice
    let mut deps: cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::testing::MockQuerier,
    > = mock_dependencies_with_balance(&coins(2, "token"));

    let info: cosmwasm_std::MessageInfo = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));

    let instantiate_msg = InstantiateMsg {
        owner: Some(MANAGER.to_string()),
        name: NAME.to_string(),
        nois_proxy_addr: NOIS_PROXY.to_string(),
        nois_proxy_denom: "ibc/717352A5277F3DE916E8FD6B87F4CA6A51F2FBA9CF04ABCFF2DF7202F8A8BC50"
            .to_string(),
        nois_proxy_amount: AMOUNT.into(),
        fee_addr: None,
        minimum_raffle_duration: None,
        minimum_raffle_timeout: None,
        max_participant_number: None,
        raffle_fee: None,
        rand_fee: None,
        creation_fee_denom: Some(NATIVE_DENOM.to_string()),
        creation_fee_amount: AMOUNT.into(),
    };

    // instantiate contract
    instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg.clone()).unwrap();

    // define assets 
    let assets: Vec<AssetInfo> = vec![
        AssetInfo::Cw721Coin(Cw721Coin {
            address: "nft".to_string(),
            token_id: "1".to_string(),
        }),
        AssetInfo::Sg721Token(Sg721Token {
            address: "nft".to_string(),
            token_id: "2".to_string(),
        }),
    ];
    // define raffle options
    let raffle_options =  RaffleOptionsMsg {
        raffle_start_timestamp: None,
        raffle_duration: None,
        raffle_timeout: None,
        comment: None,
        max_participant_number: None,
        max_ticket_per_address: None,
        raffle_preview: None,
    };
    // define improper raffle ticket price
    let bad_ticket_price = AssetInfo::Sg721Token(
        Sg721Token {
            address: "wrong-asset".to_string(),
            token_id: "id".to_string(),
        }
    );
    // define msg
    let bad_raffle_msg = ExecuteMsg::CreateRaffle {
        owner: Some(MANAGER.to_string()),
        assets: assets.clone(),
        raffle_options: raffle_options.clone(),
        raffle_ticket_price: bad_ticket_price,
    };
    // simulate broadcast, expect to unwrap error
    execute(deps.as_mut(), mock_env(), info, bad_raffle_msg).unwrap();


    // // Invalid CancelRaffle
    // let info: cosmwasm_std::MessageInfo = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));

    // instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg.clone()).unwrap();

    // // define proper raffle ticket price
    // let raffle_ticket_price: AssetInfo = AssetInfo::Coin(
    //     Coin {
    //         denom: "eret".to_string(),
    //         amount: AMOUNT.into(),
    //     }
    // );
    // // define msg
    // let create_raffle_msg = ExecuteMsg::CreateRaffle {
    //     owner: Some(MANAGER.to_string()),
    //     assets,
    //     raffle_options,
    //     raffle_ticket_price,
    // };
    // // create raffle
    // execute(deps.as_mut(), mock_env(), info, create_raffle_msg).unwrap();

    // // try to cancel raffle with different owner

    // let bad_cancel_msg = ExecuteMsg::CancelRaffle { raffle_id: () }

}

// EXECUTE TESTS



// Invalid ModifyRaffle
// Invalid BuyTicket
// Invalid ToggleLock
// Invalid UpdateRandomness
