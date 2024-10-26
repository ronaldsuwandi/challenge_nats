use std::error::Error;
use log::{debug, error, info};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use crate::parser::{ClientRequest, Command};
use serde_json::json;

pub struct Server {}

impl Server {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn handle(&self, mut socket: TcpStream) -> Result<(), Box<dyn Error>> {
        let mut req_buffer = [0; 4096];
        let (mut socket_rd, mut socket_wr) = socket.split();
        let mut client_request = ClientRequest::new();

        let local_addr = socket_rd.local_addr().expect("Failed to get local address");
        let peer_addr = socket_rd.peer_addr().expect("Failed to get peer address");

        let info = json!({
            "hostname": local_addr.ip().to_string(),
            "port": local_addr.port(),
            "client_ip": peer_addr.ip().to_string(),
        }); 

        let response = format!("INFO {}\n", info);
        
        socket_wr.write_all(response.as_bytes()).await?;

        // loop here so we can stream the input (large input)
        loop {
            // read input
            match socket_rd.read(&mut req_buffer).await {
                Ok(0) => {
                    debug!("Input stream closed");
                    break;
                }
                Ok(n) => {
                    let cmd = client_request.parse(&req_buffer[0..n])?;
                    info!("command={:?}", cmd);
                    match cmd {
                        Command::Noop => {}
                        Command::Connect(_) => {
                            socket_wr.write_all(b"+OK\n").await?;
                        }
                        Command::Pub { .. } => {}
                        Command::Sub { .. } => {}
                        Command::Ping => {
                            socket_wr.write_all(b"PONG\n").await?;
                        }
                        Command::Pong => {}
                    }
                }
                Err(e) => {
                    error!("error: {}", e);
                }
            };
        }


        Ok(())
    }
}