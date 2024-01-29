use actix::{Actor, Addr, System};
use futures::join;
use lib::{
    coordinator::{Coordinator, NewOrder, NewStore},
    ecom::{ecom_connection_listener, ecom_network},
    errors::Errors,
};
use rand::{thread_rng, Rng};
use std::{
    collections::HashMap,
    env::args,
    fs::File,
    io::{BufRead, BufReader},
    str::FromStr,
};
use tokio::time::{sleep, Duration};
use tokio::{io::Interest, net::TcpListener};

const ARGS_ORDER_FILE: usize = 1;
const ARGS_MY_IP: usize = 2;
const ARGS_MY_ID: usize = 3;
const ARGS_ECOM_PORT: usize = 8;
const ARGS_STORES_PORT: usize = 9;

/// This main starts the system where every async function and actors will co-exist.
/// But before all that, it parses de arguments from the terminal. With this arguments
/// main knows the ecoms ips, the ports tu use, orders and stock files, and the process id.
fn main() -> Result<(), Errors> {
    let args: Vec<String> = args().collect(); // Args Order: orders_file, my_ip, my_id, ecommerce_ip1, ecom1_id, ecommerce_ip2, ecom2_id, ecommerces_port, stores_port
    let address_stores = args[ARGS_MY_IP].to_string() + ":" + &args[ARGS_STORES_PORT];

    let orders = load_online_orders(args[ARGS_ORDER_FILE].clone())?;
    let rng = thread_rng();
    let my_id =
        <usize as FromStr>::from_str(&args[ARGS_MY_ID]).map_err(|_| Errors::CouldNotParse)?;
    let coord = Coordinator {
        online_orders: orders.clone(),
        active_stores: HashMap::new(),
        active_ecoms: HashMap::new(),
        rng,
        id: my_id,
        curr_leader: Some(my_id),
    };

    let system = System::new();

    let mut ecoms: Vec<(String, String)> = vec![];
    for i in (4..8).step_by(2) {
        let addr = format!("{}:{}", args[i].clone(), args[ARGS_ECOM_PORT]);
        let id = args[i + 1].clone();
        ecoms.push((addr, id));
    }

    system.block_on(async {
        let coord_addr = coord.start();

        let my_addr = args[ARGS_MY_IP].to_string() + ":" + &args[ARGS_ECOM_PORT];
        let ecom_network_fut = ecom_network(ecoms, coord_addr.clone(), my_id.to_string());
        let ecom_conn_istener_fut = ecom_connection_listener(my_addr, coord_addr.clone());

        let discover_stores_fut = discover_stores(address_stores, coord_addr.clone());
        let order_manager_fut = order_manager(coord_addr.clone(), orders);

        let (_, _, _, _) = join!(
            discover_stores_fut,
            ecom_network_fut,
            order_manager_fut,
            ecom_conn_istener_fut
        );
    });

    system.run().map_err(|_| Errors::SystemRunFail)?;

    Ok(())
}

/// This async function gets all the orders from a vec, and sends those orders to the Coordinator actor
/// in random intervals.
async fn order_manager(addr: Addr<Coordinator>, orders: Vec<String>) -> Result<(), Errors> {
    let mut rng_sleep = thread_rng();

    let mut i = 0;
    while i < orders.len() {
        let secs = rng_sleep.gen_range(2, 6);
        let dur = Duration::from_secs(secs);
        sleep(dur).await;
        let order = orders[i].to_string();
        println!("[ORDER_MANAGER] Orden procesando...");
        if let Ok(Ok(_)) = addr
            .send(NewOrder {
                order,
                visited_stores: vec![],
            })
            .await
        {
            i += 1;
        }
    }
    Ok(())
}

/// Reads the online_orders file and pushes each order into a vec, which will be used by order_manager()
fn load_online_orders(orders_file: String) -> Result<Vec<String>, Errors> {
    let mut online_orders: Vec<String> = vec![];
    let file = File::open(orders_file).map_err(|_| Errors::FileDoesNotExist)?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let order = line.map_err(|_| Errors::ErrorReadingFile)?;
        online_orders.push(order);
    }
    Ok(online_orders)
}

/// This async function is responsible for discovering and connecting with new stores.
/// With each connection the coordinator is told to create a new AbstractStore actor instance.
async fn discover_stores(addr: String, coord: Addr<Coordinator>) -> Result<(), Errors> {
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|_| Errors::ConnectionError)?;
    while let Ok(tuple) = listener.accept().await {
        let (stream, _) = tuple;
        let mut buf = [0; 1];
        let timeout_duration = Duration::from_secs(10);
        match tokio::time::timeout(timeout_duration, stream.ready(Interest::READABLE)).await {
            Ok(_) => {
                match stream.try_read(&mut buf) {
                    Ok(_) => {
                        // The coordinator is told to create a new AbstractStore
                        println!(
                            "[DISC_STORES] El id recibido es [{:?}]",
                            String::from_utf8(buf.to_vec())
                        );
                        let store_id =
                            String::from_utf8(buf.to_vec()).map_err(|_| Errors::CouldNotParse)?;
                        let res = coord.try_send(NewStore { store_id, stream });
                        match res {
                            Ok(_) => {},
                            Err(_) => println!("[DISC_STORES] No se pudo mandar el mensaje para crear una nueva store"),
                        }
                    }
                    Err(e) => {
                        eprintln!("[DISC_STORES] Error a la hora de leer: {:?}", e)
                    }
                }
            }
            Err(_) => eprintln!("[DISC_STORES] Tiempo de lectura agotado."),
        }
    }
    Ok(())
}
