use cosmwasm_std::{
    entry_point, to_binary,   CosmosMsg, Deps, DepsMut,Binary,
    Env, MessageInfo, Response, StdResult, Uint128, WasmMsg,BankMsg,Coin, Decimal
};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg,Image, InstantiateMsg, QueryMsg, WhiteUserInfo};
use crate::state::{
    CONFIG,ADMININFO,State, AdminInfo, USERINFO,COLLECTIONINFO, CollectionInfo, FREEMINTER, WHITEUSERS
};
use crate::rand::{sha_256, Prng};

use cw721_base::{ExecuteMsg as Cw721BaseExecuteMsg, MintMsg};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaChaRng;


#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner:msg.owner,
    };
    CONFIG.save(deps.storage, &state)?;
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Mint{address} => execute_mint(deps, env, info,address),
        ExecuteMsg::ChangeOwner { address } => execute_chage_owner(deps, info, address),
        ExecuteMsg::AddCollection { members, nft_address,collection}  => execute_add_collection(deps, info,members, nft_address,collection),
        ExecuteMsg::UpdateCollection { members, nft_address,collection}  => execute_update_collection(deps, info,members, nft_address,collection),
        ExecuteMsg::SetMintFlag { address, time } => execute_set_flag(deps, info, address,time),
        ExecuteMsg::AddFreeMinter { address, minters } => execute_free_minter(deps, info, address,minters),
        ExecuteMsg::SwitchSaleType { address, public_mint, private_mint, free_mint } => execute_switch_type(deps, info, address,public_mint,private_mint,free_mint),
        ExecuteMsg::AddWhiteUsers { address, white_users } => execute_add_white_user(deps, info, address,white_users)
    }                                  
}

