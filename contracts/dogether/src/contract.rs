use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, BankMsg, Binary, CanonicalAddr, Coin, ContractResult,
    CosmosMsg, Decimal, Deps, DepsMut, Env, Fraction, MessageInfo, Reply, ReplyOn, Response,
    StdError, StdResult, SubMsg, SubcallResponse, Uint128, Uint64, WasmMsg, WasmQuery,
};

use crate::error::ContractError;
use crate::math::{
    decimal_multiplication_in_256, decimal_subtraction_in_256, decimal_summation_in_256,
};
use crate::msg::{Anchor, CountResponse, EpochStateResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{read_config, read_state, store_config, store_state, Config, State};
use crate::taxation::deduct_tax;
use cosmwasm_bignumber::{Decimal256, Uint256};
use cw20;
use cw20_base_dogether;
use loterra_staking_contract_dogether;
use std::ops::{Mul, Sub};
use std::str::FromStr;

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        admin: deps.api.addr_canonicalize(info.sender.as_str())?,
        denom: "uusd".to_string(),
        money_market_address: deps
            .api
            .addr_canonicalize(msg.money_market_address.as_str())?,
        anchor_aust_address: deps
            .api
            .addr_canonicalize(msg.anchor_aust_address.as_str())?,
        code_id_staking: msg.code_id_staking,
        label_staking: msg.label_staking,
        unbonding_period: msg.unbonding_period,
        loterra_address: deps.api.addr_canonicalize(msg.loterra_address.as_str())?,
    };
    store_config(deps.storage, &config)?;

    let state = State {
        staking_address: deps.api.addr_canonicalize(info.sender.as_str())?,
        cw20_address: deps.api.addr_canonicalize(info.sender.as_str())?,
        draw_period: msg.draw_period,
        next_draw: msg.next_draw,
        total_ust_pool: Uint128::zero(),
    };
    store_state(deps.storage, &state)?;

    let instantiation_cw20 = WasmMsg::Instantiate {
        admin: None,
        code_id: msg.code_id_cw20,
        msg: msg.message_cw20,
        send: vec![],
        label: msg.label_cw20,
    };
    let cosmos_msg_cw20 = CosmosMsg::Wasm(instantiation_cw20);
    let sub_msg_cw20 = SubMsg {
        id: 0,
        msg: cosmos_msg_cw20,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };
    Ok(Response {
        submessages: vec![sub_msg_cw20],
        messages: vec![],
        attributes: vec![
            attr("instantiate", "Dogether"),
            attr("instantiate_cw20", "Dogether cw20"),
            attr("instantiate_staking", "Dogether staking"),
        ],
        data: None,
    })
}

