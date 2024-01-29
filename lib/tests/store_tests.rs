// Module dedicated to testing the unitary behavior of the store actor

#[cfg(test)]
mod tests {
    use actix::prelude::*;
    use lib::errors::Errors;
    use lib::store::{LocalProductOrder, ProductStock, ReserveProduct, Store, _GetStock};
    use std::collections::HashMap;
    const VOLUME_SIZE: usize = 10000;

    #[actix_rt::test]
    async fn test_store_actor() {
        let mut store = Store {
            stock: HashMap::new(),
            reserve_sender: tokio::sync::mpsc::channel(1).0,
            active_ecoms: HashMap::new(),
            connection: false,
            leader: 0,
        };

        let product_stock = ProductStock {
            available_quantity: 10,
            reserved_quantity: 0,
        };

        store.stock.insert("product1".to_string(), product_stock);

        let order = LocalProductOrder {
            product: "product1".to_string(),
            quantity: 5,
        };

        let addr = store.start();
        let res = match addr.send(order).await {
            Ok(Ok(res)) => res,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };
        assert_eq!(res, ());

        let product_stock = match addr.send(_GetStock {}).await {
            Ok(Ok(stock)) => stock,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };
        let product_stock = match product_stock.get("product1") {
            Some(stock) => stock,
            None => {
                assert_eq!(true, false);
                return;
            }
        };

        assert_eq!(product_stock.available_quantity, 5);
        assert_eq!(product_stock.reserved_quantity, 0);
    }

    #[actix_rt::test]
    async fn test_store_actor_not_enough_stock() {
        let mut store = Store {
            stock: HashMap::new(),
            reserve_sender: tokio::sync::mpsc::channel(1).0,
            active_ecoms: HashMap::new(),
            connection: false,
            leader: 0,
        };

        let product_stock = ProductStock {
            available_quantity: 10,
            reserved_quantity: 0,
        };

        store.stock.insert("product1".to_string(), product_stock);

        let order = LocalProductOrder {
            product: "product1".to_string(),
            quantity: 15,
        };

        let addr = store.start();
        let res = match addr.send(order).await {
            Ok(Err(res)) => res,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };

        assert_eq!(res, Errors::NotEnoughStockError);

        let product_stock = match addr.send(_GetStock {}).await {
            Ok(Ok(stock)) => stock,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };
        let product_stock = match product_stock.get("product1") {
            Some(stock) => stock,
            None => {
                assert_eq!(true, false);
                return;
            }
        };

        assert_eq!(product_stock.available_quantity, 10);
        assert_eq!(product_stock.reserved_quantity, 0);
    }

    #[actix_rt::test]
    async fn test_store_actor_product_not_found() {
        let mut store = Store {
            stock: HashMap::new(),
            reserve_sender: tokio::sync::mpsc::channel(1).0,
            active_ecoms: HashMap::new(),
            connection: false,
            leader: 0,
        };

        let product_stock = ProductStock {
            available_quantity: 10,
            reserved_quantity: 0,
        };

        store.stock.insert("product1".to_string(), product_stock);

        let order = LocalProductOrder {
            product: "product2".to_string(),
            quantity: 5,
        };

        let addr = store.start();
        let res = match addr.send(order).await {
            Ok(Err(res)) => res,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };

        assert_eq!(res, Errors::ProductNotFoundError);

        let product_stock = match addr.send(_GetStock {}).await {
            Ok(Ok(stock)) => stock,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };
        let product_stock = match product_stock.get("product1") {
            Some(stock) => stock,
            None => {
                assert_eq!(true, false);
                return;
            }
        };

        assert_eq!(product_stock.available_quantity, 10);
        assert_eq!(product_stock.reserved_quantity, 0);
    }

    #[actix_rt::test]
    async fn test_store_actor_reserve() {
        let mut store = Store {
            stock: HashMap::new(),
            reserve_sender: tokio::sync::mpsc::channel(1).0,
            active_ecoms: HashMap::new(),
            connection: false,
            leader: 0,
        };

        let product_stock = ProductStock {
            available_quantity: 10,
            reserved_quantity: 0,
        };

        store.stock.insert("product1".to_string(), product_stock);

        let order = LocalProductOrder {
            product: "product1".to_string(),
            quantity: 5,
        };

        let addr = store.start();
        let res = addr.send(order).await;

        assert!(res.is_ok());

        let product_stock = match addr.send(_GetStock {}).await {
            Ok(Ok(stock)) => stock,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };
        let product_stock = match product_stock.get("product1") {
            Some(stock) => stock,
            None => {
                assert_eq!(true, false);
                return;
            }
        };

        assert_eq!(product_stock.available_quantity, 5);
        assert_eq!(product_stock.reserved_quantity, 0);

        let res = addr
            .send(ReserveProduct {
                product: "product1".to_string(),
                quantity: 5,
                time_limit: 1,
            })
            .await;

        assert!(res.is_ok());

        let product_stock = match addr.send(_GetStock {}).await {
            Ok(Ok(stock)) => stock,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };
        let product_stock = match product_stock.get("product1") {
            Some(stock) => stock,
            None => {
                assert_eq!(true, false);
                return;
            }
        };

        assert_eq!(product_stock.available_quantity, 5);
        assert_eq!(product_stock.reserved_quantity, 5);
    }

