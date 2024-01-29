extern crate actix;

use crate::abstract_ecom::{AbstractEcom, Election, NewLeader2, SendOrder};
use crate::abstract_store::{AbstractStore, NewLeader, Order};
use crate::errors::Errors;
use actix::{Actor, Addr, AsyncContext, Context, Handler, Message, StreamHandler};
use rand::rngs::ThreadRng;
use rand::seq::SliceRandom;
use rand::Rng;
use std::clone::Clone;
use std::collections::HashMap;
use std::str::FromStr;
use tokio::io::{split, AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio_stream::wrappers::LinesStream;

const MIN_SECS_LIMIT: u64 = 1;
const MAX_SECS_LIMIT: u64 = 10;

/// Coordinator actor. It is in charge of handling the connection with the ecommerces and the stores,
/// as well as redirecting the orders to the stores and handling the stock and election of the leader.
pub struct Coordinator {
    pub online_orders: Vec<String>,
    pub active_stores: HashMap<String, Addr<AbstractStore>>,
    pub active_ecoms: HashMap<usize, Addr<AbstractEcom>>,
    pub rng: ThreadRng,
    pub id: usize,
    pub curr_leader: Option<usize>,
}

impl Actor for Coordinator {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        println!("¡Coordinator is alive!");
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// Message to notify the coordinator that a new store has connected. It contains the store id and the stream
/// to communicate with it. It creates a new _AbstractStore_ actor and stores it in the active_stores hashmap.
pub struct NewStore {
    pub store_id: String,
    pub stream: TcpStream,
}

impl Handler<NewStore> for Coordinator {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: NewStore, coord_ctx: &mut Self::Context) -> Result<(), Errors> {
        println!(
            "[COORDINATOR] Se recibio una nueva store con id [{}]",
            msg.store_id
        );

        let store_addr = AbstractStore::create(|ctx| {
            let (read, write_half) = split(msg.stream);
            AbstractStore::add_stream(LinesStream::new(BufReader::new(read).lines()), ctx);
            let write = Some(write_half);
            AbstractStore {
                write,
                store_id: msg.store_id.clone(),
                stock: HashMap::new(),
                orders_buffer: vec![],
                coordinator: coord_ctx.address(),
            }
        });

        if let Some(leader_id) = self.curr_leader {
            let _ = store_addr.try_send(NewLeader {
                leader_id: leader_id.to_string(),
            });
        } else {
            println!("[COORDINATOR] No hay lider para avisarle a la nueva AbstractStore");
            return Err(Errors::NoActiveLeader);
        }

        let cloned_id = msg.store_id.clone();
        self.active_stores.insert(cloned_id, store_addr);
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// Message to notify the coordinator that a new order has arrived. It contains the order and the stores that have already been visited.
/// It redirects the order to a store that has the product in stock. If no store has the product in stock, it returns an error.
pub struct NewOrder {
    pub order: String,
    pub visited_stores: Vec<String>,
}

impl Handler<NewOrder> for Coordinator {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: NewOrder, ctx: &mut Self::Context) -> Result<(), Errors> {
        if self.active_stores.is_empty() {
            println!("[COORDINATOR] No hay tiendas conectadas");
            return Err(Errors::NoActiveStoresError);
        } else if self.curr_leader.is_none() {
            println!("[COORDINATOR] No hay lider al que mandar los pedidos");
            return Err(Errors::NoActiveLeader);
        }

        if let Some(id) = self.curr_leader {
            if self.id != id {
                if let Some(ecom_addr) = self.active_ecoms.get_mut(&id) {
                    let _ = ecom_addr.try_send(SendOrder {
                        order: msg.order.clone(),
                    });
                    return Ok(());
                }
            }
        }

        if msg.visited_stores.len() >= self.active_stores.len() {
            println!(
                "[COORDINATOR] No hay tiendas con stock para el pedido [{}]",
                msg.order
            );
            return Ok(());
        } else {
            match self
                .active_stores
                .keys()
                .collect::<Vec<&String>>()
                .choose(&mut self.rng)
            {
                Some(id) => {
                    if msg.visited_stores.contains(id) {
                        ctx.notify(msg);
                        return Ok(());
                    }
                    let store_addr = self.active_stores.get(*id);
                    if let Some(addr) = store_addr {
                        let order_msg = format!(
                            "{},{}",
                            msg.order,
                            self.rng.gen_range(MIN_SECS_LIMIT, MAX_SECS_LIMIT)
                        );
                        if addr
                            .try_send(Order {
                                order: order_msg.clone(),
                                visited_stores: msg.visited_stores,
                            })
                            .is_err()
                        {
                            eprintln!("[COORDINATOR] Error al enviar mensaje Order con la orden [{}] al AbstractStore con id [{}]", order_msg, id);
                        }
                    }
                }
                None => {
                    println!("Error a la hora de conseguir un store id");
                    return Err(Errors::NotEnoughStockError);
                }
            };
        }
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// Message to notify the coordinator that a store has disconnected. It contains the store id.
pub struct StoreDisconnected {
    pub store_id: String,
}

impl Handler<StoreDisconnected> for Coordinator {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: StoreDisconnected, _: &mut Self::Context) -> Self::Result {
        let a = self.active_stores.remove(&msg.store_id);
        if a.is_none() {
            return Err(Errors::StoreNotConnectedError);
        }
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// Message to notify the coordinator that a new ecommerce has connected. It contains the ecommerce id and the stream to communicate with it.
/// It creates a new AbstractEcom.
pub struct NewEcom {
    pub id: String,
    pub stream: TcpStream,
}

impl Handler<NewEcom> for Coordinator {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: NewEcom, coord_ctx: &mut Self::Context) -> Result<(), Errors> {
        let new_ecom_id =
            <usize as FromStr>::from_str(&msg.id).map_err(|_| Errors::CouldNotParse)?;

        let abstract_ecom = AbstractEcom::create(|ctx| {
            let (read, write_half) = split(msg.stream);
            AbstractEcom::add_stream(LinesStream::new(BufReader::new(read).lines()), ctx);
            let write = Some(write_half);
            AbstractEcom {
                write,
                id: new_ecom_id,
                coord: coord_ctx.address(),
            }
        });

        if let Some(leader) = self.curr_leader {
            if new_ecom_id > leader {
                println!("[COORDINATOR] Cambio de lider a [{new_ecom_id}]");
                self.curr_leader = Some(new_ecom_id);

                // All abstract stores need to know that a new leader has been designated
                for addr in self.active_stores.values() {
                    let _ = addr.try_send(NewLeader {
                        leader_id: new_ecom_id.to_string(),
                    });
                }
            }
        }

        self.active_ecoms.insert(new_ecom_id, abstract_ecom);

        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// Message to notify the coordinator that an ecom has disconnected. It contains the ecom id.
/// It removes the coresponding ecom from his connected ones.
pub struct EcomDisconnected {
    pub ecom_id: usize,
}

impl Handler<EcomDisconnected> for Coordinator {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: EcomDisconnected, ctx: &mut Self::Context) -> Self::Result {
        let a = self.active_ecoms.remove(&msg.ecom_id);
        if a.is_none() {
            return Err(Errors::StoreNotConnectedError);
        }

        if let Some(curr) = self.curr_leader {
            if curr == msg.ecom_id {
                // Se busca un nuevo lider
                ctx.notify(CoordElection {
                    visited: vec![],
                    ecom_id: self.id + 1,
                });

                self.curr_leader = None;
            }
        } else {
            self.active_ecoms.remove(&msg.ecom_id);
        }
        Ok(())
    }
}

/// Message that takes part in the election of a new ecom leader.
/// It starts the process to notify the next ecom in the ring that an election is being held.
#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
pub struct CoordElection {
    pub ecom_id: usize,
    pub visited: Vec<usize>,
}

impl Handler<CoordElection> for Coordinator {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: CoordElection, ctx: &mut Self::Context) -> Self::Result {
        if msg.visited.contains(&self.id) {
            if let Some(max_id) = msg.visited.iter().max() {
                for ecom in self.active_ecoms.values() {
                    let _ = ecom.try_send(NewLeader2 {
                        new_leader_id: *max_id,
                    });
                }
            }
        } else {
            let mut new_vec = msg.visited.clone();
            new_vec.push(self.id);
            match self.active_ecoms.get(&msg.ecom_id) {
                Some(addr) => {
                    let _ = addr.try_send(Election { visited: new_vec });
                }
                None => {
                    if let Some(min_ecom_id) = self.active_ecoms.keys().min() {
                        if let Some(min_ecom_addr) = self.active_ecoms.get(min_ecom_id) {
                            let _ = min_ecom_addr.try_send(Election { visited: new_vec });
                        }
                    } else {
                        ctx.notify(ChangeLeader {
                            new_leader_id: self.id,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

/// Message that is called when a new leader is choosen. It notifies every store connected
/// the new leader id.
#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
pub struct ChangeLeader {
    pub new_leader_id: usize,
}

impl Handler<ChangeLeader> for Coordinator {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: ChangeLeader, _: &mut Self::Context) -> Self::Result {
        self.curr_leader = Some(msg.new_leader_id);
        for i in self.active_stores.keys() {
            if let Some(store_addr) = self.active_stores.get(i) {
                let _ = store_addr.try_send(NewLeader {
                    leader_id: msg.new_leader_id.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Message that answers the store who is the leader.
#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
pub struct GetLeader {
    pub sender_id: String,
}

impl Handler<GetLeader> for Coordinator {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: GetLeader, _: &mut Self::Context) -> Result<(), Errors> {
        if let Some(addr) = self.active_stores.get(&msg.sender_id) {
            if let Some(leader_id) = self.curr_leader {
                let _ = addr.try_send(NewLeader {
                    leader_id: leader_id.to_string(),
                });
            }
        }
        Ok(())
    }
}

// ------------------------ TEST PURPOSE MESSAGES ------------------------ //
pub struct _GetActiveStores;

impl Message for _GetActiveStores {
    type Result = Result<HashMap<String, Addr<AbstractStore>>, String>;
}

impl Handler<_GetActiveStores> for Coordinator {
    type Result = Result<HashMap<String, Addr<AbstractStore>>, String>;

    fn handle(
        &mut self,
        _: _GetActiveStores,
        _: &mut Self::Context,
    ) -> Result<HashMap<String, Addr<AbstractStore>>, String> {
        Ok(self.active_stores.clone())
    }
}