// And declare a custom Error variant for the ones where you will want to make use of it
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Pool {} => try_pool(deps, env, info),
        ExecuteMsg::UnPool { amount } => try_un_pool(deps, env, info, amount),
        ExecuteMsg::ClaimUnPool {} => try_claim_un_pool(deps, env, info),
        ExecuteMsg::RedeemEarning {} => try_redeem_earning(deps, env, info),
    }
}
pub fn try_pool(deps: DepsMut, _env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let mut state = read_state(deps.storage)?;
    let config = read_config(deps.storage)?;

    let sent = match info.funds.len() {
        0 => Err(ContractError::EmptyFunds {}),
        1 => {
            if info.funds[0].denom != config.denom {
                return Err(ContractError::WrongDenom {});
            }
            Ok(info.funds[0].amount)
        }
        _ => Err(ContractError::MultipleDenoms {}),
    }?;
    /*
       Deposit stable coin to anchor contract
    */
    let deposit = Anchor::DepositStable {};
    let deposit_msg = WasmMsg::Execute {
        contract_addr: deps
            .api
            .addr_humanize(&config.money_market_address)?
            .to_string(),
        msg: to_binary(&deposit)?,
        send: vec![deduct_tax(
            &deps.querier,
            Coin {
                denom: config.denom.clone(),
                amount: sent,
            },
        )?],
    };

    /*
       Bond is a customized cw20 message who mint some cw20 tokens
       and stake at the same times

       @message: Bond
       @params: (contract_address: String, amount: Uint128, msg: Binary, recipient: String)
    */

    let bond = cw20_base_dogether::msg::ExecuteMsg::Bond {
        contract: deps.api.addr_humanize(&state.staking_address)?.to_string(),
        amount: deduct_tax(
            &deps.querier,
            Coin {
                denom: config.denom,
                amount: sent,
            },
        )?
        .amount,
        msg: to_binary(&loterra_staking_contract_dogether::msg::ReceiveMsg::BondStake {})?,
        recipient: info.sender.to_string(),
    };

    let bond_msg = WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&state.cw20_address)?.to_string(),
        msg: to_binary(&bond)?,
        send: vec![],
    };

    // Add UST amount pooled
    state.total_ust_pool = state.total_ust_pool.checked_add(sent).unwrap();
    store_state(deps.storage, &state)?;

    Ok(Response {
        submessages: vec![],
        messages: vec![deposit_msg.into(), bond_msg.into()],
        attributes: vec![],
        data: None,
    })
}
pub fn try_un_pool(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut state = read_state(deps.storage)?;
    let _config = read_config(deps.storage)?;
    /*
       We should probably query staking contract in order to check the user balance ?
    */
    if amount.is_zero() {
        return Err(ContractError::EmptyAmount {});
    }
    /*
       Call staking contract in order to init unPool with un-bonding period
    */
    let un_bond = loterra_staking_contract_dogether::msg::ExecuteMsg::UnbondStake {
        amount,
        address: info.sender.to_string(),
    };
    let msg_un_bond = WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&state.staking_address)?.to_string(),
        msg: to_binary(&un_bond)?,
        send: vec![],
    };

    Ok(Response {
        submessages: vec![],
        messages: vec![msg_un_bond.into()],
        attributes: vec![],
        data: None,
    })
}

