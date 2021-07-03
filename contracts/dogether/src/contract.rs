use cosmwasm_std::{entry_point, to_binary, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128, Uint64, WasmMsg, WasmQuery, Decimal, StdError};

use crate::error::ContractError;
use crate::msg::{CountResponse, ExecuteMsg, InstantiateMsg, QueryMsg, Anchor, EpochStateResponse};
use crate::state::{Config, State, store_config, read_state, read_config, store_state};
use crate::taxation::deduct_tax;
use cw20;
use cw20_base_dogether;
use loterra_staking_contract_dogether;
use crate::math::{decimal_multiplication_in_256, decimal_subtraction_in_256, decimal_summation_in_256, decimal_div_in_256};
use cosmwasm_bignumber::{Decimal256, Uint256};
use std::ops::{Mul, Sub};
use std::str::FromStr;

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[entry_point]
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
    };
    store_config(deps.storage, &config)?;

    let state = State{
        staking_address: deps.api.addr_canonicalize("addr0002")?,
        cw20_address: deps.api.addr_canonicalize("addr0003")?,
        draw_period: 0,
        total_ust_pool: Uint128(150_000_000_000)
    };
    store_state(deps.storage, &state)?;

    let instantiation_cw20 = WasmMsg::Instantiate {
        admin: None,
        code_id: msg.code_id_cw20,
        msg: msg.message_cw20,
        send: vec![],
        label: msg.label_cw20,
    };

    let instantiation_staking = WasmMsg::Instantiate {
        admin: None,
        code_id: msg.code_id_staking,
        msg: msg.message_staking,
        send: vec![],
        label: msg.label_staking,
    };

    Ok(Response {
        submessages: vec![],
        messages: vec![instantiation_cw20.into(), instantiation_staking.into()],
        attributes: vec![],
        data: None,
    })
}

// And declare a custom Error variant for the ones where you will want to make use of it
#[entry_point]
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
pub fn try_pool(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
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
       TODO: deposit stable coin to anchor contract
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
                denom: config.denom,
                amount: sent,
            },
        )?],
    };

    /*
      Bond is a customized cw20 message who mint some cw20 tokens
      and stake at the same times
      TODO: Customize the cw20 base contract and add Bond msg
      @message: Bond
      @params: (contract_address: String, amount: Uint128, msg: Binary, recipient: String)
   */
    let bond = cw20_base_dogether::msg::ExecuteMsg::Bond {
        contract: deps.api.addr_humanize(&state.staking_address)?.to_string(),
        amount: sent,
        msg: Binary::from_base64("eyAiYm9uZF9zdGFrZSI6IHt9IH0=")?,
        recipient: info.sender.to_string()
    };

    let bond_msg = WasmMsg::Execute {
        contract_addr: "".to_string(),
        msg: to_binary(&bond)?,
        send: vec![]
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
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut state = read_state(deps.storage)?;
    let config = read_config(deps.storage)?;
    if amount.is_zero() {
        return Err(ContractError::EmptyAmount {});
    }
    /*
       TODO: Call staking contract in order to init unPool with un-bonding period
    */
    let un_bond = loterra_staking_contract_dogether::msg::ExecuteMsg::UnbondStake { amount, address: info.sender.to_string()};
    let msg_un_bond = WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&state.staking_address)?.to_string(),
        msg: to_binary(&un_bond)?,
        send: vec![]
    };

    Ok(Response{
        submessages: vec![],
        messages: vec![msg_un_bond.into()],
        attributes: vec![],
        data: None
    })
}

