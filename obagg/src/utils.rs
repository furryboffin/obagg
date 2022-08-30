// use std::error::Error;
// use log::debug;
use rust_decimal::Decimal;
// use tokio_tungstenite::tungstenite::Message;

use crate::{
    config,
    definitions::{Orderbook, OrderbookLevel},
    orderbook::Level,
};

pub fn hash_key_offset() -> Decimal {
    Decimal::new(10000000000000000, 0)
}

pub fn map_key(k: (Decimal, Level), conf: &config::Server, is_bids: bool) -> (Decimal, Level) {
    if (is_bids && conf.identical_level_order) || (!is_bids && !conf.identical_level_order) {
        (
            k.0 * hash_key_offset() + k.1.amount.to_string().parse::<Decimal>().unwrap(),
            k.1,
        )
    } else {
        (
            k.0 * hash_key_offset() - k.1.amount.to_string().parse::<Decimal>().unwrap(),
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

// pub fn handle_message(msg: Message) -> Result<String, Box<dyn Error + Send + Sync>> {
//     match msg {
//         Message::Text(s) => Ok(s),
//         Message::Close(c) => {
//             debug!("Message::Close received : {}", c.expect("Close Frame was None!"));
//             Err();
//         },
//         Message::Binary(b) => {
//             debug!("Message::Binary received : length = {}", b.len());
//             return;
//         },
//         Message::Frame(f) => {
//             debug!("Message::Frame received : {}", f);
//             return;
//         },
//         Message::Ping(p) => {
//             debug!("Message::Ping received : length = {}", p.len());
//             return;
//         },
//         Message::Pong(p) => {
//             debug!("Message::Pong received : length = {}", p.len());
//             return;
//         },
//     }
// }

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

#[cfg(test)]
mod tests {
    use crate::config;
    use crate::definitions::Orderbook;
    use crate::orderbook::Level;
    use rust_decimal::Decimal;
    use std::collections::BTreeMap;

    #[test]
    fn map_key() {
        let mut orderbook = Orderbook::new();
        let binance_bid_level = Level {
            amount: 10.10,
            exchange: "binance".into(),
            price: 100.222,
        };
        let bitstamp_bid_level = Level {
            amount: 20.20,
            exchange: "bitstamp".into(),
            price: 100.222,
        };
        let binance_ask_level = Level {
            amount: 10.10,
            exchange: "binance".into(),
            price: 100.333,
        };
        let bitstamp_ask_level = Level {
            amount: 20.20,
            exchange: "bitstamp".into(),
            price: 100.333,
        };
        orderbook
            .bids
            .insert(Decimal::new(100222, 3), binance_bid_level);
        orderbook
            .asks
            .insert(Decimal::new(100222, 3), binance_ask_level);
        orderbook
            .bids
            .insert(Decimal::new(100333, 3), bitstamp_bid_level);
        orderbook
            .asks
            .insert(Decimal::new(100333, 3), bitstamp_ask_level);
        let mut conf: config::Server = config::read_config();

        // explicitly set the identical level order for testing.
        conf.identical_level_order = true;

        // now map the keys to add sub ordering.
        let bids: BTreeMap<Decimal, Level> = orderbook
            .bids
            .into_iter()
            .map(|k| super::map_key(k, &conf, true))
            .collect();
        let asks: BTreeMap<Decimal, Level> = orderbook
            .asks
            .into_iter()
            .map(|k| super::map_key(k, &conf, false))
            .collect();

        // check that the sub ordering is now correct
        assert!(bids.values().collect::<Vec<&Level>>()[0].exchange == String::from("binance"));
        assert!(asks.values().collect::<Vec<&Level>>()[0].exchange == String::from("binance"));

        assert!(bids.values().collect::<Vec<&Level>>()[0].amount == f64::from(10.10));
        assert!(asks.values().collect::<Vec<&Level>>()[0].amount == f64::from(10.10));
    }
}
