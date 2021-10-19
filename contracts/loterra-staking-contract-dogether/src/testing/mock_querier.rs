use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_slice, to_binary, Addr, Api, BalanceResponse, BankQuery, Binary, CanonicalAddr, Coin,
    ContractResult, Decimal, OwnedDeps, Querier, QuerierResult, QueryRequest, Response, StdError,
    StdResult, SystemError, SystemResult, Uint128, WasmQuery,
};
use std::str::FromStr;
use terra_cosmwasm::{
    ExchangeRateItem, ExchangeRatesResponse, TaxCapResponse, TaxRateResponse, TerraQuery,
    TerraQueryWrapper, TerraRoute,
};

pub const MOCK_HUB_CONTRACT_ADDR: &str = "hub";
pub const MOCK_CW20_CONTRACT_ADDR: &str = "lottery";
//pub const MOCK_REWARD_CONTRACT_ADDR: &str = "reward";
pub const MOCK_TOKEN_CONTRACT_ADDR: &str = "token";

pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));
    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<TerraQueryWrapper>,
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<TerraQueryWrapper> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<TerraQueryWrapper>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                if contract_addr == &"loterra".to_string() {
                    let res = loterra::msg::ConfigResponse {
                        admin: CanonicalAddr(to_binary(&"o").unwrap()),
                        block_time_play: 0,
                        every_block_time_play: 0,
                        denom_stable: "".to_string(),
                        combination_len: 0,
                        jackpot_percentage_reward: 0,
                        token_holder_percentage_fee_reward: 0,
                        fee_for_drand_worker_in_percentage: 0,
                        prize_rank_winner_percentage: vec![],
                        poll_count: 0,
                        poll_default_end_height: 0,
                        price_per_ticket_to_register: Uint128::from(1_u128),
                        terrand_contract_address: CanonicalAddr(to_binary(&"o").unwrap()),
                        loterra_cw20_contract_address: CanonicalAddr(to_binary(&"o").unwrap()),
                        loterra_staking_contract_address: CanonicalAddr(to_binary(&"o").unwrap()),
                        altered_contract_address: CanonicalAddr(to_binary(&"o").unwrap()),
                        safe_lock: false,
                        lottery_counter: 0,
                        holders_bonus_block_time_end: 0,
                        bonus_burn_rate: 0,
                        bonus: 0,
                    };
                    return SystemResult::Ok(ContractResult::from(to_binary(&res)));
                }
                panic!("DO NOT ENTER HERE")
            }
            QueryRequest::Custom(TerraQueryWrapper { route, query_data }) => match query_data {
                TerraQuery::TaxRate {} => {
                    let res = TaxRateResponse {
                        rate: Decimal::percent(1),
                    };
                    SystemResult::Ok(ContractResult::from(to_binary(&res)))
                }
                TerraQuery::TaxCap { denom: _ } => {
                    let cap = Uint128::from(1000000_u128);
                    let res = TaxCapResponse { cap };
                    SystemResult::Ok(ContractResult::from(to_binary(&res)))
                }
                _ => panic!("DO NOT ENTER HERE"),
            },
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<TerraQueryWrapper>) -> Self {
        WasmMockQuerier { base }
    }
}