pub fn try_claim_un_pool(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = read_state(deps.storage)?;
    /*
       Call staking contract in order to withdrawal unPool with un-bonding period succeed
    */
    /*
       @burn: un-pool claiming force staking contract to burn the amount of cw20
       @refund: get the right refund amount of aUst to redeem UST and send UST back to the sender
    */
    let withdraw = loterra_staking_contract_dogether::msg::ExecuteMsg::WithdrawStake {
        cap: None,
        address: info.sender.to_string(),
    };
    let withdraw_msg = WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&state.staking_address)?.to_string(),
        msg: to_binary(&withdraw)?,
        send: vec![],
    };
    let cosmos_msg = CosmosMsg::Wasm(withdraw_msg);

    let sub_msg = SubMsg {
        id: 2,
        msg: cosmos_msg,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    // Remove UST amount pooled
    //state.total_ust_pool = state.total_ust_pool.checked_sub(amount).unwrap();
    // store_state(deps.storage, &state)?;
    Ok(Response {
        submessages: vec![sub_msg],
        messages: vec![],
        attributes: vec![],
        data: None,
    })
}
pub fn try_redeem_earning(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = read_state(deps.storage)?;

    let config = read_config(deps.storage)?;
    if !info.funds.is_empty() {
        return Err(ContractError::DoNotSendFunds {});
    }
    if env.block.height < state.next_draw {
        return Err(ContractError::RetryRedeemLater(state.next_draw));
    }
    /*
       Multiply with anchor tax
       Redeem earning from anchor
    */
    let epoch = Anchor::EpochState {
        block_height: None,
        distributed_interest: None,
    };
    let msg_epoch = WasmQuery::Smart {
        contract_addr: deps
            .api
            .addr_humanize(&config.money_market_address)?
            .to_string(),
        msg: to_binary(&epoch)?,
    };
    let res: EpochStateResponse = deps.querier.query(&msg_epoch.into())?;
    let total_ust_pool = Decimal::from_ratio(state.total_ust_pool, Uint128(1));
    let total_with_interest_ust =
        decimal_multiplication_in_256(total_ust_pool, res.exchange_rate.into());
    let interest_ust = decimal_subtraction_in_256(total_with_interest_ust, total_ust_pool);
    //let interest_a_ust_decimal = Decimal256::from(interest_ust) / res.exchange_rate;
    //println!("{}", interest_a_ust_decimal);
    let interest_a_ust_decimal =
        Decimal256::from_ratio(Decimal256::from(interest_ust).0, res.exchange_rate.0);
    println!("{}", interest_a_ust_decimal);
    //let interest_to_withdraw =Uint256::from(interest_a_ust.0);
    // let x = Uint128::from(Decimal::from(interest_a_ust.into()));
    //decimal_summation_in_256(interest_ust, Decimal::from_ratio(interest_ust, res.exchange_rate));
    let interest_to_withdraw = Decimal::from(interest_a_ust_decimal.into()) * Uint128(1);
    println!("{}", interest_to_withdraw);

    //println!("{:?}", get_decimals(interest_a_ust));
    //let all_reward_with_decimals =  decimal_summation_in_256( Decimal::from_ratio(Uint128(7500000000), Uint128(1)), get_decimals(interest_a_ust)?);
    //println!("{}", all_reward_with_decimals);

    /*
          Redeem stable coin from anchor
    */
    let redeem = cw20::Cw20ExecuteMsg::Send {
        contract: deps
            .api
            .addr_humanize(&config.money_market_address)?
            .to_string(),
        amount: interest_to_withdraw,
        msg: Some(to_binary(&Anchor::RedeemStable {})?),
    };
    let msg_redeem = WasmMsg::Execute {
        contract_addr: deps
            .api
            .addr_humanize(&config.anchor_aust_address)?
            .to_string(),
        msg: to_binary(&redeem)?,
        send: vec![],
    };

    // Get the total contract balance and send all ust to staking contract
    let contract_balance = deps
        .querier
        .query_balance(env.contract.address, config.denom.clone())?;

    /*
       Send earning to staking contract
    */
    let update_global_index =
        loterra_staking_contract_dogether::msg::ExecuteMsg::UpdateGlobalIndex {};
    let msg_update_global_index = WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&state.staking_address)?.to_string(),
        msg: to_binary(&update_global_index)?,
        send: vec![deduct_tax(
            &deps.querier,
            Coin {
                denom: config.denom,
                amount: contract_balance.amount,
            },
        )?],
    };

    state.next_draw = env.block.height.checked_add(state.draw_period).unwrap();
    store_state(deps.storage, &state)?;

    Ok(Response {
        submessages: vec![],
        messages: vec![msg_redeem.into(), msg_update_global_index.into()],
        attributes: vec![],
        data: None,
    })
}

/*pub fn try_increment(deps: DepsMut) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.count += 1;
        Ok(state)
    })?;

    Ok(Response::default())
}

pub fn try_reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        state.count = count;
        Ok(state)
    })?;
    Ok(Response::default())
}*/
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    let config = read_config(deps.storage)?;
    match msg.id {
        0 => cw20_instance_reply(deps, env, msg.result),
        1 => staking_instance_reply(deps, env, msg.result),
        2 => withdraw_reply(deps, env, msg.result),
        _ => Err(ContractError::Unauthorized {}),
    }
}

