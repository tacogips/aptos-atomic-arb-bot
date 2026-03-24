pub mod amm;
pub mod events;
pub mod resources;
pub mod router;
pub mod specifics;

pub use amm::*;
use anyhow::Result;
use aptos_api_types::{MoveResource, MoveStructTag, MoveStructValue, WriteSetChange};
pub use resources::*;
pub use router::*;
pub use specifics::*;

pub trait ConvertResource: Sized {
    fn from_resources(resources: &Vec<MoveResource>) -> Vec<Self> {
        resources
            .iter()
            .filter_map(|resource| match Self::from_resource(resource) {
                Ok(resource) => resource,
                Err(_e) => None,
            })
            .collect()
    }

    fn from_resource(e: &MoveResource) -> Result<Option<Self>> {
        let type_tag = &e.typ;
        let move_struct_value = &e.data;
        Self::from_tag_and_data(type_tag, move_struct_value)
    }

    fn from_change(e: &WriteSetChange) -> Result<Option<Self>>;
    fn from_tag_and_data(type_tag: &MoveStructTag, data: &MoveStructValue) -> Result<Option<Self>>;

    fn into_coin_pair(self) -> Result<Option<ReservedCoinPair>>;
}
