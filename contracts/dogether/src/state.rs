use cosmwasm_std::{CanonicalAddr, Uint128, Storage, StdResult};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub admin: CanonicalAddr,
    pub denom: String,
    pub money_market_address: CanonicalAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub staking_address: CanonicalAddr,
    pub cw20_address: CanonicalAddr,
    pub draw_period: u64,
    pub total_ust_pool: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("config");

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    CONFIG.save(storage, config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    CONFIG.load(storage)
}

pub const STATE: Item<State> = Item::new("state");

pub fn store_state(storage: &mut dyn Storage, state: &State) -> StdResult<()> {
    STATE.save(storage, state)
}

pub fn read_state(storage: &dyn Storage) -> StdResult<State> {
    STATE.load(storage)
}
