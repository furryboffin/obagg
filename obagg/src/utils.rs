pub fn stream_function() -> Arc<Mutex<Pin<Box<dyn Stream<Item = Summary> + Send + Sync + 'static>>>> {
    // creating infinite stream with requested message
    Arc::new(Mutex::new(Box::pin(stream! {
        let entries = vec!(
            Summary {
                spread: 10.0,
                bids: vec![
                    Level {
                        exchange: "binance".to_string(),
                        price: 100.1,
                        amount: 500.1,
                    }
                ],
                asks: vec![
                    Level {
                        exchange: "binance".to_string(),
                        price: 100.1,
                        amount: 500.1,
                    }
                ],
            },
            Summary {
                spread: 10.0,
                bids: vec![
                    Level {
                        exchange: "binance".to_string(),
                        price: 100.1,
                        amount: 500.1,
                    }
                ],
                asks: vec![
                    Level {
                        exchange: "binance".to_string(),
                        price: 100.1,
                        amount: 500.1,
                    }
                ],
            }
        );

        for entry in entries {
            yield entry;
        }
    })))
}

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
