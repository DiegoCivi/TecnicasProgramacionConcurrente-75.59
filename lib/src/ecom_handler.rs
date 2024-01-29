use crate::errors::Errors;
use crate::store::Store;
use crate::store::{NewLeader, ReserveProduct};
use actix::dev::ContextFutureSpawner;
use actix::fut::{wrap_future, ActorFutureExt};
use actix::{Actor, ActorContext, Addr, Context, Handler, Message, StreamHandler};
use std::str::FromStr;
use tokio::io::AsyncWriteExt;
use tokio::{io::WriteHalf, net::TcpStream};

extern crate actix;

/// Will become the actor that is responsible for the connection between ecommerces and stores,
/// from the side of the stores.
pub struct EcomHandler {
    pub ecom: Option<WriteHalf<TcpStream>>,
    pub ecom_id: String,
    pub store: Addr<Store>,
}

impl Actor for EcomHandler {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("¡EcomHandler is alive with id [{}]!", self.ecom_id);
    }
}

/// Handles the supported messages received from the ecommerce.
impl StreamHandler<Result<String, std::io::Error>> for EcomHandler {
    fn handle(&mut self, read: Result<String, std::io::Error>, _: &mut Self::Context) {
        if let Ok(line) = read {
            let split: Vec<&str> = line.split(',').collect();
            match split[0] {
                "LEADER" => {
                    let _ = self.store.try_send(NewLeader {
                        ecom_id: split[1].to_string(),
                    });
                }
                _ => {
                    let quantity =
                        <usize as FromStr>::from_str(split[1]).map_err(|_| Errors::CouldNotParse);
                    let time_limit =
                        <usize as FromStr>::from_str(split[2]).map_err(|_| Errors::CouldNotParse);
                    if quantity.is_err() || time_limit.is_err() {
                        eprintln!("No se pudo parsear la linea [{line}]", line = line);
                        return;
                    }

                    if let Ok(amount) = quantity {
                        if let Ok(time) = time_limit {
                            let reserved_prod = ReserveProduct {
                                product: split[0].to_string(),
                                quantity: amount,
                                time_limit: time,
                            };
                            if self.store.try_send(reserved_prod).is_err() {
                                eprintln!("[ONLINE_SALES] No se pudo enviar el pedido [{line}] a la store");
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Receives a String and sends it to the ecommerce
#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
pub struct Answer {
    pub answer: String,
}

impl Handler<Answer> for EcomHandler {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: Answer, ctx: &mut Self::Context) -> Result<(), Errors> {
        let mut write = self
            .ecom
            .take()
            .expect("No debería poder llegar otro mensaje antes de que vuelva por usar ctx.wait");
        wrap_future::<_, Self>(async move {
            write
                .write_all(format!("{}\n", msg.answer).as_bytes())
                .await
                .expect(&msg.answer);
            write
        })
        .map(|write, this, _| this.ecom = Some(write))
        .wait(ctx);
        Ok(())
    }
}

/// Stops the execution of the actor.
#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
pub struct Stop;

impl Handler<Stop> for EcomHandler {
    type Result = Result<(), Errors>;

    fn handle(&mut self, _: Stop, ctx: &mut Self::Context) -> Result<(), Errors> {
        ctx.stop();
        Ok(())
    }
}