fn execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address:String
) -> Result<Response, ContractError> {
    //address check
    deps.api.addr_validate(&address)?;
    let sender = info.sender.to_string();

    let collection_info = COLLECTIONINFO.may_load(deps.storage, &address)?;

    if collection_info == None{
        return Err(ContractError::CollectionNotFound { });
    }

    let  collection_info = collection_info.unwrap();

    if collection_info.start_mint_time>env.block.time.seconds(){
        return Err(ContractError::MintNotStarted {  });
    }

    if collection_info.mint_count >= collection_info.total_nft {
        return Err(ContractError::MintEnded {});
    }


    if !collection_info.free_mint  {
        if collection_info.public_mint
            {
                let count = USERINFO.may_load(deps.storage,(&address,&sender))?;
            
                let price = collection_info.public_price;

                let free_minter = FREEMINTER.may_load(deps.storage, (&address,&sender))?; 

                if count == None{
                    USERINFO.save(deps.storage,(&address,&sender), &Uint128::new(1))?;
                }

                else {
                    let count = count.unwrap() + Uint128::new(1);
                    USERINFO.save(deps.storage,(&address,&sender), &count)?;
                    if count > collection_info.max_nft && free_minter == None{
                        return Err(ContractError::MintExceeded {  })
                    }
                }

                if free_minter == None
                    {
                        let amount= info
                            .funds
                            .iter()
                            .find(|c| c.denom == collection_info.denom)
                            .map(|c| Uint128::from(c.amount))
                            .unwrap_or_else(Uint128::zero);
                
                        if amount != price{
                            return Err(ContractError::Notenough {});
                        }
                    }

                let mut check_mint = collection_info.check_mint;

                let count = check_mint.len();
                
                let prng_seed: Vec<u8> = sha_256(base64::encode("entropy").as_bytes()).to_vec();
                let random_seed = new_entropy(&info,&env, prng_seed.as_ref(), prng_seed.as_ref());
                let mut rng = ChaChaRng::from_seed(random_seed);
                let  rand_num = (rng.next_u32() % (count as u32)) as usize ;
                let rand = check_mint[rand_num];
                check_mint.remove(rand_num);
            
                let token_id = [collection_info.name,rand.to_string()].join(".");
            
                COLLECTIONINFO.update(deps.storage, &address,|collection_info|->StdResult<_>{
                    let mut collection_info = collection_info.unwrap();    
                    collection_info.mint_count = collection_info.mint_count+Uint128::new(1);
                    collection_info.check_mint = check_mint;
                    Ok(collection_info)
                })?;

                let admins = ADMININFO.load(deps.storage,&address)?;
                let mut messages:Vec<CosmosMsg> = vec![];
                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: address,
                        msg: to_binary(&Cw721BaseExecuteMsg::Mint(MintMsg {
                            //::<Metadata>
                            token_id: token_id.clone(),
                            owner: sender,
                            token_uri: Some([[collection_info.url,rand.to_string()].join(""),"json".to_string()].join(".")),
                            extension:  Image{
                                image:Some([[collection_info.image_url,rand.to_string()].join(""),"png".to_string()].join("."))
                            }
                        }))?,
                        funds: vec![],
                    }));

                if free_minter == None{
                    for admin in admins {
                        messages.push(CosmosMsg::Bank(BankMsg::Send {
                                to_address: admin.address,
                                amount:vec![Coin{
                                    denom:collection_info.denom.clone(),
                                    amount:admin.portion * price
                                }]
                        }));
                    }
                }
            

                Ok(Response::new()
                    .add_messages(messages)
                )
            }
        else {
                let count = WHITEUSERS.may_load(deps.storage,(&address,&sender))?;
                if count == None{
                    return Err(ContractError::NOTWHITEUSERS {  });
                }
                else {
                    let count = count.unwrap();
                    if count == Uint128::new(0){
                        return Err(ContractError::MintExceeded {  });
                    } 
                    else  {
                        WHITEUSERS.update(deps.storage, (&address,&sender), 
                        |count|->StdResult<_>{
                            let mut count = count.unwrap();
                            count = count - Uint128::new(1);
                            Ok(count)
                      })?; 
                    }
                }
                
                let price = collection_info.private_price;

                let free_minter = FREEMINTER.may_load(deps.storage, (&address,&sender))?; 

                if free_minter == None
                    {
                        let amount= info
                            .funds
                            .iter()
                            .find(|c| c.denom == collection_info.denom)
                            .map(|c| Uint128::from(c.amount))
                            .unwrap_or_else(Uint128::zero);
                
                        if amount != price{
                            return Err(ContractError::Notenough {});
                        }
                    }

                let mut check_mint = collection_info.check_mint;

                let count = check_mint.len();
                
                let prng_seed: Vec<u8> = sha_256(base64::encode("entropy").as_bytes()).to_vec();
                let random_seed = new_entropy(&info,&env, prng_seed.as_ref(), prng_seed.as_ref());
                let mut rng = ChaChaRng::from_seed(random_seed);
                let  rand_num = (rng.next_u32() % (count as u32)) as usize ;
                let rand = check_mint[rand_num];
                check_mint.remove(rand_num);
            
                let token_id = [collection_info.name,rand.to_string()].join(".");
            
                COLLECTIONINFO.update(deps.storage, &address,|collection_info|->StdResult<_>{
                    let mut collection_info = collection_info.unwrap();    
                    collection_info.mint_count = collection_info.mint_count+Uint128::new(1);
                    collection_info.check_mint = check_mint;
                    Ok(collection_info)
                })?;

                let admins = ADMININFO.load(deps.storage,&address)?;
                let mut messages:Vec<CosmosMsg> = vec![];
                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: address,
                        msg: to_binary(&Cw721BaseExecuteMsg::Mint(MintMsg {
                            //::<Metadata>
                            token_id: token_id.clone(),
                            owner: sender,
                            token_uri: Some([[collection_info.url,rand.to_string()].join(""),"json".to_string()].join(".")),
                            extension:  Image{
                                image:Some([[collection_info.image_url,rand.to_string()].join(""),"png".to_string()].join("."))
                            }
                        }))?,
                        funds: vec![],
                    }));

                if free_minter == None{
                    for admin in admins {
                        messages.push(CosmosMsg::Bank(BankMsg::Send {
                                to_address: admin.address,
                                amount:vec![Coin{
                                    denom:collection_info.denom.clone(),
                                    amount:admin.portion * price
                                }]
                        }));
                    }
                }
            

                Ok(Response::new()
                    .add_messages(messages)
                )
        }
    }

    else{
        let count = USERINFO.may_load(deps.storage,(&address,&sender))?;
        let free_minter = FREEMINTER.may_load(deps.storage, (&address,&sender))?; 
    
        if count == None{
            USERINFO.save(deps.storage,(&address,&sender), &Uint128::new(1))?;
        }

        else {
            let count = count.unwrap() + Uint128::new(1);
            USERINFO.save(deps.storage,(&address,&sender), &count)?;
            if count > collection_info.max_nft && free_minter == None{
                return Err(ContractError::MintExceeded {  })
            }
        }

        let mut check_mint = collection_info.check_mint;

        let count = check_mint.len();
        
        let prng_seed: Vec<u8> = sha_256(base64::encode("entropy").as_bytes()).to_vec();
        let random_seed = new_entropy(&info,&env, prng_seed.as_ref(), prng_seed.as_ref());
        let mut rng = ChaChaRng::from_seed(random_seed);
        let  rand_num = (rng.next_u32() % (count as u32)) as usize ;
        let rand = check_mint[rand_num];
        check_mint.remove(rand_num);
    
        let token_id = [collection_info.name,rand.to_string()].join(".");
    
        COLLECTIONINFO.update(deps.storage, &address,|collection_info|->StdResult<_>{
            let mut collection_info = collection_info.unwrap();    
            collection_info.mint_count = collection_info.mint_count+Uint128::new(1);
            collection_info.check_mint = check_mint;
            Ok(collection_info)
        })?;

        Ok(Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: address,
                msg: to_binary(&Cw721BaseExecuteMsg::Mint(MintMsg {
                    //::<Metadata>
                    token_id: token_id.clone(),
                    owner: sender,
                    token_uri: Some([[collection_info.url,rand.to_string()].join(""),"json".to_string()].join(".")),
                    extension:  Image{
                        image:Some([[collection_info.image_url,rand.to_string()].join(""),"png".to_string()].join("."))
                    }
                }))?,
                funds: vec![],
            }))
        )
    }
}


