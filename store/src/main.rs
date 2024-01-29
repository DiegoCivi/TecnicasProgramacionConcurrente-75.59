use actix::prelude::*;
use actix::Actor;
use lib::store::NewEcomHandler;
use lib::{
    errors::Errors,
    store::{
        Connect, DispatchProduct, KillConnection, LocalProductOrder, ProductStock, ShowState, Store,
    },
};
use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::env::args;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::{fs::File as TFile, io::AsyncWriteExt, task};
use tokio::{
    io::{AsyncBufReadExt, BufReader as TBufReader},
    join,
    time::Duration,
};
use tokio_stream::{wrappers::LinesStream, StreamExt};

const STOCK_FILE_INDEX: usize = 1;
const ORDERS_FILE_INDEX: usize = 2;
const ID_INDEX: usize = 3;
const ECOM_AMOUNT_INDEX: usize = 4;
const PORT_INDEX: usize = 5;

const MIN_SECS_DISPATCH: u64 = 1;
const MAX_SECS_DISPATCH: u64 = 10;
const PHYSICAL_CLIENTS_DELAY: u64 = 2;
const DISCONNECTION_CHANNEL_SIZE: usize = 5;
const RESERVE_CHANNEL_SIZE: usize = 10;

const CONNECT_INPUT: &str = "C";
const KILL_INPUT: &str = "K";
const STOCK_INPUT: &str = "S";

const IPS_START: usize = 6;
const IPS_END_INDEX: usize = IPS_START + 6;

/// This main initializes the Store actor and to run every async function that make possible for the store
/// side to run concurrently
fn main() -> Result<(), Errors> {
    let args: Vec<String> = args().collect(); // Args order: stock_file orders_file ecommerce_addr id

    let (reserve_sender, mut reserve_receiver): (Sender<String>, Receiver<String>) =
        mpsc::channel(RESERVE_CHANNEL_SIZE);

    let store = initialize_store(args[STOCK_FILE_INDEX].clone(), reserve_sender)?;

    let ecom_amount = <usize as FromStr>::from_str(&args[ECOM_AMOUNT_INDEX])
        .map_err(|_| Errors::CouldNotParse)?;
    let (senders_vect, receivers_vect) =
        create_channels(ecom_amount).map_err(|_| Errors::ChannelError)?;
    let ips_ecoms = args_vec(&args);

    let system = System::new();
    system.block_on(async {
        let store_addr = store.start();

        let physical_sales_fut =
            physical_sales(args[ORDERS_FILE_INDEX].clone(), store_addr.clone());
        let ecom_connection_fut = ecom_connection(
            ips_ecoms,
            args[ID_INDEX].clone(),
            store_addr.clone(),
            receivers_vect,
        );
        let reserves_manager_fut = reserves_manager(store_addr.clone(), &mut reserve_receiver);
        let user_input_fut = user_input(store_addr.clone(), senders_vect);

        let _ = join!(
            physical_sales_fut,
            reserves_manager_fut,
            user_input_fut,
            ecom_connection_fut
        );
    });

    Ok(())
}

/// Creates a vec with tuples in the form of (ip, id), where each tuple corresponds to a different ecom in the network
fn args_vec(args: &[String]) -> Vec<(String, String)> {
    let mut ecoms: Vec<(String, String)> = vec![];
    for i in (IPS_START..IPS_END_INDEX).step_by(2) {
        let ip = format!("{}:{}", args[i].clone(), args[PORT_INDEX].clone());
        let id = args[i + 1].clone();
        ecoms.push((ip, id));
    }
    ecoms
}

type StringChannels = (Vec<Sender<String>>, Vec<Receiver<String>>);
/// Creates a vec with a channel for each known ecom in the network.
fn create_channels(amount: usize) -> Result<StringChannels, Errors> {
    let mut senders: Vec<Sender<String>> = vec![];
    let mut receivers: Vec<Receiver<String>> = vec![];
    for _ in 0..amount {
        let (sender, receiver): (Sender<String>, Receiver<String>) =
            mpsc::channel(DISCONNECTION_CHANNEL_SIZE);
        senders.push(sender);
        receivers.push(receiver);
    }
    Ok((senders, receivers))
}

/// This async function listens to the user input from the terminal and handles the different supportede messages.
async fn user_input(store: Addr<Store>, senders_vect: Vec<Sender<String>>) -> Result<(), Errors> {
    let mut input = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    while let Ok(msg) = input.next_line().await {
        if let Some(user_msg) = msg {
            match user_msg.as_str() {
                KILL_INPUT => {
                    let _ = store.send(KillConnection {}).await;
                }
                CONNECT_INPUT => {
                    let _ = store.send(Connect {}).await;
                    for sender in &senders_vect {
                        let _ = sender.send(user_msg.clone()).await;
                    }
                }
                STOCK_INPUT => {
                    let _ = store.send(ShowState {}).await;
                }
                _ => eprintln!(
                    "[USER_INPUT] Se recibio un mensaje por terminal que no se sabe responder"
                ),
            }
        }
    }
    Ok(())
}

