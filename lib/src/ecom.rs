use std::str::FromStr;

use crate::{
    coordinator::{Coordinator, NewEcom},
    errors::Errors,
};
use actix::Addr;
use tokio::io::BufReader;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_stream::{wrappers::LinesStream, StreamExt};

/// Function to spawn a task for each ecommerce connection attempt. It receives a vector of tuples containing the ip and id of each ecommerce.
/// It also receives the coordinator address, the address of the current node of the network and its id.
pub async fn ecom_network(
    ips_ecoms: Vec<(String, String)>,
    coord: Addr<Coordinator>,
    my_id: String,
) -> Result<(), Errors> {
    for (ip, id) in ips_ecoms {
        let coord_clone = coord.clone();
        let my_id_clone = my_id.clone();

        // Spawn a task for each connection attempt
        tokio::spawn(async move {
            let ecom_connection = TcpStream::connect(&ip).await;
            if let Ok(mut stream) = ecom_connection {
                let id_msg = format!("{}\n", my_id_clone);
                let _ = stream.write(&id_msg.into_bytes()).await;
                let _ = coord_clone.send(NewEcom { id, stream }).await;
            }
        });
    }
    Ok(())
}

/// This function accepts connections from ecommerces and spawns a task for each one.
pub async fn ecom_connection_listener(
    addr: String,
    coord: Addr<Coordinator>,
) -> Result<(), Errors> {
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|_| Errors::ConnectionError)?;

    while let Ok(tuple) = listener.accept().await {
        let mut lines = LinesStream::new(BufReader::new(tuple.0).lines());
        let id = lines.next().await;
        if let Some(Ok(ecom_id)) = id {
            let stream = lines.into_inner().into_inner().into_inner();
            let _ = coord
                .send(NewEcom {
                    id: ecom_id,
                    stream,
                })
                .await;
        }
    }
    Ok(())
}

/// This function is responsible for creating a vec with the visited ecoms received in a String
pub fn vec_from_election_msg(msg: String) -> Vec<usize> {
    let mut vec: Vec<usize> = vec![];
    let ids = msg.split('/');
    for id in ids {
        let temp = <usize as FromStr>::from_str(id).map_err(|_| Errors::CouldNotParse);
        if let Ok(n) = temp {
            vec.push(n);
        }
    }

    vec
}

/// Creates the election message from the election message received by the previous ecom in the ring
pub fn election_from_vec(visited: Vec<usize>, my_id: String) -> String {
    let mut msg = "ELECTION,".to_string();
    for ecom in visited {
        msg = msg + &ecom.to_string();
        msg += "/";
    }
    msg += &my_id;
    msg += "\n";
    msg
}