fn execute_chage_owner(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
   let state =CONFIG.load(deps.storage)?;
    if state.owner != info.sender.to_string() {
        return Err(ContractError::Unauthorized {});
    }

    CONFIG.update(deps.storage,
        |mut state|-> StdResult<_>{
            state.owner = address;
            Ok(state)
        }
    )?;

    Ok(Response::default())
}

fn execute_set_flag(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
    time:u64
) -> Result<Response, ContractError> {
   let state =CONFIG.load(deps.storage)?;
    if state.owner != info.sender.to_string() {
        return Err(ContractError::Unauthorized {});
    }
    let collection_info = COLLECTIONINFO.may_load(deps.storage, &address)?;
    if collection_info == None{
        return Err(ContractError::CollectionNotFound {  })
    }
    COLLECTIONINFO.update(deps.storage, &address, 
        |collection_info|->StdResult<_>{
            let mut collection_info = collection_info.unwrap();
            collection_info.start_mint_time = time;
            Ok(collection_info)
        })?;
    Ok(Response::default())
}


fn execute_free_minter(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
    minters:Vec<String>
) -> Result<Response, ContractError> {
   let state =CONFIG.load(deps.storage)?;
    if state.owner != info.sender.to_string() {
        return Err(ContractError::Unauthorized {});
    }
    let collection_info = COLLECTIONINFO.may_load(deps.storage, &address)?;
    if collection_info == None{
        return Err(ContractError::CollectionNotFound {  })
    }
    for minter  in minters {
        let flag  = true;
        FREEMINTER.save(deps.storage, (&address,&minter),&flag)?;
    }
    Ok(Response::default())
}


fn execute_switch_type(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
    public_mint:bool,
    private_mint:bool,
    free_mint:bool
) -> Result<Response, ContractError> {
   let state =CONFIG.load(deps.storage)?;
    if state.owner != info.sender.to_string() {
        return Err(ContractError::Unauthorized {});
    }
    let collection_info = COLLECTIONINFO.may_load(deps.storage, &address)?;
    if collection_info == None{
        return Err(ContractError::CollectionNotFound {  })
    }
    COLLECTIONINFO.update(deps.storage, &address, 
        |collection_info| -> StdResult<_>{
            let mut collection_info = collection_info.unwrap();
                collection_info.public_mint = public_mint;
                collection_info.private_mint = private_mint;
                collection_info.free_mint = free_mint;
            Ok(collection_info)
        })?;
    Ok(Response::default())
}