/// This async function receives online orders from the Store actor. For each order it will create a task that will sleep
/// for a random number of seconds, simulating the time that is waited for the reserved product to be dispatched.
async fn reserves_manager(
    store: Addr<Store>,
    reserve_receiver: &mut Receiver<String>,
) -> Result<(), Errors> {
    let mut rng = thread_rng();
    let mut task_id = 0;

    while let Some(msg) = reserve_receiver.recv().await {
        // Seconds that will simulate how long the product is reserved
        let seconds: u64 = rng.gen_range(MIN_SECS_DISPATCH, MAX_SECS_DISPATCH);
        let duration_to_dispatch = std::time::Duration::from_secs(seconds);

        // Creation of DispatchProduct message
        let split: Vec<&str> = msg.split(',').collect();
        let product = split[0].to_string();
        let quantity = <usize as FromStr>::from_str(split[1]).map_err(|_| Errors::CouldNotParse)?;
        let time_limit = <u64 as FromStr>::from_str(split[2]).map_err(|_| Errors::CouldNotParse)?;
        let cancel_order = time_limit < seconds;
        let dispatch_msg = DispatchProduct {
            product,
            quantity,
            cancel_order,
        };

        // Cloned store address to notify when to dispatch the product
        let cloned_addr = store.clone();

        // Tasks will run concurrently waiting for each product to be ready to be dispatched
        tokio::spawn(async move {
            tokio::time::sleep(duration_to_dispatch).await;
            match cloned_addr.try_send(dispatch_msg) {
                Ok(_) => println!("[TASK_{task_id}] Se envio correctamente el mensaje DispatchProduct con orden [{msg}]"),
                Err(_) => println!("[TASK_{task_id}] No se pudo enviar el mensaje DispatchProduct con orden [{msg}]"),
            };
        });
        task_id += 1;
    }
    Ok(())
}

/// For each ecom in the network, a task that will handle the conection is created.
async fn ecom_connection(
    ips_ecoms: Vec<(String, String)>,
    my_id: String,
    store: Addr<Store>,
    mut receivers: Vec<Receiver<String>>,
) -> Result<(), Errors> {
    for (ip, id) in ips_ecoms {
        let my_id_clone = my_id.clone();
        let store_clone = store.clone();
        if let Some(mut receiver) = receivers.pop() {
            task::spawn(async move {
                let _ = online_sales(ip, my_id_clone, store_clone, &mut receiver, id).await;
            });
        } else {
            eprintln!("[ECOM_CONNECTION] No hay receivers para mandarle online_sales()");
        }
    }
    Ok(())
}

/// This async function handles the connection of the store with the ecommerce.
async fn online_sales(
    ip_addr: String,
    id: String,
    store: Addr<Store>,
    receiver: &mut Receiver<String>,
    ecom_id: String,
) -> Result<(), Errors> {
    loop {
        if let Ok(mut stream) = TcpStream::connect(ip_addr.clone()).await {
            let _ = stream.write(id.as_bytes()).await;
            let _ = store.try_send(NewEcomHandler {
                stream,
                ecom_id: ecom_id.clone(),
            });

            if let Some(connection) = receiver.recv().await {
                if connection == CONNECT_INPUT {
                    println!("[ONLINE_SALES] Rconectando con ecommerce");
                } else {
                    println!("[ONLINE_SALES] Se termina la conexion con el ecommerce");
                }
            } else {
                eprintln!("[ONLINE_SALES] Error a la hora de leer por el channel para recuperar la conexion con el ecom.");
            }
        }
    }
}

/// Responsible of execution the async function thath simulates the arrival of physical clients.
async fn physical_sales(client_orders: String, store: Addr<Store>) -> Result<(), Errors> {
    let result = receive_clients(client_orders, &store).await;
    match result {
        Ok(_) => println!("[PHYS_SALES] ¡All orders were processed!"),
        Err(e) => println!("[PHYS_SALES] ¡Error: {:?}!", e),
    }
    Ok(())
}

/// Initializes the Struct Actor, thath will become the Store actor. Reads the stock file
/// so as to create the Store with its initial stock.
fn initialize_store(stock_file: String, reserve_sender: Sender<String>) -> Result<Store, Errors> {
    let initial_stock = File::open(stock_file).map_err(|_| Errors::FileDoesNotExist)?;
    let reader = BufReader::new(initial_stock);
    let mut stock_hash: HashMap<String, ProductStock> = HashMap::new();

    for line in reader.lines() {
        let text = line.map_err(|_| Errors::ErrorReadingFile)?;

        let item: Vec<&str> = text.split(',').collect();

        let quantity = <usize as FromStr>::from_str(item[1]).map_err(|_| Errors::CouldNotParse)?;

        let product_stock = ProductStock {
            available_quantity: quantity,
            reserved_quantity: 0,
        };
        stock_hash.insert(item[0].to_string(), product_stock);
    }
    let store = Store {
        stock: stock_hash,
        reserve_sender,
        active_ecoms: HashMap::new(),
        connection: false,
        leader: 0,
    };
    Ok(store)
}

/// This async function simulates the arrival of physical clients. It reads the client_orders file
/// and sleeps for a random number of seconds.
async fn receive_clients(clients: String, store: &Addr<Store>) -> Result<(), Errors> {
    let file = TFile::open(clients)
        .await
        .map_err(|_| Errors::FileDoesNotExist)?;

    let mut lines = LinesStream::new(TBufReader::new(file).lines());
    while let Some(line) = lines.next().await {
        let text = line.map_err(|_| Errors::ErrorReadingFile)?;

        let splitted_order: Vec<&str> = text.split(',').collect();
        let order_quantity =
            <usize as FromStr>::from_str(splitted_order[1]).map_err(|_| Errors::CouldNotParse)?;

        // Creation of the order
        let order = LocalProductOrder {
            product: splitted_order[0].to_owned(),
            quantity: order_quantity,
        };
        println!("[PHYS_SALES] Order: {} = {}", order.product, order.quantity);

        // Sending the order of the client to the store
        let result = store
            .send(order)
            .await
            .map_err(|_| Errors::CouldNotReserve)?;
        if result.is_err() {
            println!("[PHYS_SALES] No hay stock para el pedido [{text}]");
        }

        tokio::time::sleep(Duration::from_secs(PHYSICAL_CLIENTS_DELAY)).await;
    }
    Ok(())
}
