// Module dedicated to testing the unitary behavior of the coordinator

#[cfg(test)]
mod tests {
    use actix::prelude::*;
    use lib::coordinator::{Coordinator, NewStore};
    use std::collections::HashMap;
    use tokio::net::{TcpListener, TcpStream};
    #[actix_rt::test]
    async fn test_coordinator_new_store() {
        let coordinator = Coordinator {
            id: 0 as usize,
            curr_leader: Some(1 as usize),
            active_stores: HashMap::new(),
            active_ecoms: HashMap::new(),
            online_orders: vec![],
            rng: rand::thread_rng(),
        };

        let store_id = "1".to_string();
        let address = "127.0.0.1:7232".to_string();
        let listener = TcpListener::bind(address.clone()).await;
        match listener {
            Ok(_) => {}
            Err(e) => {
                println!("Error: {}", e);
                assert_eq!(true, false);
            }
        }
        let stream_tcp = match TcpStream::connect(address.clone()).await {
            Ok(stream) => stream,
            Err(e) => {
                println!("Error: {}", e);
                assert_eq!(true, false);
                return;
            }
        };

        let new_store = NewStore {
            store_id: store_id.clone(),
            stream: stream_tcp,
        };
        let addr = coordinator.start();
        let _ = addr.send(new_store).await;
        let active_stores = match addr.send(lib::coordinator::_GetActiveStores).await {
            Ok(Ok(stock)) => stock,
            _ => HashMap::new(),
        };
        assert_eq!(active_stores.contains_key(&store_id), true);
    }

    #[actix_rt::test]
    #[should_panic]
    async fn test_coordinator_new_store_fail() {
        let coordinator = Coordinator {
            id: 0,
            curr_leader: Some(1 as usize),
            active_stores: HashMap::new(),
            active_ecoms: HashMap::new(),
            online_orders: vec![],
            rng: rand::thread_rng(),
        };

        let store_id = "1".to_string();
        let address = "127.0.0.2:7232".to_string();
        let stream_tcp = match TcpStream::connect(address.clone()).await {
            Ok(stream) => stream,
            Err(e) => {
                println!("Error: {}", e);
                assert_eq!(true, false);
                return;
            }
        };
        let new_store = NewStore {
            store_id: store_id.clone(),
            stream: stream_tcp,
        };
        let addr = coordinator.start();
        let _ = addr.send(new_store).await;
        let active_stores = match addr.send(lib::coordinator::_GetActiveStores).await {
            Ok(Ok(stock)) => stock,
            _ => HashMap::new(),
        };
        assert_eq!(active_stores.contains_key(&store_id), false);
    }

    #[actix_rt::test]
    async fn test_coordinator_store_disconnected() {
        let coordinator = Coordinator {
            id: 0,
            curr_leader: Some(1 as usize),
            active_stores: HashMap::new(),
            active_ecoms: HashMap::new(),
            online_orders: vec![],
            rng: rand::thread_rng(),
        };

        let store_id = "1".to_string();
        let address = "127.0.0.3:7232".to_string();
        let listener = TcpListener::bind(address.clone()).await;
        match listener {
            Ok(_) => {}
            Err(e) => {
                println!("Error: {}", e);
                assert_eq!(true, false);
            }
        }
        let stream_tcp = match TcpStream::connect(address.clone()).await {
            Ok(stream) => stream,
            Err(e) => {
                println!("Error: {}", e);
                assert_eq!(true, false);
                return;
            }
        };
        let new_store = NewStore {
            store_id: store_id.clone(),
            stream: stream_tcp,
        };
        let addr = coordinator.start();
        let _ = addr.send(new_store).await;
        let active_stores = match addr.send(lib::coordinator::_GetActiveStores).await {
            Ok(Ok(stock)) => stock,
            _ => HashMap::new(),
        };
        assert_eq!(active_stores.contains_key(&store_id), true);
        let _ = addr
            .send(lib::coordinator::StoreDisconnected {
                store_id: store_id.clone(),
            })
            .await;
        let active_stores = match addr.send(lib::coordinator::_GetActiveStores).await {
            Ok(Ok(stock)) => stock,
            _ => HashMap::new(),
        };
        assert_eq!(active_stores.contains_key(&store_id), false);
    }

    #[actix_rt::test]
    #[should_panic]
    async fn test_coordinator_store_disconnected_fail() {
        let coordinator = Coordinator {
            id: 0,
            curr_leader: Some(1 as usize),
            active_stores: HashMap::new(),
            active_ecoms: HashMap::new(),
            online_orders: vec![],
            rng: rand::thread_rng(),
        };

        let store_id = "1".to_string();
        let address = "127.0.0.4:7232".to_string();
        let listener = TcpListener::bind(address.clone()).await;
        match listener {
            Ok(_) => {}
            Err(e) => {
                println!("Error: {}", e);
                assert_eq!(true, false);
            }
        }
        let stream_tcp = match TcpStream::connect(address.clone()).await {
            Ok(stream) => stream,
            Err(e) => {
                println!("Error: {}", e);
                assert_eq!(true, false);
                return;
            }
        };
        let new_store = NewStore {
            store_id: store_id.clone(),
            stream: stream_tcp,
        };
        let addr = coordinator.start();
        let _ = addr.send(new_store).await;
        let active_stores = match addr.send(lib::coordinator::_GetActiveStores).await {
            Ok(Ok(stock)) => stock,
            _ => HashMap::new(),
        };
        assert_eq!(active_stores.contains_key(&store_id), true);
        let resultado = addr
            .send(lib::coordinator::StoreDisconnected {
                store_id: "2".to_string(),
            })
            .await;
        assert_eq!(resultado.is_err(), true);
        let active_stores = match addr.send(lib::coordinator::_GetActiveStores).await {
            Ok(Ok(stock)) => stock,
            _ => HashMap::new(),
        };
        assert_eq!(active_stores.contains_key(&store_id), true);
    }
}
