extern crate actix;

use crate::{
    coordinator::{Coordinator, GetLeader, NewOrder, StoreDisconnected},
    errors::Errors,
};
use actix::{
    fut::wrap_future, prelude::ContextFutureSpawner, Actor, ActorContext, ActorFutureExt, Addr,
    AsyncContext, Context, Handler, Message, StreamHandler,
};
use std::{collections::HashMap, str::FromStr};
use tokio::{
    io::{AsyncWriteExt, WriteHalf},
    net::TcpStream,
};

/// AbstractStore actor. It is in charge of handling the connection with the coordinator and the actual store.
/// It also handles the stock and the orders.
pub struct AbstractStore {
    pub write: Option<WriteHalf<TcpStream>>,
    pub store_id: String,
    pub stock: HashMap<String, usize>,
    pub orders_buffer: Vec<UpdateStock>,
    pub coordinator: Addr<Coordinator>,
}

impl Actor for AbstractStore {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("¡AbstractStore is alive with id [{}]!", self.store_id);
    }
}

/// This handler is responsible for reading every message sent by the store.
/// Each message is handled differently, and has consequences on the AbstractStore
/// or even the Coordinator.
impl StreamHandler<Result<String, std::io::Error>> for AbstractStore {
    fn handle(&mut self, read: Result<String, std::io::Error>, ctx: &mut Self::Context) {
        if let Ok(line) = read {
            let split: Vec<&str> = line.split(',').collect();
            let product = split[1].to_owned();
            let quantity = split[2].to_owned();
            match split[0] {
                "STOCK" => {
                    let msg = AddStock { product, quantity };
                    ctx.notify(msg);
                }
                "APPROVED" => {
                    println!("[ABSTRACT_STORE] [{}], pedido [{}]", self.store_id, line);
                    let msg = UpdateStock { product, quantity };
                    ctx.notify(msg);
                }
                "CANCELLED" => {
                    println!(
                        "[ABSTRACT_STORE] Mi id es [{}], pedido [{}]",
                        self.store_id, line
                    );
                }
                "LEADER" => {
                    let _ = self.coordinator.try_send(GetLeader {
                        sender_id: self.store_id.clone(),
                    });
                }
                _ => println!(
                    "[ABSTRACT_STORE] Mi id es [{}], me llego un mensaje que no se responder",
                    self.store_id
                ),
            }
        }
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        println!("[ABSTRACT_STORE_{}] Conexion terminada.", self.store_id);
        let _ = self.coordinator.try_send(StoreDisconnected {
            store_id: self.store_id.clone(),
        });
        ctx.stop()
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// Message to notify the coordinator that a new order has arrived. It contains the order and the stores that have already been visited.
/// It redirects the order to a store that has the product in stock. If no store has the product in stock, it returns an error.
pub struct Order {
    pub order: String,
    pub visited_stores: Vec<String>,
}

impl Handler<Order> for AbstractStore {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: Order, ctx: &mut Self::Context) -> Result<(), Errors> {
        // Checks if there is stock of the product
        let split: Vec<&str> = msg.order.split(',').collect();
        let product = split[0].to_string();
        let quantity = <usize as FromStr>::from_str(split[1]).map_err(|_| Errors::CouldNotParse)?;

        if let Some(stock_quantity) = self.stock.get(&product) {
            if stock_quantity < &quantity {
                let mut new_vec = msg.visited_stores;
                new_vec.push(self.store_id.clone());
                let _ = self.coordinator.try_send(NewOrder {
                    order: msg.order,
                    visited_stores: new_vec,
                });
                return Ok(());
            }
        }

        let order = msg.order.clone();
        let mut write = self
            .write
            .take()
            .expect("No debería poder llegar otro mensaje antes de que vuelva por usar ctx.wait");
        wrap_future::<_, Self>(async move {
            let _ = write.write_all(format!("{}\n", order).as_bytes()).await;
            write
        })
        .map(|write, this, _| this.write = Some(write))
        .wait(ctx);
        Ok(())
    }
}

#[derive(Message, Clone)]
#[rtype(result = "Result<(), Errors>")]
/// Message to _update_ the stock of a product. It contains the product and the quantity to be substracted from the current
/// one.
pub struct UpdateStock {
    pub product: String,
    pub quantity: String,
}

impl Handler<UpdateStock> for AbstractStore {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: UpdateStock, _: &mut Self::Context) -> Result<(), Errors> {
        if !self.orders_buffer.is_empty() {
            let mut new_orders_buffer: Vec<UpdateStock> = vec![];
            for order in &self.orders_buffer {
                let prod_opt = self.stock.get_mut(&order.product);
                if let Some(prod_quantity) = prod_opt {
                    let order_quantity = <usize as FromStr>::from_str(&order.quantity)
                        .map_err(|_| Errors::CouldNotParse)?;
                    if order_quantity > *prod_quantity {
                        return Err(Errors::NoStockError);
                    }
                    *prod_quantity -= order_quantity;
                } else {
                    new_orders_buffer.push((*order).clone());
                }
            }
            self.orders_buffer = new_orders_buffer;
        }

        let prod_opt = self.stock.get_mut(&msg.product);
        if let Some(prod_quantity) = prod_opt {
            let order_quantity =
                <usize as FromStr>::from_str(&msg.quantity).map_err(|_| Errors::CouldNotParse)?;
            if order_quantity > *prod_quantity {
                return Err(Errors::NoStockError);
            }
            *prod_quantity -= order_quantity;
        } else {
            // If we receive an update of a product we dont have, we put it on the buffer to check later
            self.orders_buffer.push(msg);
        }
        println!(
            "[ABSTRACT_STORE] Mi id es [{}] y mi stock es [{:?}]",
            self.store_id, self.stock
        );
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// Message to _add_ stock of a product. It contains the product and the quantity to be added.
pub struct AddStock {
    pub product: String,
    pub quantity: String,
}

impl Handler<AddStock> for AbstractStore {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: AddStock, _: &mut Self::Context) -> Result<(), Errors> {
        let quantity =
            <usize as FromStr>::from_str(&msg.quantity).map_err(|_| Errors::CouldNotParse)?;
        self.stock.insert(msg.product, quantity);
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// Message to notify the coordinator that this store is the new leader. It contains the id of the new leader.
pub struct NewLeader {
    pub leader_id: String,
}

impl Handler<NewLeader> for AbstractStore {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: NewLeader, ctx: &mut Self::Context) -> Result<(), Errors> {
        let mut write = self
            .write
            .take()
            .expect("No debería poder llegar otro mensaje antes de que vuelva por usar ctx.wait");
        wrap_future::<_, Self>(async move {
            let _ = write
                .write_all(format!("LEADER,{}\n", msg.leader_id).as_bytes())
                .await;
            write
        })
        .map(|write, this, _| this.write = Some(write))
        .wait(ctx);
        Ok(())
    }
}

// ------------------ TEST PURPOSE MESSAGES ------------------

pub struct _GetStock;

impl Message for _GetStock {
    type Result = Result<HashMap<String, usize>, Errors>;
}

impl Handler<_GetStock> for AbstractStore {
    type Result = Result<HashMap<String, usize>, Errors>;

    fn handle(
        &mut self,
        _: _GetStock,
        _: &mut Self::Context,
    ) -> Result<HashMap<String, usize>, Errors> {
        Ok(self.stock.clone())
    }
}
