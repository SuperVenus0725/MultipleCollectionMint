use cosmwasm_std::{ Uint128};
use schemars::{JsonSchema};
use serde::{Deserialize, Serialize};

use crate::state::{AdminInfo, CollectionInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
   pub owner:String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Mint{address:String},
    ChangeOwner {address:String},
    AddCollection{members:Vec<AdminInfo>,nft_address:String,collection:CollectionInfo},
    UpdateCollection{members:Vec<AdminInfo>,nft_address:String,collection:CollectionInfo},
    SetMintFlag{address:String,time:u64},
    AddFreeMinter{address:String,minters:Vec<String>},
    SwitchSaleType{address:String,public_mint:bool,private_mint:bool,free_mint:bool},
    AddWhiteUsers{address:String,white_users:Vec<WhiteUserInfo>}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
      GetStateInfo{},
      GetCollectionInfo{nft_address:String,address:String},
      GetUserInfo{nft_address:String,address:String},
      GetAdminInfo{nft_address:String}
    }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Image {   
    pub image: Option<String>,
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct WhiteUserInfo {   
    pub address: String,
    pub count:Uint128
}
