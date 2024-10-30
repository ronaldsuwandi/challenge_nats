use crate::commands::MainCommand::{Connect, Disconnect};
use crate::commands::{ClientCommand, MainCommand};
use crate::parser::ClientRequest;
use crate::server::Server;
use log::{debug, error, info, warn};
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

        let client_id = self.client_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let (tx, mut rx) = tokio::sync::mpsc::channel::<MainCommand>(100);

        if let Err(e) = self.main_tx.send(Connect { client_id, tx }).await {
            error!("error sending to main channel: {}", e);
            return;
        }

        // loop here so we can stream the input (large input)
        loop {
            tokio::select! {
                // read input
                socket_result = socket.read(&mut req_buffer) => {
                    match socket_result {
                        Ok(0) => {
                            debug!("Input stream closed");
                            break;
                        }
                        Ok(n) => {
                            match client_request.parse(&req_buffer[0..n]) {
                                Ok(cmd) => {
                                    info!("command={:?}", cmd);
                                    self.handle_commands(cmd, &mut socket, client_id).await;
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
                    }

                }

                // read from main command channel
                Some(cmd) = rx.recv() => {
                    match cmd {
                       MainCommand::PublishedMessage{subject, msg, subscription_id} => {
                            let mut buf: Vec<u8> = vec![];
                            let msg_bytes = msg.as_bytes();
                            let response = format!("MSG {} {} {}\n", subject, subscription_id, msg_bytes.len());

                            buf.extend_from_slice(response.as_bytes());
                            buf.extend_from_slice(msg_bytes);
                            buf.push(b'\n');

                            if let Err(e) = socket.write_all(buf.as_slice()).await {
                                error!("error writing to socket: {}", e);
                            }
                            debug!("publish message for subject {}", subject);
                        },
                        MainCommand::ShutDown => {
                            info!("shutting down client {}", client_id);
                            // skip sending disconnect command
                            return;
                        },
                        _ => {
                            warn!("received command on the client side, should be PublishedMessage or ShutDown only: {:?}", cmd);
                        }
                    };
                }
            }
        }

        if let Err(e) = self.main_tx.send(Disconnect { client_id }).await {
            error!("error sending to main channel: {}", e);
        }
    }

    async fn handle_new_connection(&self, socket: &mut TcpStream) -> Result<(), Error> {
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

    async fn handle_connect(&self, socket: &mut TcpStream) -> Result<(), Error> {
        socket.write_all(b"+OK\n").await?;
        Ok(())
    }

    async fn handle_ping(&self, socket: &mut TcpStream) -> Result<(), Error> {
        socket.write_all(b"PONG\n").await?;
        Ok(())
    }

    async fn handle_pub(&self, subject: String, msg: String, socket: &mut TcpStream) -> Result<(), Error> {
        info!("publishing to {}", subject);

        if let Err(e) = self.main_tx.send(MainCommand::Publish { subject, msg }).await {
            error!("error sending to main channel: {}", e);
        }

        socket.write_all(b"+OK\n").await?;
        Ok(())
    }

    async fn handle_sub(&self, subject: String, subscription_id: String, client_id: u32, socket: &mut TcpStream) -> Result<(), Error> {
        info!("client_id {} subscribing to {} (id: {})", client_id, subject, subscription_id);
        if let Err(e) = self.main_tx.send(MainCommand::Subscribe { subject, client_id, subscription_id }).await {
            error!("error sending to main channel: {}", e);
        }
        socket.write_all(b"+OK\n").await?;
        Ok(())
    }
    
    async fn handle_unsub(&self, subscription_id: String, client_id: u32, socket: &mut TcpStream) -> Result<(), Error> {
        info!("client_id {} unsubscribing to {} ", client_id, subscription_id);
        if let Err(e) = self.main_tx.send(MainCommand::Unsubscribe { client_id, subscription_id }).await {
            error!("error sending to main channel: {}", e);
        }
        socket.write_all(b"+OK\n").await?;
        Ok(())
    }
    
    async fn handle_commands(&self, cmd: ClientCommand, socket: &mut TcpStream, client_id: u32) {
        let cmd_result = match cmd {
            ClientCommand::Noop => { Ok(()) }
            ClientCommand::Connect(_) => self.handle_connect(socket).await,
            ClientCommand::Pub { subject, msg } => self.handle_pub(subject, msg, socket).await,
            ClientCommand::Sub { subject, id } => self.handle_sub(subject, id, client_id, socket).await,
            ClientCommand::Unsub { id } => self.handle_unsub(id, client_id, socket).await,
            ClientCommand::Ping => self.handle_ping(socket).await,
            ClientCommand::Pong => { Ok(()) }
        };

        if let Some(e) = cmd_result.err() {
            error!("error: {:?}", e);
            let _ = socket.write_all(b"-ERR\n").await;
        }
    }
}