fn execute_add_white_user(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
    white_users:Vec<WhiteUserInfo>
) -> Result<Response, ContractError> {
   let state =CONFIG.load(deps.storage)?;
    if state.owner != info.sender.to_string() {
        return Err(ContractError::Unauthorized {});
    }
    let collection_info = COLLECTIONINFO.may_load(deps.storage, &address)?;
    if collection_info == None{
        return Err(ContractError::CollectionNotFound {  })
    }
    
    for white_user in white_users{
        WHITEUSERS.save(deps.storage, (&address,&white_user.address), &white_user.count)?;
    }

    Ok(Response::default())
}






fn execute_add_collection(
    deps: DepsMut,
    info: MessageInfo,
    members: Vec<AdminInfo>,
    nft_address:String,
    collection:CollectionInfo
)->Result<Response,ContractError>{

    let state = CONFIG.load(deps.storage)?;

    deps.api.addr_validate(&nft_address)?;

    if info.sender.to_string() != state.owner{
        return Err(ContractError::Unauthorized {});
    }

    let total_count =Uint128::u128(&collection.total_nft) as u32;
    let check_length =  collection.check_mint.len() as u32;

    if total_count != check_length{
        return Err(ContractError::WrongNumber {  })
    }

    
    let mut total = Decimal::zero();
    for admin in members.clone(){
        deps.api.addr_validate(&admin.address)?;
        total = total+admin.portion;
    }

    if total!= Decimal::one(){
        return Err(ContractError::WrongPortion {  })
    }

    ADMININFO.save(deps.storage,&nft_address,&members)?;

    COLLECTIONINFO.save(deps.storage,&nft_address,&CollectionInfo{
       total_nft:collection.total_nft,
       mint_count:Uint128::new(0),
       url:collection.url,
       check_mint:collection.check_mint,
       image_url:collection.image_url,
       price:collection.price,
       denom:collection.denom,
       max_nft:collection.max_nft,
       name:collection.name,
       can_mint:true,
       public_mint:collection.public_mint,
       private_mint:collection.private_mint,
       free_mint:collection.free_mint,
       private_price:collection.private_price,
       public_price:collection.public_price,
       start_mint_time:collection.start_mint_time,
       private_mint_period:collection.private_mint_period,
       public_mint_period:collection.public_mint_period
    })?;
    Ok(Response::default())
}

fn execute_update_collection(
    deps: DepsMut,
    info: MessageInfo,
    members: Vec<AdminInfo>,
    nft_address:String,
    collection:CollectionInfo
)->Result<Response,ContractError>{

    let state = CONFIG.load(deps.storage)?;

    deps.api.addr_validate(&nft_address)?;

    if info.sender.to_string() != state.owner{
        return Err(ContractError::Unauthorized {});
    }

    let collection_info = COLLECTIONINFO.may_load(deps.storage, &nft_address)?;
    if collection_info == None{
        return Err(ContractError::CollectionNotFound {  })
    }
    let collection_info = collection_info.unwrap();
    
    let mut total = Decimal::zero();
    for admin in members.clone(){
        deps.api.addr_validate(&admin.address)?;
        total = total+admin.portion;
    }

    if total!= Decimal::one(){
        return Err(ContractError::WrongPortion {  })
    }


    ADMININFO.save(deps.storage,&nft_address,&members)?;

    COLLECTIONINFO.save(deps.storage,&nft_address,&CollectionInfo{
       total_nft:collection.total_nft,
       mint_count:collection_info.mint_count,
       url:collection.url,
       check_mint:collection_info.check_mint,
       image_url:collection.image_url,
       price:collection.price,
       denom:collection.denom,
       max_nft:collection.max_nft,
       name:collection.name,
       can_mint:true,
       public_mint:collection.public_mint,
       private_mint:collection.private_mint,
       free_mint:collection.free_mint,
       private_price:collection.private_price,
       public_price:collection.public_price,
       start_mint_time:collection.start_mint_time,
       private_mint_period:collection.private_mint_period,
       public_mint_period:collection.public_mint_period
    })?;
    Ok(Response::default())
}




