use crate::coins::CoinId;
use crate::hasher::*;
use crate::read_db;
use crate::swap::specifics::anime::resource::AnimeResource;
use crate::swap::specifics::apto_swap::resource::AptoSwapResource;
use crate::swap::specifics::aux::resource::AuxResource;
use crate::swap::specifics::cetus::resource::CetusResource;
use crate::swap::specifics::liquid::resource::LiquidResource;
use crate::swap::specifics::obric::resource::ObricResource;
use crate::swap::specifics::pancake::resource::PancakeResource;
use crate::swap::specifics::thala::resource::ThalaResource;
use crate::swap::SwapContract;
use crate::swap::{ConvertResource, ReservedCoinPair};
use anyhow::Result;
use aptos_api_types::{MoveConverter, MoveResource, WriteSetChange};

use aptos_storage_interface::DbReader;
use serde::Serialize;
use std::sync::Arc;

use aptos_storage_interface::state_view::DbStateView;

#[derive(Debug, Serialize, Clone)]
pub enum SwapResource {
    LiquidResource(LiquidResource),
    AuxResource(AuxResource),
    AnimeResource(AnimeResource),
    PancakeResource(PancakeResource),
    CetusResource(CetusResource),
    ObricResource(ObricResource),
    AptoSwapResource(AptoSwapResource),
    ThalaResource(ThalaResource),
}

impl SwapResource {
    pub fn swap_contract(&self) -> SwapContract {
        match self {
            Self::LiquidResource(_) => SwapContract::Liquid,
            Self::AuxResource(_) => SwapContract::Aux,
            Self::AnimeResource(_) => SwapContract::Anime,
            Self::PancakeResource(_) => SwapContract::Pancake,
            Self::CetusResource(_) => SwapContract::Cetus,
            Self::ObricResource(_) => SwapContract::Obric,
            Self::AptoSwapResource(_) => SwapContract::AptoSwap,
            Self::ThalaResource(_) => SwapContract::Thala,
        }
    }

    pub fn into_reserverd_coin_pair(self) -> Result<Option<ReservedCoinPair>> {
        let res = match self {
            Self::LiquidResource(resource) => resource.into_coin_pair()?,
            Self::AuxResource(resource) => resource.into_coin_pair()?,
            Self::AnimeResource(resource) => resource.into_coin_pair()?,
            Self::PancakeResource(resource) => resource.into_coin_pair()?,
            Self::CetusResource(resource) => resource.into_coin_pair()?,
            Self::ObricResource(resource) => resource.into_coin_pair()?,
            Self::AptoSwapResource(resource) => resource.into_coin_pair()?,
            Self::ThalaResource(resource) => resource.into_coin_pair()?,
        };
        Ok(res)
    }

    pub fn into_coin_ids(self) -> Result<Option<(CoinId, CoinId)>> {
        let coin_pair = self.into_reserverd_coin_pair()?;
        match coin_pair {
            None => Ok(None),
            Some(coin_pair) => {
                let (lhs, rhs) = coin_pair.coin_ids();
                Ok(Some((lhs.clone(), rhs.clone())))
            },
        }
    }

