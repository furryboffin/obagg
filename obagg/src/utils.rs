use rust_decimal::Decimal;

use crate::{
    config,
    definitions::{self, Orderbook, OrderbookLevel},
    orderbook::Level,
};

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

pub fn handle_update_message(
    v: &Vec<OrderbookLevel>,
    ob: &mut Orderbook,
    d: usize,
    is_bids: bool,
    exchange: &str,
) {
    let mut iter = v.iter().take(d);
    if is_bids {
        let b = &mut ob.bids;
        let other_b = &mut ob.asks;
        let other_b_clone = other_b.clone();
        let mut other_iter = other_b_clone.iter();
        while let Some(l) = iter.next() {
            let key = l.get_price();
            b.remove(&key);
            if l.get_amount() > 0.0 {
                b.insert(key, l.get_level(exchange.into()));

                // check if a bid overlaps old ask levels
                while let Some((k, _)) = other_iter.next() {
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
        let mut other_iter = other_b_clone.iter().rev();
        while let Some(l) = iter.next() {
            let key = l.get_price();
            b.remove(&key);
            if l.get_amount() > 0.0 {
                b.insert(key, l.get_level(exchange.into()));

                // check if a bid overlaps old ask levels
                while let Some((k, _)) = other_iter.next() {
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