pub fn try_claim_un_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = read_state(deps.storage)?;
    let config = read_config(deps.storage)?;
    /*
       TODO: Call staking contract in order to withdrawal unPool with un-bonding period succeed
    */
    // Remove UST amount pooled
    //state.total_ust_pool = state.total_ust_pool.checked_sub(amount).unwrap();
    //store_state(deps.storage, &state)?;
    Ok(Response::default())
}
pub fn try_redeem_earning(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let state = read_state(deps.storage)?;
    let config = read_config(deps.storage)?;
    /*
       TODO: Multiply with anchor tax
       TODO: Redeem earning from anchor
    */
    let epoch = Anchor::EpochState { block_height: None, distributed_interest: None };
    let msg_epoch = WasmQuery::Smart { contract_addr: deps.api.addr_humanize(&config.money_market_address)?.to_string(), msg: to_binary(&epoch)?};
    let res :EpochStateResponse = deps.querier.query(&msg_epoch.into())?;
    println!("{}", res.exchange_rate);
    // TODO: this calculation need decimal256
    let total_ust_pool = Decimal::from_ratio(state.total_ust_pool, Uint128(1));
    println!("{}", total_ust_pool);
    let total_with_interest_ust = decimal_multiplication_in_256(total_ust_pool, res.exchange_rate.into());
    println!("{}", total_with_interest_ust);
    let interest_ust =
        decimal_subtraction_in_256 (total_with_interest_ust, total_ust_pool);
    println!("{}", interest_ust);
    let interest_a_ust = Decimal256::from(interest_ust) / res.exchange_rate;
        //decimal_div_in_256(interest_ust, res.exchange_rate.into());

    let interest_to_withdraw =Uint256::from(interest_a_ust.0);
   // let x = Uint128::from(Decimal::from(interest_a_ust.into()));
    let e = Decimal::from(interest_a_ust.into()) * Uint128(1);
    println!("{}, {}", interest_to_withdraw, e);

    //println!("{:?}", get_decimals(interest_a_ust));
    //let all_reward_with_decimals =  decimal_summation_in_256( Decimal::from_ratio(Uint128(7500000000), Uint128(1)), get_decimals(interest_a_ust)?);
    //println!("{}", all_reward_with_decimals);
    /*
       TODO: Calculation difference in stake
    */
    let total_pooled = state.total_ust_pool;
    /*
        Send earning to staking contract
     */
    let update_global_index = loterra_staking_contract_dogether::msg::ExecuteMsg::UpdateGlobalIndex {};
    let msg_update_global_index = WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&state.staking_address)?.to_string(),
        msg: to_binary(&update_global_index)?,
        send: vec![Coin{ denom: config.denom, amount: Default::default() }]
    };
    Ok(Response{
        submessages: vec![],
        messages: vec![msg_update_global_index.into()],
        attributes: vec![],
        data: None
    })
}

// calculate the reward with decimal
fn get_decimals(value: Decimal) -> StdResult<Decimal> {
    let stringed: &str = &*value.to_string();
    let parts: &[&str] = &*stringed.split('.').collect::<Vec<&str>>();
    match parts.len() {
        1 => Ok(Decimal::zero()),
        2 => {
            let decimals = Decimal::from_str(&*("0.".to_owned() + parts[1]))?;
            Ok(decimals)
        }
        _ => Err(StdError::generic_err("Unexpected number of dots")),
    }
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

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        _ => Ok(Default::default())
    }
}

fn query_count(deps: Deps) -> StdResult<u64> {
    Ok(10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use crate::mock_querier::{mock_dependencies};
    use cosmwasm_std::{coins, from_binary};
    fn default_init(deps: DepsMut) {
        let msg = InstantiateMsg {
            code_id_cw20: 0,
            message_cw20: Default::default(),
            label_cw20: "".to_string(),
            code_id_staking: 0,
            message_staking: Default::default(),
            label_staking: "".to_string(),
            money_market_address: "addr0001".to_string(),
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
            code_id_staking: 0,
            message_staking: Default::default(),
            label_staking: "".to_string(),
            money_market_address: "addr0001".to_string(),
        };
        let info = mock_info("addr0000", &coins(1000, "uusd"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.messages.len());

        // it worked, let's query the state
        //let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        //let value: CountResponse = from_binary(&res).unwrap();
        //assert_eq!(17, value.count);
    }

    #[test]
    fn pool_tokens() {
        let mut deps = mock_dependencies(&[]);
        default_init(deps.as_mut());
        let info = mock_info("addr0000", &coins(100, "uusd"));
        let env = mock_env();
        let res = try_pool(deps.as_mut(), env, info);
        println!("{:?}", res)
    }

    #[test]
    fn redeem_earning() {
        let mut deps = mock_dependencies(&[]);
        default_init(deps.as_mut());
        let info = mock_info("addr0000", &coins(100, "uusd"));
        let env = mock_env();
        let res = try_redeem_earning(deps.as_mut(), env, info);
        println!("{:?}", res)
    }

    /* #[test]
    fn increment() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }*/
}