    #[actix_rt::test]
    async fn test_store_actor_reserve_not_enough_stock() {
        let mut store = Store {
            stock: HashMap::new(),
            reserve_sender: tokio::sync::mpsc::channel(1).0,
            active_ecoms: HashMap::new(),
            connection: false,
            leader: 0,
        };

        let product_stock = ProductStock {
            available_quantity: 10,
            reserved_quantity: 0,
        };

        store.stock.insert("product1".to_string(), product_stock);

        let order = LocalProductOrder {
            product: "product1".to_string(),
            quantity: 5,
        };

        let addr = store.start();
        let res = addr.send(order).await;

        assert!(res.is_ok());

        let product_stock = match addr.send(_GetStock {}).await {
            Ok(Ok(stock)) => stock,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };
        let product_stock = match product_stock.get("product1") {
            Some(stock) => stock,
            None => {
                assert_eq!(true, false);
                return;
            }
        };

        assert_eq!(product_stock.available_quantity, 5);
        assert_eq!(product_stock.reserved_quantity, 0);

        let res = match addr
            .send(ReserveProduct {
                product: "product1".to_string(),
                quantity: 10,
                time_limit: 1,
            })
            .await
        {
            Ok(Err(res)) => res,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };

        assert_eq!(res, Errors::NotEnoughStockError);

        let product_stock = match addr.send(_GetStock {}).await {
            Ok(Ok(stock)) => stock,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };
        let product_stock = match product_stock.get("product1") {
            Some(stock) => stock,
            None => {
                assert_eq!(true, false);
                return;
            }
        };

        assert_eq!(product_stock.available_quantity, 5);
        assert_eq!(product_stock.reserved_quantity, 0);
    }

    #[actix_rt::test]
    async fn test_store_actor_order_volume_random() {
        let mut store = Store {
            stock: HashMap::new(),
            reserve_sender: tokio::sync::mpsc::channel(1).0,
            active_ecoms: HashMap::new(),
            connection: false,
            leader: 0,
        };

        for i in 0..VOLUME_SIZE {
            let product_stock = ProductStock {
                available_quantity: 25,
                reserved_quantity: 0,
            };

            store.stock.insert(format!("product{}", i), product_stock);
        }

        let mut orders = Vec::new();
        for i in 0..VOLUME_SIZE {
            let order = LocalProductOrder {
                product: format!("product{}", i),
                quantity: rand::Rng::gen_range(&mut rand::thread_rng(), 1, 25),
            };
            orders.push(order);
        }

        let addr = store.start();
        for order in orders.clone() {
            let res = addr.send(order).await;
            assert!(res.is_ok());
        }
        let product_stock = match addr.send(_GetStock {}).await {
            Ok(Ok(stock)) => stock,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };
        for i in 0..VOLUME_SIZE {
            let product_stock = match product_stock.get(&format!("product{}", i)) {
                Some(stock) => stock,
                None => {
                    assert_eq!(true, false);
                    return;
                }
            };
            assert_eq!(product_stock.available_quantity, 25 - orders[i].quantity);
            assert_eq!(product_stock.reserved_quantity, 0);
        }
    }

    #[actix_rt::test]
    async fn test_store_actor_order_volume_random_not_enough_stock() {
        let mut store = Store {
            stock: HashMap::new(),
            reserve_sender: tokio::sync::mpsc::channel(1).0,
            active_ecoms: HashMap::new(),
            connection: false,
            leader: 0,
        };

        for i in 0..VOLUME_SIZE {
            let product_stock = ProductStock {
                available_quantity: 25,
                reserved_quantity: 0,
            };

            store.stock.insert(format!("product{}", i), product_stock);
        }

        let mut orders = Vec::new();
        for i in 0..VOLUME_SIZE {
            let order = LocalProductOrder {
                product: format!("product{}", i),
                quantity: rand::Rng::gen_range(&mut rand::thread_rng(), 26, 50),
            };
            orders.push(order);
        }

        let addr = store.start();
        for order in orders.clone() {
            let res = match addr.send(order).await {
                Ok(Err(res)) => res,
                _ => {
                    assert_eq!(true, false);
                    return;
                }
            };
            assert_eq!(res, Errors::NotEnoughStockError);
        }

        let product_stock = match addr.send(_GetStock {}).await {
            Ok(Ok(stock)) => stock,
            _ => {
                assert_eq!(true, false);
                return;
            }
        };
        for i in 0..VOLUME_SIZE {
            let product_stock = match product_stock.get(&format!("product{}", i)) {
                Some(stock) => stock,
                None => {
                    assert_eq!(true, false);
                    return;
                }
            };
            assert_eq!(product_stock.available_quantity, 25);
            assert_eq!(product_stock.reserved_quantity, 0);
        }
    }
}