pub fn cw20_instance_reply(
    deps: DepsMut,
    _env: Env,
    msg: ContractResult<SubcallResponse>,
) -> Result<Response, ContractError> {
    let mut state = read_state(deps.storage)?;
    let config = read_config(deps.storage)?;
    match msg {
        ContractResult::Ok(subcall) => {
            let contract_address = subcall
                .events
                .into_iter()
                .find(|e| e.kind == "instantiate_contract")
                .and_then(|ev| {
                    ev.attributes
                        .into_iter()
                        .find(|attr| attr.key == "contract_address")
                        .and_then(|addr| Some(addr.value))
                })
                .unwrap();
            state.cw20_address = deps.api.addr_canonicalize(&contract_address.as_str())?;
            store_state(deps.storage, &state)?;

            let data = loterra_staking_contract_dogether::msg::InstantiateMsg {
                cw20_token_addr: Addr::unchecked(contract_address.clone()),
                loterra_addr: deps.api.addr_humanize(&config.loterra_address)?,
                reward_denom: config.denom,
                unbonding_period: config.unbonding_period,
            };
            let instantiation_staking = WasmMsg::Instantiate {
                admin: None,
                code_id: config.code_id_staking,
                msg: to_binary(&data)?,
                send: vec![],
                label: config.label_staking,
            };
            let cosmos_msg_staking = CosmosMsg::Wasm(instantiation_staking);
            let sub_msg_staking = SubMsg {
                id: 1,
                msg: cosmos_msg_staking,
                gas_limit: None,
                reply_on: ReplyOn::Success,
            };

            Ok(Response {
                submessages: vec![sub_msg_staking],
                messages: vec![],
                attributes: vec![
                    attr("cw20-address", contract_address),
                    attr("cw20-instantiate", "success"),
                ],
                data: None,
            })
        }
        ContractResult::Err(_) => Err(ContractError::Unauthorized {}),
    }
}
pub fn staking_instance_reply(
    deps: DepsMut,
    _env: Env,
    msg: ContractResult<SubcallResponse>,
) -> Result<Response, ContractError> {
    let mut state = read_state(deps.storage)?;
    match msg {
        ContractResult::Ok(subcall) => {
            let contract_address = subcall
                .events
                .into_iter()
                .find(|e| e.kind == "instantiate_contract")
                .and_then(|ev| {
                    ev.attributes
                        .into_iter()
                        .find(|attr| attr.key == "contract_address")
                        .and_then(|addr| Some(addr.value))
                })
                .unwrap();
            state.staking_address = deps.api.addr_canonicalize(&contract_address.as_str())?;
            store_state(deps.storage, &state)?;
            Ok(Response {
                submessages: vec![],
                messages: vec![],
                attributes: vec![
                    attr("staking-address", contract_address),
                    attr("staking-instantiate", "success"),
                ],
                data: None,
            })
        }
        ContractResult::Err(_) => Err(ContractError::Unauthorized {}),
    }
}

pub fn withdraw_reply(
    deps: DepsMut,
    env: Env,
    msg: ContractResult<SubcallResponse>,
) -> Result<Response, ContractError> {
    let mut state = read_state(deps.storage)?;
    let config = read_config(deps.storage)?;
    match msg {
        ContractResult::Ok(subcall) => {
            let (withdrawing_amount, recipient) = subcall
                .events
                .into_iter()
                .find(|e| e.kind == "message")
                .and_then(|ev| {
                    let amount = ev
                        .attributes
                        .clone()
                        .into_iter()
                        .find(|attr| attr.key == "withdraw")
                        .and_then(|withdraw| Some(withdraw.value));
                    let recipient = ev
                        .attributes
                        .into_iter()
                        .find(|attr| attr.key == "recipient")
                        .and_then(|recipient| Some(recipient.value));
                    Some((amount, recipient))
                })
                .unwrap();
            let amount_to_withdraw =
                Uint128::from(withdrawing_amount.unwrap().parse::<u128>().unwrap());

            /*
               Calculation of aUST amount and withdraw from anchor.
            */
            let epoch = Anchor::EpochState {
                block_height: None,
                distributed_interest: None,
            };
            let msg_epoch = WasmQuery::Smart {
                contract_addr: deps
                    .api
                    .addr_humanize(&config.money_market_address)?
                    .to_string(),
                msg: to_binary(&epoch)?,
            };
            let res: EpochStateResponse = deps.querier.query(&msg_epoch.into())?;
            let total_ust_pool = Decimal::from_ratio(amount_to_withdraw, Uint128(1));
            let interest_a_ust_decimal =
                Decimal256::from_ratio(Decimal256::from(total_ust_pool).0, res.exchange_rate.0);
            let interest_to_withdraw = Decimal::from(interest_a_ust_decimal.into()) * Uint128(1);

            /*
                Redeem stable coin from anchor
            */
            let redeem = cw20::Cw20ExecuteMsg::Send {
                contract: deps
                    .api
                    .addr_humanize(&config.money_market_address)?
                    .to_string(),
                amount: interest_to_withdraw,
                msg: Some(to_binary(&Anchor::RedeemStable {})?),
            };
            let msg_redeem = WasmMsg::Execute {
                contract_addr: deps
                    .api
                    .addr_humanize(&config.anchor_aust_address)?
                    .to_string(),
                msg: to_binary(&redeem)?,
                send: vec![],
            };
            // Get the total contract balance and send ust
            let contract_balance = deps
                .querier
                .query_balance(env.contract.address, config.denom.clone())?;

            if contract_balance.amount >= amount_to_withdraw {
                return Err(ContractError::NotEnoughFunds {});
            }

            state.total_ust_pool = state
                .total_ust_pool
                .checked_sub(amount_to_withdraw)
                .unwrap();
            store_state(deps.storage, &state)?;

            let net_amount = deduct_tax(
                &deps.querier,
                Coin {
                    denom: config.denom.clone(),
                    amount: amount_to_withdraw,
                },
            )?;
            let bank_msg = CosmosMsg::Bank(BankMsg::Send {
                to_address: recipient.unwrap(),
                amount: vec![net_amount.clone()],
            });
            Ok(Response {
                submessages: vec![],
                messages: vec![msg_redeem.into(), bank_msg],
                attributes: vec![
                    attr("withdraw", net_amount.amount),
                    attr("status", "success"),
                ],
                data: None,
            })
        }
        ContractResult::Err(_) => Err(ContractError::Unauthorized {}),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        _ => Ok(Default::default()),
    }
}

