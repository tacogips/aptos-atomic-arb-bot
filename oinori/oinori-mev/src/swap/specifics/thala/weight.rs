use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

// weight canbe 5 - 95

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThalaPoolWeight {
    pub w1: ThalaWeight,
    pub w2: ThalaWeight,
    pub w3: Option<ThalaWeight>,
    pub w4: Option<ThalaWeight>,
}

impl ThalaPoolWeight {
    pub fn from_values(w1: u64, w2: u64, w3: u64, w4: u64) -> Result<Self> {
        Ok(Self {
            w1: ThalaWeight::from_value(w1)?.ok_or(anyhow!("w1 must not be null"))?,
            w2: ThalaWeight::from_value(w2)?.ok_or(anyhow!("w2 must not be null"))?,
            w3: ThalaWeight::from_value(w3)?,
            w4: ThalaWeight::from_value(w4)?,
        })
    }

    pub fn unpack_encoded_weights(&self) -> (u64, u64, u64, u64) {
        let encoded_w1 = EncodedWeight::encode(Some(self.w1));
        let encoded_w2 = EncodedWeight::encode(Some(self.w2));
        let encoded_w3 = EncodedWeight::encode(self.w3);
        let encoded_w4 = EncodedWeight::encode(self.w4);

        (encoded_w1.0, encoded_w2.0, encoded_w3.0, encoded_w4.0)
    }

    pub fn from_encoded_weights(w1: u64, w2: u64, w3: u64, w4: u64) -> Result<Option<Self>> {
        let decoded_w1 = EncodedWeight::decode(w1)?;
        match decoded_w1 {
            None => Ok(None),
            Some(decoded_w1) => {
                let decoded_w2 =
                    EncodedWeight::decode(w2)?.ok_or(anyhow!("w2 must not be null {}", w2))?;
                let decoded_w3 = EncodedWeight::decode(w3)?;
                let decoded_w4 = EncodedWeight::decode(w4)?;

                Ok(Some(Self {
                    w1: decoded_w1,
                    w2: decoded_w2,
                    w3: decoded_w3,
                    w4: decoded_w4,
                }))
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct ThalaWeight(pub u64);
impl ThalaWeight {
    pub fn from_str(s: &str) -> Result<Self> {
        let v = match s {
            "Weight_5" => 5,
            "Weight_10" => 10,
            "Weight_15" => 15,
            "Weight_20" => 20,
            "Weight_25" => 25,
            "Weight_30" => 30,
            "Weight_35" => 35,
            "Weight_40" => 40,
            "Weight_45" => 45,
            "Weight_50" => 50,
            "Weight_55" => 55,
            "Weight_60" => 60,
            "Weight_65" => 65,
            "Weight_70" => 70,
            "Weight_75" => 75,
            "Weight_80" => 80,
            "Weight_85" => 85,
            "Weight_90" => 90,
            "Weight_95" => 95,
            other => return Err(anyhow!("Unsupported weight {:?}", other)),
        };
        Ok(Self(v))
    }
    fn from_value(v: u64) -> Result<Option<ThalaWeight>> {
        match v {
            0 => Ok(None),
            5 | 10 | 15 | 20 | 25 | 30 | 35 | 40 | 45 | 50 | 55 | 60 | 65 | 70 | 75 | 80 | 85
            | 90 | 95 => Ok(Some(ThalaWeight(v))),

            other => Err(anyhow!("Unsupported weight {:?}", other)),
        }
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug)]
pub struct EncodedWeight(pub u64);
impl EncodedWeight {
    fn encode(v: Option<ThalaWeight>) -> Self {
        match v {
            None => Self(0),
            Some(v) => Self(v.0 / 5), // value:range 1 - 19
        }
    }

    fn decode(v: u64) -> Result<Option<ThalaWeight>> {
        let v = v * 5;
        match v {
            0 => Ok(None),
            5 | 10 | 15 | 20 | 25 | 30 | 35 | 40 | 45 | 50 | 55 | 60 | 65 | 70 | 75 | 80 | 85
            | 90 | 95 => Ok(Some(ThalaWeight(v))),

            other => Err(anyhow!("Unsupported weight {:?}", other)),
        }
    }
}
