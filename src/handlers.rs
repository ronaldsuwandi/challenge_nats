use io::ErrorKind::NotConnected;
use std::io;
use std::str::from_utf8;
use std::sync::atomic::Ordering::SeqCst;
use crate::commands::MainCommand::{Connect, Disconnect, InitClient};
use crate::commands::{ClientCommand, MainCommand};
use crate::parser::{ClientConnectOpts, ClientRequest};
use crate::server::Server;
use log::{debug, error, info, warn};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt, Error};
use tokio::net::TcpStream;

impl Server {
    pub async fn handle(&self, mut socket: TcpStream) {
        let mut req_buffer = [0; 96];
        let mut client_request = ClientRequest::new();

        if let Err(e) = self.handle_new_connection(&mut socket).await {
            error!("error handling connection: {}", e);
            return;
        }

        let client_id = self.client_id.fetch_add(1, SeqCst);

        let (tx, mut rx) = tokio::sync::mpsc::channel::<MainCommand>(100);

        if let Err(e) = self.main_tx.send(InitClient { client_id, tx }).await {
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
                            info!("input = {}", from_utf8(&req_buffer[0..n]).unwrap());

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
                            let response = format!("MSG {} {} {}\r\n", subject, subscription_id, msg_bytes.len());

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

    async fn handle_connect(&self, client_id: u32, socket: &mut TcpStream, client_connect_opts: ClientConnectOpts) -> Result<(), Error> {
        let verbose = client_connect_opts.verbose;
        if let Err(e) = self.main_tx.send(Connect { client_id, client_connect_opts}).await {
            error!("error sending to main channel: {}", e);
        }
        if verbose {
            socket.write_all(b"+OK\r\n").await?;
        }
        Ok(())
    }

    async fn handle_ping(&self, client_id: u32, socket: &mut TcpStream) -> Result<(), Error> {
        self.check_client_connected(client_id).await?;
        socket.write_all(b"PONG\r\n").await?;
        Ok(())
    }

    async fn handle_pub(&self, client_id: u32, subject: String, msg: String, socket: &mut TcpStream) -> Result<(), Error> {
        self.check_client_connected(client_id).await?;
        info!("publishing to {}", subject);

        if let Err(e) = self.main_tx.send(MainCommand::Publish { subject, msg }).await {
            error!("error sending to main channel: {}", e);
        }
        if self.check_client_verbose(client_id).await? {
            socket.write_all(b"+OK\n").await?;
        }
        Ok(())
    }

    async fn handle_sub(&self, client_id: u32, subject: String, subscription_id: String, socket: &mut TcpStream) -> Result<(), Error> {
        self.check_client_connected(client_id).await?;
        info!("client_id {} subscribing to {} (id: {})", client_id, subject, subscription_id);
        if let Err(e) = self.main_tx.send(MainCommand::Subscribe { subject, client_id, subscription_id }).await {
            error!("error sending to main channel: {}", e);
        }
        if self.check_client_verbose(client_id).await? {
            socket.write_all(b"+OK\n").await?;
        }
        Ok(())
    }

    async fn handle_unsub(&self, client_id: u32, subscription_id: String, socket: &mut TcpStream) -> Result<(), Error> {
        self.check_client_connected(client_id).await?;
        info!("client_id {} unsubscribing to {} ", client_id, subscription_id);
        if let Err(e) = self.main_tx.send(MainCommand::Unsubscribe { client_id, subscription_id }).await {
            error!("error sending to main channel: {}", e);
        }
        if self.check_client_verbose(client_id).await? {
            socket.write_all(b"+OK\n").await?;
        }
        Ok(())
    }

    async fn handle_commands(&self, cmd: ClientCommand, socket: &mut TcpStream, client_id: u32) {
        let cmd_result = match cmd {
            ClientCommand::Noop => { Ok(()) }
            ClientCommand::Connect(opts) => self.handle_connect(client_id, socket, opts).await,
            ClientCommand::Pub { subject, msg } => self.handle_pub(client_id, subject, msg, socket).await,
            ClientCommand::Sub { subject, id } => self.handle_sub(client_id, subject, id, socket).await,
            ClientCommand::Unsub { id } => self.handle_unsub(client_id, id, socket).await,
            ClientCommand::Ping => self.handle_ping(client_id, socket).await,
            ClientCommand::Pong => { Ok(()) }
        };

        if let Some(e) = cmd_result.err() {
            error!("error: {:?}", e);
            let _ = socket.write_all(b"-ERR\n").await;
        }
    }

    async fn check_client_connected(&self, client_id: u32) -> Result<(), Error> {
        let clients_tx = self.clients_tx.read().await;
        clients_tx.get(&client_id)
            .ok_or(Error::new(NotConnected, "client is not connected"))
            .and_then(|(_, client_state)| {
                if !client_state.connected {
                    return Err(Error::new(NotConnected, "client is not connected"));
                }
                Ok(())
            })
    }

    async fn check_client_verbose(&self, client_id: u32) -> Result<bool, Error> {
        let clients_tx = self.clients_tx.read().await;
        if let Some((_, client_state)) = clients_tx.get(&client_id) {
            Ok(client_state.verbose)
        } else {
            Ok(false)
        }
    }
}
