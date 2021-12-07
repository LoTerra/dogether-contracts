use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Binary, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub code_id_cw20: u64,
    pub message_cw20: Binary,
    pub label_cw20: String,
    pub code_id_staking: u64,
    pub label_staking: String,
    pub money_market_address: String,
    pub anchor_aust_address: String,
    pub next_draw: u64,
    pub draw_period: u64,
    pub unbonding_period: u64,
    pub loterra_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Pool UST to anchor yield in order to get automated ticket with earning
    Pool {},
    /// UnPool tokens and get back UST, un-bonding period required!
    UnPool { amount: Uint128 },
    /// Withdraw unPool tokens after un-bonding period succeed
    ClaimUnPool {},
    /// Redeem earning from anchor
    RedeemEarning {},
    /// Purchase tickets on LoTerra lottery contract with pool rewards coop mode
    GetTicket {
        combination: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Query the state
    State {},
    /// Query the config
    Config {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    /// Deposit stable to anchor
    DepositStable {},
    /// Return stable coins to a user
    /// according to exchange rate
    RedeemStable {},
    /// Query Epoch from anchor
    EpochState {
        block_height: Option<u64>,
        distributed_interest: Option<Uint256>,
    },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CountResponse {
    pub count: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub staking_address: String,
    pub cw20_address: String,
    pub draw_period: u64,
    pub next_draw: u64,
    pub total_ust_pool: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub admin: String,
    pub denom: String,
    pub money_market_address: String,
    pub anchor_aust_address: String,
    pub unbonding_period: u64,
    pub loterra_address: String,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct EpochStateResponse {
    pub exchange_rate: Decimal256,
    pub aterra_supply: Uint256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