/*
fn query_count(deps: Deps) -> StdResult<u64> {
    Ok(10)
}
*/
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_querier::mock_dependencies;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Api, Attribute, CosmosMsg, Empty, Event, ReplyOn};
    use cw20::Cw20ExecuteMsg;

    fn default_init(deps: DepsMut) {
        let msg = InstantiateMsg {
            code_id_cw20: 0,
            message_cw20: Default::default(),
            label_cw20: "".to_string(),
            code_id_staking: 1,
            label_staking: "".to_string(),
            money_market_address: "money".to_string(),
            anchor_aust_address: "aust".to_string(),
            next_draw: 1000,
            draw_period: 100,
            unbonding_period: 100_000,
            loterra_address: "loterra".to_string(),
        };
        let info = mock_info("addr0000", &coins(1000, "uusd"));
        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps, mock_env(), info, msg).unwrap();
    }
    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&[]);
        let msg = InstantiateMsg {
            code_id_cw20: 0,
            message_cw20: Default::default(),
            label_cw20: "".to_string(),
            code_id_staking: 1,
            label_staking: "".to_string(),
            money_market_address: "money".to_string(),
            anchor_aust_address: "aust".to_string(),
            next_draw: 100,
            draw_period: 1000,
            unbonding_period: 100_000,
            loterra_address: "loterra".to_string(),
        };
        let info = mock_info("addr0000", &coins(1000, "uusd"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        //let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        //let value: CountResponse = from_binary(&res).unwrap();
        //assert_eq!(17, value.count);
    }

    #[test]
    fn pool_tokens() {
        let mut deps = mock_dependencies(&[]);
        default_init(deps.as_mut());
        let info = mock_info("addr0000", &[]);
        let env = mock_env();
        let res = try_pool(deps.as_mut(), env, info);
        println!("{:?}", res);
        match res {
            Err(ContractError::EmptyFunds {}) => {}
            _ => panic!("Do not enter here"),
        }

        let info = mock_info("addr0000", &coins(100, "uusd"));
        let env = mock_env();
        // Instantiate contract cw20
        let rep = Reply {
            id: 0,
            result: ContractResult::Ok(SubcallResponse {
                events: vec![Event {
                    kind: "instantiate_contract".to_string(),
                    attributes: vec![attr("contract_address", "cw20")],
                }],
                data: None,
            }),
        };
        let res = reply(deps.as_mut(), env.clone(), rep).unwrap();
        // Instantiate contract staking
        let rep = Reply {
            id: 1,
            result: ContractResult::Ok(SubcallResponse {
                events: vec![Event {
                    kind: "instantiate_contract".to_string(),
                    attributes: vec![attr("contract_address", "staking")],
                }],
                data: None,
            }),
        };
        let res = reply(deps.as_mut(), env.clone(), rep).unwrap();

        // Try pool
        let res = try_pool(deps.as_mut(), env, info.clone()).unwrap();
        println!("{:?}", res);
        let state = read_state(deps.as_ref().storage).unwrap();
        let deposit = Anchor::DepositStable {};
        let bond = cw20_base_dogether::msg::ExecuteMsg::Bond {
            contract: deps
                .api
                .addr_humanize(&state.staking_address)
                .unwrap()
                .to_string(),
            amount: Uint128(99),
            msg: to_binary(&loterra_staking_contract_dogether::msg::ReceiveMsg::BondStake {})
                .unwrap(),
            recipient: info.sender.to_string(),
        };
        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "money".to_string(),
                    msg: to_binary(&deposit).unwrap(),
                    send: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128(99)
                    }]
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "cw20".to_string(),
                    msg: to_binary(&bond).unwrap(),
                    send: vec![]
                })
            ]
        )
    }
    #[test]
    fn un_pool() {
        let mut deps = mock_dependencies(&[]);
        default_init(deps.as_mut());
        let info = mock_info("addr0000", &[]);
        let env = mock_env();
        // Instantiate contract staking
        let rep = Reply {
            id: 1,
            result: ContractResult::Ok(SubcallResponse {
                events: vec![Event {
                    kind: "instantiate_contract".to_string(),
                    attributes: vec![attr("contract_address", "staking")],
                }],
                data: None,
            }),
        };
        let res = reply(deps.as_mut(), env.clone(), rep).unwrap();

        let msg = ExecuteMsg::UnPool {
            amount: Uint128(1_000),
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let un_bond_msg = loterra_staking_contract_dogether::msg::ExecuteMsg::UnbondStake {
            amount: Uint128(1_000),
            address: "addr0000".to_string(),
        };
        println!("{:?}", res);

        assert_eq!(
            res.messages,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "staking".to_string(),
                msg: to_binary(&un_bond_msg).unwrap(),
                send: vec![]
            })]
        );

        let msg = ExecuteMsg::UnPool { amount: Uint128(0) };
        let res = execute(deps.as_mut(), env, info, msg);
        match res {
            Err(ContractError::EmptyAmount {}) => {}
            _ => panic!("Do not enter here"),
        }
    }
    #[test]
    fn claim_un_pool() {
        let mut deps = mock_dependencies(&[]);
        default_init(deps.as_mut());
        let info = mock_info("addr0000", &[]);
        let env = mock_env();
        // Instantiate contract staking
        let rep = Reply {
            id: 1,
            result: ContractResult::Ok(SubcallResponse {
                events: vec![Event {
                    kind: "instantiate_contract".to_string(),
                    attributes: vec![attr("contract_address", "staking")],
                }],
                data: None,
            }),
        };
        let res = reply(deps.as_mut(), env.clone(), rep).unwrap();

        let res = try_claim_un_pool(deps.as_mut(), env, info).unwrap();
        println!("{:?}", res);
        let msg = loterra_staking_contract_dogether::msg::ExecuteMsg::WithdrawStake {
            cap: None,
            address: "addr0000".to_string(),
        };
        let wasm_msg = WasmMsg::Execute {
            contract_addr: "staking".to_string(),
            msg: to_binary(&msg).unwrap(),
            send: vec![],
        };
        let submessage = SubMsg {
            id: 2,
            msg: CosmosMsg::Wasm(wasm_msg),
            gas_limit: None,
            reply_on: ReplyOn::Success,
        };
        assert_eq!(
            res,
            Response {
                submessages: vec![submessage],
                messages: vec![],
                attributes: vec![],
                data: None
            }
        )
    }
    #[test]
    fn redeem_earning() {
        let mut deps = mock_dependencies(&[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(7_500_000_000),
        }]);
        default_init(deps.as_mut());
        let mut state = read_state(deps.as_ref().storage).unwrap();
        state.total_ust_pool = Uint128(150_000_000_000);
        store_state(deps.as_mut().storage, &state).unwrap();
        // Sending funds error
        let info = mock_info("addr0000", &coins(100, "uusd"));
        let env = mock_env();
        // Instantiate contract cw20
        let rep = Reply {
            id: 0,
            result: ContractResult::Ok(SubcallResponse {
                events: vec![Event {
                    kind: "instantiate_contract".to_string(),
                    attributes: vec![attr("contract_address", "cw20")],
                }],
                data: None,
            }),
        };
        let res = reply(deps.as_mut(), env.clone(), rep).unwrap();
        // Instantiate contract staking
        let rep = Reply {
            id: 1,
            result: ContractResult::Ok(SubcallResponse {
                events: vec![Event {
                    kind: "instantiate_contract".to_string(),
                    attributes: vec![attr("contract_address", "staking")],
                }],
                data: None,
            }),
        };
        let res = reply(deps.as_mut(), env.clone(), rep).unwrap();

        // try redeem
        let res = try_redeem_earning(deps.as_mut(), env, info);
        match res {
            Err(ContractError::DoNotSendFunds {}) => {}
            _ => panic!("Do not enter here"),
        }
        let info = mock_info("addr0000", &[]);
        let env = mock_env();
        let state_before = read_state(deps.as_ref().storage).unwrap();
        let res = try_redeem_earning(deps.as_mut(), env.clone(), info).unwrap();
        let state = read_state(deps.as_ref().storage).unwrap();
        println!("{:?}", res);
        assert_eq!(state_before.total_ust_pool, state.total_ust_pool);
        assert!(state_before.next_draw < state.next_draw);
        assert_eq!(state.next_draw, env.block.height + state.draw_period);
        assert_eq!(state_before.draw_period, state.draw_period);
        assert_eq!(state_before.staking_address, state.staking_address);
        assert_eq!(state_before.cw20_address, state.cw20_address);

        let update_global_index =
            loterra_staking_contract_dogether::msg::ExecuteMsg::UpdateGlobalIndex {};
        let redeem = cw20::Cw20ExecuteMsg::Send {
            contract: "money".to_string(),
            amount: Uint128(7_142_857_142),
            msg: Some(to_binary(&Anchor::RedeemStable {}).unwrap()),
        };
        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "aust".to_string(),
                    msg: to_binary(&redeem).unwrap(),
                    send: vec![]
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "staking".to_string(),
                    msg: to_binary(&update_global_index).unwrap(),
                    send: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128(7_499_000_000)
                    }]
                })
            ]
        )
    }

    #[test]
    fn redeem_earning_still_in_progress() {
        let mut deps = mock_dependencies(&[]);
        default_init(deps.as_mut());
        let info = mock_info("addr0000", &[]);
        let mut env = mock_env();
        env.block.height = 100;
        let res = try_redeem_earning(deps.as_mut(), env, info);
        match res {
            Err(ContractError::RetryRedeemLater(msg)) => {
                assert_eq!(1000, msg)
            }
            _ => panic!("Do not enter here"),
        }
    }

    #[test]
    fn reply_cw20_instantiated() {
        let mut deps = mock_dependencies(&[]);
        default_init(deps.as_mut());
        let info = mock_info("addr0000", &[]);
        let mut env = mock_env();
        let state_before = read_state(deps.as_ref().storage).unwrap();
        let rep = Reply {
            id: 0,
            result: ContractResult::Ok(SubcallResponse {
                events: vec![Event {
                    kind: "instantiate_contract".to_string(),
                    attributes: vec![attr("contract_address", "cw20")],
                }],
                data: None,
            }),
        };
        let res = reply(deps.as_mut(), env, rep).unwrap();
        let state = read_state(deps.as_ref().storage).unwrap();
        assert_eq!(
            state.cw20_address,
            deps.api.addr_canonicalize("cw20").unwrap()
        );
        assert_ne!(state_before.cw20_address, state.cw20_address);

        let msg = loterra_staking_contract_dogether::msg::InstantiateMsg {
            cw20_token_addr: Addr::unchecked("cw20"),
            loterra_addr: Addr::unchecked("loterra"),
            reward_denom: "uusd".to_string(),
            unbonding_period: 100000,
        };
        let instantiate_staking = CosmosMsg::Wasm(WasmMsg::Instantiate {
            admin: None,
            code_id: 1,
            msg: to_binary(&msg).unwrap(),
            send: vec![],
            label: "".to_string(),
        });
        let sub_msg = SubMsg {
            id: 1,
            msg: instantiate_staking,
            gas_limit: None,
            reply_on: ReplyOn::Success,
        };
        assert_eq!(
            res,
            Response {
                submessages: vec![sub_msg],
                messages: vec![],
                attributes: vec![
                    attr("cw20-address", "cw20"),
                    attr("cw20-instantiate", "success")
                ],
                data: None
            }
        )
    }

    #[test]
    fn reply_staking_instantiated() {
        let mut deps = mock_dependencies(&[]);
        default_init(deps.as_mut());
        let info = mock_info("addr0000", &[]);
        let mut env = mock_env();
        let state_before = read_state(deps.as_ref().storage).unwrap();
        let rep = Reply {
            id: 1,
            result: ContractResult::Ok(SubcallResponse {
                events: vec![Event {
                    kind: "instantiate_contract".to_string(),
                    attributes: vec![attr("contract_address", "staking")],
                }],
                data: None,
            }),
        };
        let res = reply(deps.as_mut(), env, rep).unwrap();
        let state = read_state(deps.as_ref().storage).unwrap();
        assert_eq!(
            state.staking_address,
            deps.api.addr_canonicalize("staking").unwrap()
        );
        assert_ne!(state_before.staking_address, state.staking_address);
        let d = deps
            .api
            .addr_humanize(&state_before.staking_address)
            .unwrap();

        assert_eq!(
            res,
            Response {
                submessages: vec![],
                messages: vec![],
                attributes: vec![
                    attr("staking-address", "staking"),
                    attr("staking-instantiate", "success")
                ],
                data: None
            }
        )
    }

    #[test]
    fn reply_withdrawal() {
        let mut deps = mock_dependencies(&[]);
        default_init(deps.as_mut());
        let info = mock_info("addr0000", &[]);
        let mut env = mock_env();
        let mut state_before = read_state(deps.as_ref().storage).unwrap();
        state_before.total_ust_pool = Uint128(150_000_000_000);
        store_state(deps.as_mut().storage, &state_before).unwrap();

        let rep = Reply {
            id: 2,
            result: ContractResult::Ok(SubcallResponse {
                events: vec![Event {
                    kind: "message".to_string(),
                    attributes: vec![
                        attr("withdraw", "100000000000"),
                        attr("recipient", "addr0008"),
                    ],
                }],
                data: None,
            }),
        };

        let res = reply(deps.as_mut(), env, rep).unwrap();

        let state = read_state(deps.as_ref().storage).unwrap();
        assert_ne!(state.total_ust_pool, state_before.total_ust_pool);
        assert_eq!(
            state.total_ust_pool,
            state_before
                .total_ust_pool
                .checked_sub(Uint128(100_000_000_000))
                .unwrap()
        );
        println!("{:?}", res);
        let msg_to = Cw20ExecuteMsg::Send {
            contract: "money".to_string(),
            amount: Uint128(95_238_095_238),
            msg: Some(to_binary(&Anchor::RedeemStable {}).unwrap()),
        };
        let wasm_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "aust".to_string(),
            msg: to_binary(&msg_to).unwrap(),
            send: vec![],
        });
        let cosmos_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: "addr0008".to_string(),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128(99_999_000_000),
            }],
        });
        assert_eq!(
            res,
            Response {
                submessages: vec![],
                messages: vec![wasm_msg, cosmos_msg],
                attributes: vec![attr("withdraw", "99999000000"), attr("status", "success")],
                data: None
            }
        )
    }
}