    pub fn from_change(
        e: &WriteSetChange,
        contract_filter: &HashSet<SwapContract>,
    ) -> Result<Option<Self>> {
        if let Some(liquid_write_resource) = LiquidResource::from_change(e)? {
            if contract_filter.contains(&SwapContract::Liquid) {
                return Ok(Some(SwapResource::LiquidResource(liquid_write_resource)));
            } else {
                return Ok(None);
            }
        }

        if let Some(aux_write_resource) = AuxResource::from_change(e)? {
            if contract_filter.contains(&SwapContract::Aux) {
                return Ok(Some(SwapResource::AuxResource(aux_write_resource)));
            } else {
                return Ok(None);
            }
        }

        if let Some(anime_write_resource) = AnimeResource::from_change(e)? {
            if contract_filter.contains(&SwapContract::Anime) {
                return Ok(Some(SwapResource::AnimeResource(anime_write_resource)));
            } else {
                return Ok(None);
            }
        }

        if let Some(pancake_write_resource) = PancakeResource::from_change(e)? {
            if contract_filter.contains(&SwapContract::Pancake) {
                return Ok(Some(SwapResource::PancakeResource(pancake_write_resource)));
            } else {
                return Ok(None);
            }
        }

        if let Some(cetus_write_resource) = CetusResource::from_change(e)? {
            if contract_filter.contains(&SwapContract::Cetus) {
                return Ok(Some(SwapResource::CetusResource(cetus_write_resource)));
            } else {
                return Ok(None);
            }
        }

        if let Some(obric_write_resource) = ObricResource::from_change(e)? {
            if contract_filter.contains(&SwapContract::Obric) {
                return Ok(Some(SwapResource::ObricResource(obric_write_resource)));
            } else {
                return Ok(None);
            }
        }

        if let Some(aptoswap_write_resource) = AptoSwapResource::from_change(e)? {
            if contract_filter.contains(&SwapContract::AptoSwap) {
                return Ok(Some(SwapResource::AptoSwapResource(
                    aptoswap_write_resource,
                )));
            } else {
                return Ok(None);
            }
        }

        if let Some(thala_write_resource) = ThalaResource::from_change(e)? {
            if contract_filter.contains(&SwapContract::Thala) {
                return Ok(Some(SwapResource::ThalaResource(thala_write_resource)));
            } else {
                return Ok(None);
            }
        }

        Ok(None)
    }

    pub fn from_changes(
        changes: &Vec<WriteSetChange>,
        contract_filter: &HashSet<SwapContract>,
    ) -> Result<Vec<Self>> {
        let mut result = Vec::with_capacity(changes.len());

        for each_change in changes {
            if let Some(change) = Self::from_change(each_change, contract_filter)? {
                result.push(change)
            }
        }
        Ok(result)
    }

    pub fn from_resource(e: &MoveResource) -> Result<Option<Self>> {
        if let Some(liquid_write_resource) = LiquidResource::from_resource(e)? {
            return Ok(Some(SwapResource::LiquidResource(liquid_write_resource)));
        }

        if let Some(aux_write_resource) = AuxResource::from_resource(e)? {
            return Ok(Some(SwapResource::AuxResource(aux_write_resource)));
        }

        if let Some(anime_write_resource) = AnimeResource::from_resource(e)? {
            return Ok(Some(SwapResource::AnimeResource(anime_write_resource)));
        }

        if let Some(pancake_write_resource) = PancakeResource::from_resource(e)? {
            return Ok(Some(SwapResource::PancakeResource(pancake_write_resource)));
        }

        if let Some(cetus_write_resource) = CetusResource::from_resource(e)? {
            return Ok(Some(SwapResource::CetusResource(cetus_write_resource)));
        }

        if let Some(obric_write_resource) = ObricResource::from_resource(e)? {
            return Ok(Some(SwapResource::ObricResource(obric_write_resource)));
        }

        if let Some(aptoswap_write_resource) = AptoSwapResource::from_resource(e)? {
            return Ok(Some(SwapResource::AptoSwapResource(
                aptoswap_write_resource,
            )));
        }

        if let Some(thala_write_resource) = ThalaResource::from_resource(e)? {
            return Ok(Some(SwapResource::ThalaResource(thala_write_resource)));
        }

        Ok(None)
    }

    pub fn from_resources(resources: &Vec<MoveResource>) -> Result<Vec<Self>> {
        let mut result = vec![];

        for each in resources {
            if let Some(resource) = Self::from_resource(each)? {
                result.push(resource)
            }
        }
        Ok(result)
    }

    pub fn load_resource(
        target_swaps: Vec<SwapContract>,
        db_reader: Arc<dyn DbReader>,
        target_version: Option<u64>,
        move_converter: &MoveConverter<DbStateView>,
    ) -> Result<Vec<Self>> {
        let mut result = vec![];
        for each in target_swaps {
            let resources = read_db::get_resources_of_account(
                db_reader.clone(),
                each.pool_address(),
                target_version,
                move_converter,
            )?;
            let mut resources = Self::from_resources(&resources.resources)?;
            result.append(&mut resources);
        }
        Ok(result)
    }
}