#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetStateInfo {} => to_binary(& query_state_info(deps)?),
        QueryMsg::GetAdminInfo { nft_address }=>to_binary(& query_admin_info(deps,nft_address)?),
        QueryMsg::GetUserInfo {nft_address, address }=>to_binary(& query_user_info(deps,nft_address,address)?),
        QueryMsg::GetCollectionInfo { nft_address,address }=>to_binary(& query_collection_info(deps,nft_address,address)?)
    }
}


pub fn query_state_info(deps:Deps) -> StdResult<State>{
    let state = CONFIG.load(deps.storage)?;
    Ok(state)
}

pub fn query_admin_info(deps:Deps,nft_address:String) -> StdResult<Vec<AdminInfo>>{
   let admin = ADMININFO.load(deps.storage,&nft_address)?;
   Ok(admin)
}

pub fn query_user_info(deps:Deps, nft_address:String,address:String) -> StdResult<Uint128>{
   let user_info = USERINFO.may_load(deps.storage, (&nft_address,&address))?;
   if user_info == None{
    Ok(Uint128::new(0))
   }
   else{
   Ok(user_info.unwrap())
    }
}

pub fn query_collection_info(deps:Deps,nft_address:String,address: String) -> StdResult<CollectionInfo>{
   let mut collection_info = COLLECTIONINFO.load(deps.storage,&nft_address)?;
   let free_minter = FREEMINTER.may_load(deps.storage, (&nft_address,&address))?;
   if free_minter != None{
     collection_info.price = Uint128::new(0)
   }
   else {
    if collection_info.private_mint{
     collection_info.price = collection_info.private_price
    } 
    else if collection_info.public_mint{
        collection_info.price = collection_info.public_price
            } 
        else {
            collection_info.price = Uint128::new(0)
        }
    }
   Ok(collection_info)
}

