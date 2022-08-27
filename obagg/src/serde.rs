use serde::{self, de, Deserialize, Deserializer};
use std::str::FromStr;

pub fn u32_from_str<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    u32::from_str(&s).map_err(de::Error::custom)
}

pub fn u64_from_str<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    u64::from_str(&s).map_err(de::Error::custom)
}

// pub fn f64_from_str<'de, D>(deserializer: D) -> Result<f64, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let s = String::deserialize(deserializer)?;
//     f64::from_str(&s).map_err(de::Error::custom)
// }

pub fn tf64_from_str<'de, D>(deserializer: D) -> Result<(f64, f64), D::Error>
where
    D: Deserializer<'de>,
{
    let v: Vec<String> = Vec::deserialize(deserializer)?;
    let level = f64::from_str(&v[0]).map_err(de::Error::custom);
    let amount = f64::from_str(&v[1]).map_err(de::Error::custom);
    match (level, amount) {
        (Ok(l), Ok(a)) => Ok((l, a)),
        (Err(e), Ok(_)) => Err(e),
        (Ok(_), Err(e)) => Err(e),
        (Err(_), Err(_)) => Err(serde::de::Error::custom(
            "Failed to deserialize both price and amount from level!",
        )),
    }
}
