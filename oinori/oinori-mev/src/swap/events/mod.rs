use crate::swap::anime::event::AnimeEvent;
use crate::swap::apto_swap::event::AptoSwapEvent;
use crate::swap::aux::event::AuxEvent;
use crate::swap::cetus::event::CetusEvent;
use crate::swap::liquid::event::LiquidEvent;
use crate::swap::obric::event::ObricEvent;
use crate::swap::pancake::event::PancakeEvent;
use anyhow::Result;
use aptos_api_types::Event;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub enum SwapEvent {
    LiquidEvent(LiquidEvent),
    AuxEvent(AuxEvent),
    AnimeEvent(AnimeEvent),
    PancakeEvent(PancakeEvent),
    CetusEvent(CetusEvent),
    ObricEvent(ObricEvent),
    AptoSwapEvent(AptoSwapEvent),
}

impl SwapEvent {
    pub fn parse(e: &Event) -> Result<Option<Self>> {
        if let Some(liquid_event) = LiquidEvent::parse(e)? {
            return Ok(Some(SwapEvent::LiquidEvent(liquid_event)));
        }

        if let Some(aux_event) = AuxEvent::parse(e)? {
            return Ok(Some(SwapEvent::AuxEvent(aux_event)));
        }

        if let Some(anime_event) = AnimeEvent::parse(e)? {
            return Ok(Some(SwapEvent::AnimeEvent(anime_event)));
        }

        if let Some(pancake_event) = PancakeEvent::parse(e)? {
            return Ok(Some(SwapEvent::PancakeEvent(pancake_event)));
        }

        if let Some(cetus_event) = CetusEvent::parse(e)? {
            return Ok(Some(SwapEvent::CetusEvent(cetus_event)));
        }

        if let Some(obric_event) = ObricEvent::parse(e)? {
            return Ok(Some(SwapEvent::ObricEvent(obric_event)));
        }

        if let Some(aptoswap_event) = AptoSwapEvent::parse(e)? {
            return Ok(Some(SwapEvent::AptoSwapEvent(aptoswap_event)));
        }

        Ok(None)
    }

    pub fn parse_vec(events: &Vec<Event>) -> Result<Vec<Self>> {
        let mut result = vec![];

        for each_event in events {
            if let Some(event) = Self::parse(each_event)? {
                result.push(event)
            }
        }
        Ok(result)
    }
}