pub fn new_entropy(info:&MessageInfo,env: &Env, seed: &[u8], entropy: &[u8]) -> [u8; 32] {
    // 16 here represents the lengths in bytes of the block height and time.
    let entropy_len = 16 + info.sender.to_string().len() + entropy.len();
    let mut rng_entropy = Vec::with_capacity(entropy_len);
    rng_entropy.extend_from_slice(&env.block.height.to_be_bytes());
    rng_entropy.extend_from_slice(&info.sender.as_bytes());
    rng_entropy.extend_from_slice(entropy);

    let mut rng = Prng::new(seed, &rng_entropy);

    rng.rand_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{CosmosMsg, from_binary};

    #[test]
    fn buy_token() {
        let mut deps = mock_dependencies();
        let instantiate_msg = InstantiateMsg {
            owner:"creator".to_string(),
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());
        
        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::AddCollection { members: vec![AdminInfo{
            address:"admin1".to_string(),
            portion:Decimal::from_ratio(70 as u128, 100 as u128)
        },
        AdminInfo{
            address:"admin2".to_string(),
             portion:Decimal::from_ratio(30 as u128, 100 as u128)
        }], 
        nft_address: "collection1".to_string(),
        collection: CollectionInfo { 
            total_nft:Uint128::new(10),
            check_mint:vec![1,2,3,4,5,6,7,8,9,10],
            url :"url".to_string(),
            image_url:"imag_url".to_string(),
            price:Uint128::new(0),
            denom : "ujunox".to_string(),
            max_nft:Uint128::new(1),
            mint_count:Uint128::new(0),
            name:"Collection1".to_string(),
            can_mint:true,
            public_mint:true,
            private_mint:false,
            free_mint:false,
            public_price:Uint128::new(20),
            private_price:Uint128::new(10),
            start_mint_time:mock_env().block.time.seconds()-10,
            private_mint_period:50,
            public_mint_period:50
            } 
        };
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();


        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::SetMintFlag { address: "collection1".to_string(), time: mock_env().block.time.seconds() };
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let collection_info = query_collection_info(deps.as_ref(), "collection1".to_string(),"user".to_string()).unwrap();
        assert_eq!(collection_info.price,Uint128::new(20));
       
        let user_info = query_user_info(deps.as_ref(), "collection1".to_string(), "user".to_string()).unwrap();
        assert_eq!(user_info,Uint128::new(0));

        
        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::SwitchSaleType { address: "collection1".to_string(),
             public_mint: true, 
             private_mint: false,
             free_mint: false 
        };
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();


        let info = mock_info("minter1", &[Coin{
            denom:"ujunox".to_string(),
            amount:Uint128::new(20)
        }]);
        let msg = ExecuteMsg::Mint { address: "collection1".to_string() };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let collection_info = query_collection_info(deps.as_ref(), "collection1".to_string(),"user".to_string()).unwrap();
        assert_eq!(collection_info.price,Uint128::new(20));
        
        assert_eq!(res.messages[1].msg,CosmosMsg::Bank(BankMsg::Send {
                to_address: "admin1".to_string(),
                amount:vec![Coin{
                    denom:"ujunox".to_string(),
                    amount:Uint128::new(14)
                }]
        }));
       
        assert_eq!(res.messages[2].msg,CosmosMsg::Bank(BankMsg::Send {
                to_address: "admin2".to_string(),
                amount:vec![Coin{
                    denom:"ujunox".to_string(),
                    amount:Uint128::new(6)
                }]
        }));

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::AddFreeMinter { address: "collection1".to_string(), minters: vec!["minter1".to_string()] };
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();


        let info = mock_info("minter1", &[]);
        let msg = ExecuteMsg::Mint { address: "collection1".to_string() };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.messages.len(),1);


        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::SwitchSaleType { address: "collection1".to_string(),
             public_mint: false, 
             private_mint: true,
             free_mint: false 
        };
       execute(deps.as_mut(), mock_env(), info, msg).unwrap();

       let collection_info = query_collection_info(deps.as_ref(), "collection1".to_string(),"user".to_string()).unwrap();
       assert_eq!(collection_info.price,Uint128::new(10));

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::AddWhiteUsers { address: "collection1".to_string(), white_users: vec![WhiteUserInfo{
            address:"minter2".to_string(),
            count:Uint128::new(1)
        }] };
       execute(deps.as_mut(), mock_env(), info, msg).unwrap();



        let info = mock_info("minter2", &[Coin{
            denom:"ujunox".to_string(),
            amount:Uint128::new(10)
        }]);
        let msg = ExecuteMsg::Mint { address: "collection1".to_string() };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
         
        assert_eq!(res.messages[1].msg,CosmosMsg::Bank(BankMsg::Send {
                to_address: "admin1".to_string(),
                amount:vec![Coin{
                    denom:"ujunox".to_string(),
                    amount:Uint128::new(7)
                }]
        }));
       
        assert_eq!(res.messages[2].msg,CosmosMsg::Bank(BankMsg::Send {
                to_address: "admin2".to_string(),
                amount:vec![Coin{
                    denom:"ujunox".to_string(),
                    amount:Uint128::new(3)
                }]
        }));

       

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::SwitchSaleType { address: "collection1".to_string(),
             public_mint: false, 
             private_mint: false,
             free_mint: true 
        };
       execute(deps.as_mut(), mock_env(), info, msg).unwrap();

         let collection_info = query_collection_info(deps.as_ref(), "collection1".to_string(),"user".to_string()).unwrap();
        assert_eq!(collection_info.price,Uint128::new(0));

        let info = mock_info("minter3", &[]);
        let msg = ExecuteMsg::Mint { address: "collection1".to_string() };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.messages.len(),1);

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::AddFreeMinter { address: "collection1".to_string(), minters: vec!["minter3".to_string()] };
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();


        let info = mock_info("minter3", &[]);
        let msg = ExecuteMsg::Mint { address: "collection1".to_string() };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.messages.len(),1);

        let collection_info = query_collection_info(deps.as_ref(), "collection1".to_string(),"user".to_string()).unwrap();
        assert_eq!(collection_info.check_mint.len(),5);
        

    }

}
