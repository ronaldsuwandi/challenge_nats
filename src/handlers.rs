use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::sync::{Arc};
use crate::commands::Command;
use crate::parser::ClientRequest;
use crate::server::Server;
use log::{debug, error, info};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt, Error};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock};

impl Server {
    async fn update_clients_map(&self, socket: Arc<Mutex<TcpStream>>) -> u32 {
        let mut clients_map = self.clients_map.write().await;
        let new_id = self.client_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        clients_map.insert(new_id, socket);

        new_id
    }

    async fn remove_client(&self, client_id: u32) {

        let _ = self.lock.lock();

        debug!("cleaning up clients_map for client id {}", client_id);
        let mut clients_map = self.clients_map.write().await;
        clients_map.remove(&client_id);

        debug!("cleaning up client_id_to_subscriptions for client id {}", client_id);
        let mut client_id_to_subscriptions = self.client_id_to_subscriptions.write().await;
        if let Some(subscriptions) = client_id_to_subscriptions.get(&client_id) {
            let subscriptions = subscriptions.read().await;
            debug!("client id {} subscriptions to clean up", subscriptions.len());

            let subscription_map = self.subscription_map.read().await;

            debug!("cleaning up subscriptions for client id {}", client_id);
            for subscription in subscriptions.iter() {
                if let(Some(client_ids)) = subscription_map.get(subscription) {
                    let mut client_ids = client_ids.write().await;
                    client_ids.remove(&client_id);
                }
            }
        }
        client_id_to_subscriptions.remove(&client_id);
    }

    pub async fn handle(&self, mut socket: TcpStream) {
        let mut req_buffer = [0; 4096];
        let mut client_request = ClientRequest::new();

        if let Err(e) = self.handle_new_connection(&mut socket).await {
            error!("error handling connection: {}", e);
            return;
        }

        let socket_arc = Arc::new(Mutex::new(socket));
        let client_id = self.update_clients_map(socket_arc.clone()).await;

        info!("client id: {}", client_id);

        {
            let tmp = self.clients_map.read().await;
            info!("map: {:?}", tmp);
        }

        let socket_arc = socket_arc.clone();
        let mut socket = socket_arc.lock().await;

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
            };
        }

        info!("client id {} disconnected", client_id);
        self.remove_client(client_id).await;

        let tmp = self.clients_map.read().await;
        info!("map after: {:?}", tmp);
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

        let sub_map = self.subscription_map.read().await;
        debug!("subscription_map lock obtained");
        if let Some(client_ids) = sub_map.get(&subject) {
            let client_ids = client_ids.read().await;
            debug!("client_ids lock obtained");

            let clients_map = self.clients_map.read().await;
            debug!("client_map lock obtained");

            let msg_arc = Arc::new(msg);

            for client_id in client_ids.iter() {
                let client_id = *client_id;
                debug!("client id: {}", client_id);
                if let Some(socket) = clients_map.get(&client_id) {
                    debug!("cloning socket and msg arc");
                    let socket = socket.clone();
                    let msg_arc = msg_arc.clone();
                    debug!("spawning task");
                    tokio::spawn(async move {
                        debug!("-> socket lock attempt");
                        let mut socket = socket.lock().await;
                        debug!("socket lock obtained. writing to socket");
                        if let Err(e) = socket.write_all(msg_arc.as_bytes()).await {
                            error!("error publishing message to client id: {}", client_id);
                        }
                    });
                }
            }
        } else {
            debug!("no subscribers for subject: {}", subject);
        }

        socket.write_all(b"+OK\n").await?;
        Ok(())
    }

    async fn handle_sub(&self, subject: String, id: String, client_id: u32, socket: &mut TcpStream) -> Result<(), Error> {
        info!("client_id {} subscribing to {} (id: {})", client_id, subject, id);

        let lock = self.lock.lock().await;

        // atomic
        let mut sub_map = self.subscription_map.write().await;
        match sub_map.get_mut(&subject) {
            Some(subscriptions) => {
                let mut subscriptions = subscriptions.write().await;
                subscriptions.insert(client_id);
            },
            None => {
                let subscriptions = HashSet::from([client_id]);
                sub_map.insert(subject.clone(), RwLock::new(subscriptions));
            }
        }

        let mut client_to_sub_map = self.client_id_to_subscriptions.write().await;
        match client_to_sub_map.get_mut(&client_id) {
            Some(subjects) => {
                let mut subjects = subjects.write().await;
                subjects.insert(subject);
            },
            None => {
                let subjects = HashSet::from([subject]);
                client_to_sub_map.insert(client_id, RwLock::new(subjects));
            }
        }
        drop(lock);
        Ok(())
    }

    async fn handle_commands(&self, cmd: Command, socket: &mut TcpStream, client_id: u32) {
        let cmd_result = match cmd {
            Command::Noop => { Ok(()) }
            Command::Connect(_) => self.handle_connect(socket).await,
            Command::Pub { subject, msg } => self.handle_pub(subject, msg, socket).await,
            Command::Sub { subject, id } => self.handle_sub(subject, id, client_id, socket).await,
            Command::Ping => self.handle_ping(socket).await,
            Command::Pong => { Ok(()) }
            _ => { Err(Error::new(ErrorKind::Other, "unknown command")) }
        };

        if let Some(e) = cmd_result.err() {
            error!("error: {:?}", e);
            let _ = socket.write_all(b"-ERR\n").await;
        }
    }
}
