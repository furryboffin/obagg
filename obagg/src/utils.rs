use rust_decimal::Decimal;

use crate::{
    config,
    definitions::{self, Orderbook},
    orderbook::Level,
};

pub fn key_from_value(v: &serde_json::Value) -> Decimal {
    v[0].as_str()
        .unwrap()
        .to_string()
        .parse::<Decimal>()
        .unwrap()
}

pub fn map_key(k: (Decimal, Level), conf: &config::Server, is_bids: bool) -> (Decimal, Level) {
    if (is_bids && conf.identical_level_order) || (!is_bids && !conf.identical_level_order) {
        (
            k.0 * definitions::hash_key_offset()
                + k.1.amount.to_string().parse::<Decimal>().unwrap(),
            k.1,
        )
    } else {
        (
            k.0 * definitions::hash_key_offset()
                - k.1.amount.to_string().parse::<Decimal>().unwrap(),
            k.1,
        )
    }
}

pub fn level_from_value(v: &serde_json::Value, exchange: &str) -> Level {
    Level {
        amount: amount_from_value(v),
        exchange: exchange.into(),
        price: v[0].as_str().unwrap().to_string().parse::<f64>().unwrap(),
    }
}

pub fn handle_update_message(
    v: &Vec<serde_json::Value>,
    ob: &mut Orderbook,
    d: usize,
    is_bids: bool,
) {
    let mut bids_iter = v.into_iter().take(d);
    if is_bids {
        let b = &mut ob.bids;
        let other_b = &mut ob.asks;
        let other_b_clone = other_b.clone();
        let mut asks_iter = other_b_clone.iter();
        while let Some(bid) = bids_iter.next() {
            let key = key_from_value(bid);
            let level = level_from_value(bid, "binance");
            b.remove(&key);
            if amount_from_value(bid) > 0.0 {
                b.insert(key, level);

                // check if a bid overlaps old ask levels

                while let Some((k, _)) = asks_iter.next() {
                    if k <= &key {
                        other_b.remove(&k);
                    } else {
                        break;
                    }
                }
            }
        }
    } else {
        let b = &mut ob.asks;
        let other_b = &mut ob.bids;
        let other_b_clone = other_b.clone();
        let mut asks_iter = other_b_clone.iter().rev();
        while let Some(bid) = bids_iter.next() {
            let key = key_from_value(bid);
            let level = level_from_value(bid, "binance");
            b.remove(&key);
            if amount_from_value(bid) > 0.0 {
                b.insert(key, level);

                // check if a bid overlaps old ask levels

                while let Some((k, _)) = asks_iter.next() {
                    if k >= &key {
                        other_b.remove(&k);
                    } else {
                        break;
                    }
                }
            }
        }
    }
}

pub fn amount_from_value(v: &serde_json::Value) -> f64 {
    v[1].as_str().unwrap().to_string().parse::<f64>().unwrap()
}

// pub fn stream_function() -> Arc<Mutex<Pin<Box<dyn Stream<Item = Summary> + Send + Sync + 'static>>>>
// {
//     // creating infinite stream with requested message
//     Arc::new(Mutex::new(Box::pin(stream! {
//         let entries = vec!(
//             Summary {
//                 spread: 10.0,
//                 bids: vec![
//                     Level {
//                         exchange: "binance".to_string(),
//                         price: 100.1,
//                         amount: 500.1,
//                     }
//                 ],
//                 asks: vec![
//                     Level {
//                         exchange: "binance".to_string(),
//                         price: 100.1,
//                         amount: 500.1,
//                     }
//                 ],
//             },
//             Summary {
//                 spread: 10.0,
//                 bids: vec![
//                     Level {
//                         exchange: "binance".to_string(),
//                         price: 100.1,
//                         amount: 500.1,
//                     }
//                 ],
//                 asks: vec![
//                     Level {
//                         exchange: "binance".to_string(),
//                         price: 100.1,
//                         amount: 500.1,
//                     }
//                 ],
//             }
//         );

//         for entry in entries {
//             yield entry;
//         }
//     })))
// }

// let repeat = std::iter::repeat(Summary {
//     spread: 10.0,
//     bids: vec![
//         Level {
//             exchange: "binance".to_string(),
//             price: 100.1,
//             amount: 500.1,
//         }
//     ],
//     asks: vec![
//         Level {
//             exchange: "binance".to_string(),
//             price: 100.1,
//             amount, 500.1,
//         }
//     ]
// })
