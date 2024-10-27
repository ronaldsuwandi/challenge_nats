use crate::commands::Command;
use crate::parser::ClientRequest;
use crate::server::Server;
use log::{debug, error, info};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt, Error};
use tokio::net::TcpStream;

impl Server {
    pub async fn handle(&self, mut socket: TcpStream) {
        let mut req_buffer = [0; 4096];
        let mut client_request = ClientRequest::new();

        if let Err(e) = self.handle_new_connection(&mut socket).await {
            error!("error handling connection: {}", e);
            return;
        }

        // loop here so we can stream the input (large input)
        loop {
            // read input
            match socket.read(&mut req_buffer).await {
                Ok(0) => {
                    debug!("Input stream closed");
                    break;
                }
                Ok(n) => {
                    match client_request.parse(&req_buffer[0..n]) {
                        Ok(cmd) => {
                            info!("command={:?}", cmd);
                            self.handle_commands(cmd, &mut socket).await;
                        }

                        Err(e) => {
                            error!("error parsing command: {}", e);
                            let _ = socket.write_all(b"-ERR\n").await;
                        }
                    };

                }
                Err(e) => {
                    error!("error: {}", e);
                }
            };
        }

    }

    pub async fn handle_new_connection(&self, socket: &mut TcpStream) -> Result<(), Error> {
        let local_addr = socket.local_addr().expect("Failed to get local address");
        let peer_addr = socket.peer_addr().expect("Failed to get peer address");

        let info = json!({
            "hostname": local_addr.ip().to_string(),
            "port": local_addr.port(),
            "client_ip": peer_addr.ip().to_string(),
        });

        let response = format!("INFO {}\n", info);

        socket.write_all(response.as_bytes()).await?;
        Ok(())
    }

    pub async fn handle_connect(&self, socket: &mut TcpStream) -> Result<(), Error> {
        socket.write_all(b"+OK\n").await?;
        Ok(())
    }

    pub async fn handle_ping(&self, socket: &mut TcpStream) -> Result<(), Error> {
        socket.write_all(b"PONG\n").await?;
        Ok(())
    }

    async fn handle_commands(&self, cmd: Command, socket: &mut TcpStream) {
        let cmd_result = match cmd {
            // Command::Noop => {}
            Command::Connect(_) => self.handle_connect(socket).await,
            // Command::Pub { .. } => {}
            // Command::Sub { .. } => {}
            Command::Ping => self.handle_ping(socket).await,
            // Command::Pong => {}
            _ => { Ok(()) }
        };

        if let Some(e) = cmd_result.err() {
            error!("error: {:?}", e);
            let _ = socket.write_all(b"-ERR\n").await;
        }

    }
}
