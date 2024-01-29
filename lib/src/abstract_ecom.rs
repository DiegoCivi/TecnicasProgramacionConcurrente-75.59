extern crate actix;

use std::str::FromStr;

use actix::dev::ContextFutureSpawner;
use actix::{fut::wrap_future, Actor, Addr, Context, Handler, Message, StreamHandler};
use actix::{ActorContext, ActorFutureExt};
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;

use crate::coordinator::{ChangeLeader, CoordElection, EcomDisconnected};
use crate::ecom::election_from_vec;
use crate::{
    coordinator::{Coordinator, NewOrder},
    ecom::vec_from_election_msg,
    errors::Errors,
};

/// The actor that manages the connection between different ecommerces.
pub struct AbstractEcom {
    pub id: usize,
    pub write: Option<WriteHalf<TcpStream>>,
    pub coord: Addr<Coordinator>,
}

impl Actor for AbstractEcom {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        println!("¡AbstractEcom is alive!");
    }
}

/// This handler receives messages sent to the ecommerce from other ecommerces.
/// It handles the different supported messages between ecommerces.
impl StreamHandler<Result<String, std::io::Error>> for AbstractEcom {
    fn handle(&mut self, read: Result<String, std::io::Error>, _: &mut Self::Context) {
        if let Ok(line) = read {
            let split: Vec<&str> = line.split(',').collect();
            match split[0] {
                "ORDER" => {
                    let order = format!("{},{}", split[1], split[2]);
                    let _ = self.coord.try_send(NewOrder {
                        order,
                        visited_stores: vec![],
                    });
                }
                "LEADER" => {
                    let id =
                        <usize as FromStr>::from_str(split[1]).map_err(|_| Errors::CouldNotParse);
                    if let Ok(new_leader_id) = id {
                        let _ = self.coord.try_send(ChangeLeader { new_leader_id });
                    }
                }
                "ELECTION" => {
                    let visited = vec_from_election_msg(split[1].to_owned());
                    let _ = self.coord.try_send(CoordElection {
                        visited,
                        ecom_id: self.id + 1,
                    });
                }
                _ => {
                    println!(
                        "[ECOM_TASK] Se recibio un mensaje desconocido desde un ecommerce: [{}]",
                        line
                    )
                }
            }
        }
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        println!("[ABSTRACT_ECOM_{}] ECOM {} DISCONNECTED", self.id, self.id);
        let _ = self.coord.try_send(EcomDisconnected { ecom_id: self.id });
        ctx.stop();
    }
}

/// Sends an order to the other ecommerce.
#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
pub struct SendOrder {
    pub order: String,
}

impl Handler<SendOrder> for AbstractEcom {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: SendOrder, ctx: &mut Self::Context) -> Result<(), Errors> {
        let msg = format!("ORDER,{}\n", msg.order);
        let mut write_half = self
            .write
            .take()
            .expect("No debería poder llegar otro mensaje antes de que vuelva por usar ctx.wait");
        wrap_future::<_, Self>(async move {
            let _ = write_half.write_all(msg.as_bytes()).await;
            write_half
        })
        .map(|write, this, _| this.write = Some(write))
        .wait(ctx);
        Ok(())
    }
}

/// This message is used to notify the next ecommerce in the ring thath an election is being held
#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
pub struct Election {
    pub visited: Vec<usize>,
}

impl Handler<Election> for AbstractEcom {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: Election, ctx: &mut Self::Context) -> Result<(), Errors> {
        let election_msg = election_from_vec(msg.visited, self.id.to_string());
        let mut write_half = self
            .write
            .take()
            .expect("No debería poder llegar otro mensaje antes de que vuelva por usar ctx.wait");
        wrap_future::<_, Self>(async move {
            let _ = write_half.write_all(election_msg.as_bytes()).await;
            write_half
        })
        .map(|write, this, _| this.write = Some(write))
        .wait(ctx);

        Ok(())
    }
}

/// After receiving the new leader from the coordinator, this message notifies the other ecommerce
/// who is the new leader.
#[derive(Message)]
#[rtype(result = "Result<(), Errors>")]
pub struct NewLeader2 {
    pub new_leader_id: usize,
}

impl Handler<NewLeader2> for AbstractEcom {
    type Result = Result<(), Errors>;

    fn handle(&mut self, msg: NewLeader2, ctx: &mut Self::Context) -> Result<(), Errors> {
        let msg = format!("LEADER,{}\n", msg.new_leader_id);
        let mut write_half = self
            .write
            .take()
            .expect("No debería poder llegar otro mensaje antes de que vuelva por usar ctx.wait");
        wrap_future::<_, Self>(async move {
            let _ = write_half.write_all(msg.as_bytes()).await;
            write_half
        })
        .map(|write, this, _| this.write = Some(write))
        .wait(ctx);

        Ok(())
    }
}
