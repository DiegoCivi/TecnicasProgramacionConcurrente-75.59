// Module dedicated to testing the unitary behavior of the abstract store

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use actix::prelude::*;
    use lib::{
        abstract_store::{AbstractStore, AddStock, UpdateStock},
        coordinator::Coordinator,
    };

    #[actix_rt::test]
    async fn test_abstract_store_add_stock() {
        let store_id = "1".to_string();
        let coord = Coordinator {
            id: 0,
            curr_leader: Some(1 as usize),
            active_stores: HashMap::new(),
            active_ecoms: HashMap::new(),
            online_orders: vec![],
            rng: rand::thread_rng(),
        };
        let abs_store = AbstractStore {
            write: None,
            store_id: store_id.clone(),
            stock: HashMap::new(),
            orders_buffer: vec![],
            coordinator: coord.start(),
        };
        let addr = abs_store.start();
        let add_stock = AddStock {
            product: "Camisa".to_string(),
            quantity: "1".to_string(),
        };
        let _ = addr.send(add_stock).await;
        let stock = match addr.send(lib::abstract_store::_GetStock).await {
            Ok(Ok(stock)) => stock,
            _ => HashMap::new(),
        };
        assert_eq!(stock.contains_key(&"Camisa".to_string()), true);
    }

    #[actix_rt::test]
    async fn test_abstract_store_add_stock_fail() {
        let store_id = "1".to_string();
        let coord = Coordinator {
            id: 0,
            curr_leader: Some(1 as usize),
            active_stores: HashMap::new(),
            active_ecoms: HashMap::new(),
            online_orders: vec![],
            rng: rand::thread_rng(),
        };
        let abs_store = AbstractStore {
            write: None,
            store_id: store_id.clone(),
            stock: HashMap::new(),
            orders_buffer: vec![],
            coordinator: coord.start(),
        };
        let addr = abs_store.start();
        let add_stock = AddStock {
            product: "Zapatillas".to_string(),
            quantity: "1".to_string(),
        };
        let _ = addr.send(add_stock).await;
        let stock = match addr.send(lib::abstract_store::_GetStock).await {
            Ok(Ok(stock)) => stock,
            _ => HashMap::new(),
        };
        assert_eq!(stock.contains_key(&"2".to_string()), false);
    }

    #[actix_rt::test]
    async fn test_abstract_store_update_stock() {
        let store_id = "1".to_string();
        let coord = Coordinator {
            id: 0,
            curr_leader: Some(1 as usize),
            active_stores: HashMap::new(),
            active_ecoms: HashMap::new(),
            online_orders: vec![],
            rng: rand::thread_rng(),
        };
        let abs_store = AbstractStore {
            write: None,
            store_id: store_id.clone(),
            stock: HashMap::new(),
            orders_buffer: vec![],
            coordinator: coord.start(),
        };
        let addr = abs_store.start();
        let add_stock = AddStock {
            product: "Campera".to_string(),
            quantity: "5".to_string(),
        };
        let _ = addr.send(add_stock).await;
        let update_stock = UpdateStock {
            product: "Campera".to_string(),
            quantity: "1".to_string(),
        };
        let _ = addr.send(update_stock).await;
        let stock = match addr.send(lib::abstract_store::_GetStock).await {
            Ok(Ok(stock)) => stock,
            _ => HashMap::new(),
        };
        let valor = match stock.get(&"Campera".to_string()) {
            Some(valor) => valor,
            None => &0,
        };
        assert_eq!(valor, &4);
    }

    #[actix_rt::test]
    async fn test_abstract_store_update_stock_fail() {
        let store_id = "1".to_string();
        let coord = Coordinator {
            id: 0,
            curr_leader: Some(1 as usize),
            active_stores: HashMap::new(),
            active_ecoms: HashMap::new(),
            online_orders: vec![],
            rng: rand::thread_rng(),
        };
        let abs_store = AbstractStore {
            write: None,
            store_id: store_id.clone(),
            stock: HashMap::new(),
            orders_buffer: vec![],
            coordinator: coord.start(),
        };
        let addr = abs_store.start();
        let add_stock = AddStock {
            product: "Campera".to_string(),
            quantity: "2".to_string(),
        };
        let _ = addr.send(add_stock).await;
        let update_stock = UpdateStock {
            product: "Campera".to_string(),
            quantity: "3".to_string(),
        };
        let result = match addr.send(update_stock).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(_)) => Err(()),
            Err(_) => Err(()),
        };
        assert_eq!(result.is_err(), true);
        let stock = match addr.send(lib::abstract_store::_GetStock).await {
            Ok(Ok(stock)) => stock,
            _ => HashMap::new(),
        };
        let valor = match stock.get(&"Campera".to_string()) {
            Some(valor) => valor,
            None => &0,
        };
        assert_eq!(valor, &2);
    }
}
