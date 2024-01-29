extern crate actix;

use crate::ecom_handler::{Answer, EcomHandler, Stop};
use crate::errors::Errors;
use actix::{Actor, Addr, AsyncContext, Context, Handler, Message, StreamHandler};
use std::collections::HashMap;
use std::str::FromStr;
use tokio::io::{split, AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::LinesStream;

#[derive(Debug, Clone)]
/// The product stock is represented by a tuple of two `usize`, the first one is the available quantity and the second one is the reserved quantity.
pub struct ProductStock {
    pub available_quantity: usize,
    pub reserved_quantity: usize,
}

/// The store is represented by a `HashMap` of products and their stock, a sender to the reserves manager,
/// a hashmap of the ecommerces that are connected to the store and a `bool` that indicates
/// if the store is connected to the coordinator. The leader is represented by a `String` and the leader write by an `Option<WriteHalf<TcpStream>>`.
pub struct Store {
    pub stock: HashMap<String, ProductStock>,
    pub reserve_sender: Sender<String>,
    pub active_ecoms: HashMap<String, Addr<EcomHandler>>,
    pub connection: bool,
    pub leader: usize,
}

impl Actor for Store {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        println!("¡Store is alive!");
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// The store receives a message to _subtract_ the quantity of a product from the stock.
/// If the product is not in the stock, it returns an error. The quantity is represented by a `usize`
/// and the product with a `String`.
pub struct LocalProductOrder {
    pub product: String,
    pub quantity: usize,
}

impl Handler<LocalProductOrder> for Store {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: LocalProductOrder, ctx: &mut Self::Context) -> Result<(), Errors> {
        if let Some(product) = self.stock.get_mut(&msg.product) {
            if product.available_quantity - product.reserved_quantity >= msg.quantity {
                product.available_quantity -= msg.quantity;

                // The physical sale needs to be sent to the ecommerce so they can update their stock.
                let ans_msg = format!("APPROVED,{},{}", msg.product, msg.quantity);
                let ans = AnswerEcom { answer: ans_msg };
                ctx.notify(ans);
                Ok(())
            } else {
                Err(Errors::NotEnoughStockError)
            }
        } else {
            Err(Errors::ProductNotFoundError)
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// The store can communicate with the coordinator for multiple reasons, this message is used to send a message to the coordinator.
pub struct AnswerEcom {
    pub answer: String,
}

impl Handler<AnswerEcom> for Store {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: AnswerEcom, _: &mut Self::Context) -> Result<(), Errors> {
        if let Some(ecom_addr) = self.active_ecoms.get(&self.leader.to_string()) {
            let _ = ecom_addr.try_send(Answer { answer: msg.answer });
        }

        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// This message can be used to _**kill**_ the connection of the store to the network.
pub struct KillConnection {}

impl Handler<KillConnection> for Store {
    type Result = Result<(), Errors>;

    fn handle(&mut self, _: KillConnection, _: &mut Self::Context) -> Result<(), Errors> {
        for addr in self.active_ecoms.values() {
            let _ = addr.try_send(Stop);
        }
        self.active_ecoms.clear();
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// This message can be used to _**connect**_ the store to the network (Even if killed before).
pub struct Connect {}

impl Handler<Connect> for Store {
    type Result = Result<(), Errors>;

    fn handle(&mut self, _: Connect, ctx: &mut Self::Context) -> Result<(), Errors> {
        self.connection = true;
        let ask_learder_msg = AnswerEcom {
            answer: "LEADER,?,?".to_string(),
        };
        ctx.notify(ask_learder_msg);
        Ok(())
    }
}

// ------------------------ E-COMMERCE PURPOSE MESSAGES ------------------------
#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// With this message we _reserve_ a quantity of a product for a certain time limit.  
/// Returns an error if the product is not in the stock or if the quantity asked is not available.  
/// The quantity is represented by a `usize`, the product with a `String` and the time limit with a `usize`.
pub struct ReserveProduct {
    pub product: String,
    pub quantity: usize,
    pub time_limit: usize,
}

impl Handler<ReserveProduct> for Store {
    type Result = Result<(), Errors>;
    fn handle(&mut self, msg: ReserveProduct, _: &mut Context<Self>) -> Result<(), Errors> {
        if let Some(product) = self.stock.get_mut(&msg.product) {
            if product.available_quantity - product.reserved_quantity >= msg.quantity {
                // The quantity asked is reserved
                product.reserved_quantity += msg.quantity;

                // We notify the reserves manager that a new reserve was made
                let reserve = format!("{},{},{}", msg.product, msg.quantity, msg.time_limit);
                match self.reserve_sender.try_send(reserve) {
                    Ok(_) => {}
                    Err(_) => eprintln!(
                        "[STORE] Error a la hora de enviar la nueva reserva al 'reserves_manager'"
                    ),
                }
                Ok(())
            } else {
                eprintln!("[STORE] No hay stock suficiente del producto.");
                Err(Errors::NotEnoughStockError)
            }
        } else {
            eprintln!("[STORE] No se encontro el producto en el stock.");
            Err(Errors::ProductNotFoundError)
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// Once reserved a product, we can _cancel_ the reservation and effectively subtract the reserved quantity from the stock.
pub struct DispatchProduct {
    pub product: String,
    pub quantity: usize,
    pub cancel_order: bool,
}

impl Handler<DispatchProduct> for Store {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: DispatchProduct, ctx: &mut Context<Self>) -> Result<(), Errors> {
        if msg.cancel_order {
            // We remove the reserved products
            if let Some(product) = self.stock.get_mut(&msg.product) {
                product.reserved_quantity -= msg.quantity;
            }

            // We tell ecom that the order was cancelled
            let answer = format!("CANCELLED,{},{}", msg.product, msg.quantity);
            let ans_msg = AnswerEcom { answer };
            ctx.notify(ans_msg);
            Ok(())
        } else if let Some(product) = self.stock.get_mut(&msg.product) {
            // We discount the products that have been dispatched
            product.reserved_quantity -= msg.quantity;
            product.available_quantity -= msg.quantity;

            // We tell ecom that the order was approved
            let answer = format!("APPROVED,{},{}", msg.product, msg.quantity);
            let ans_msg = AnswerEcom { answer };
            ctx.notify(ans_msg);
            Ok(())
        } else {
            Err(Errors::ProductNotFoundError)
        }
    }
}

// ------------------------ STATE CHECKING PURPOSE MESSAGES ------------------------
#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// This message is used to _show the state_ of the store in the console.
pub struct ShowState;

impl Handler<ShowState> for Store {
    type Result = Result<(), Errors>;

    fn handle(&mut self, _: ShowState, _: &mut Context<Self>) -> Result<(), Errors> {
        for (product, product_stock) in &self.stock {
            println!(
                "Product: [{product}] has a quantity of [{}]  and has [{}] reserved.",
                product_stock.available_quantity, product_stock.reserved_quantity
            );
        }
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// This message is used to _share the state_ of the store with the coordinator through the message _AnswerEcom_.
pub struct ShareStock;

impl Handler<ShareStock> for Store {
    type Result = Result<(), Errors>;

    fn handle(&mut self, _: ShareStock, ctx: &mut Context<Self>) -> Result<(), Errors> {
        for (product, product_stock) in &self.stock {
            let prod_str = format!("STOCK,{},{}", product, product_stock.available_quantity);
            ctx.notify(AnswerEcom { answer: prod_str });
        }
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// This message is used to _share the state_ of the store with the coordinator through the message _AnswerEcom_.
pub struct NewEcomHandler {
    pub stream: TcpStream,
    pub ecom_id: String,
}

impl Handler<NewEcomHandler> for Store {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: NewEcomHandler, store_ctx: &mut Context<Self>) -> Result<(), Errors> {
        let ecom_addr = EcomHandler::create(|ctx| {
            let (read, write_half) = split(msg.stream);
            EcomHandler::add_stream(LinesStream::new(BufReader::new(read).lines()), ctx);
            let write = Some(write_half);
            EcomHandler {
                ecom: write,
                ecom_id: msg.ecom_id.clone(),
                store: store_ctx.address(),
            }
        });

        self.active_ecoms.insert(msg.ecom_id, ecom_addr);
        Ok(())
    }
}

// ------------------------ CHANGE STATE PURPOSE MESSAGES ------------------------

#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
/// This message is used to _remove_ an ecom end point from the store. More specifically, it is used when an ecom disconnects
/// from the network and also when a new leader is elected.
pub struct NewLeader {
    pub ecom_id: String,
}

impl Handler<NewLeader> for Store {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: NewLeader, ctx: &mut Context<Self>) -> Result<(), Errors> {
        let new_ecom_id =
            <usize as FromStr>::from_str(&msg.ecom_id).map_err(|_| Errors::CouldNotParse)?;

        self.leader = new_ecom_id;
        ctx.notify(ShareStock);

        Ok(())
    }
}

// ------------------------ TEST PURPOSE MESSAGES ------------------------

pub struct _GetStock;
/// Returns the stock of the store (hashmap)
impl Message for _GetStock {
    type Result = Result<HashMap<String, ProductStock>, String>;
}

impl Handler<_GetStock> for Store {
    type Result = Result<HashMap<String, ProductStock>, String>;

    fn handle(
        &mut self,
        _: _GetStock,
        _: &mut Self::Context,
    ) -> Result<HashMap<String, ProductStock>, String> {
        Ok(self.stock.clone())
    }
}

impl Clone for LocalProductOrder {
    fn clone(&self) -> Self {
        LocalProductOrder {
            product: self.product.clone(),
            quantity: self.quantity,
        }
    }
}
