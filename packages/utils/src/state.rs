use cosmwasm_std::MessageInfo;
use cosmwasm_std::StdError;
use cosmwasm_std::Coin;
use cosmwasm_std::{Addr, Api, StdResult, Uint128};
use cosmwasm_schema::cw_serde;

#[cw_serde]
pub struct Cw1155Coin {
    pub address: String,
    pub token_id: String,
    pub value: Uint128,
}

#[cw_serde]
pub struct Cw721Coin {
    pub address: String,
    pub token_id: String,
}

#[cw_serde]
pub struct Cw20Coin {
    pub address: String,
    pub amount: Uint128,
}

#[cw_serde]
pub enum AssetInfo {
    Coin(Coin),
    Cw20Coin(Cw20Coin),
    Cw721Coin(Cw721Coin),
    Cw1155Coin(Cw1155Coin),
}

pub fn maybe_addr(api: &dyn Api, human: Option<String>) -> StdResult<Option<Addr>> {
    human.map(|x| api.addr_validate(&x)).transpose()
}

#[cw_serde]
pub struct OwnerStruct{
    pub owner: Addr,
    pub new_owner: Option<Addr>,
}

impl OwnerStruct{

    pub fn new(owner: Addr) -> Self{
        OwnerStruct { owner, new_owner: None }
    }

    pub fn propose_new_owner(mut self, new_owner: Addr) -> Self{
        self.new_owner = Some(new_owner);
        self
    }

    pub fn validate_new_owner(mut self, info: MessageInfo) -> StdResult<Self>{
        if let Some(new_owner) = self.new_owner{
            if info.sender == new_owner{
                self.owner = info.sender;
                self.new_owner = None;
                Ok(self)
            }else{
                Err(StdError::generic_err("Unauthorized"))
            }
        }else{
            Err(StdError::generic_err("Unauthorized"))
        }
    }
}