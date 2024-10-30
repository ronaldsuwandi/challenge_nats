use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32};
use log::{info, warn};
use tokio::sync;
use tokio::sync::{RwLock};
use tokio::sync::mpsc::{Receiver, Sender};
use crate::commands::MainCommand;

pub struct Server {
    pub client_id: AtomicU32,

    pub subscription_subject_to_id: RwLock<HashMap<String, HashSet<String>>>,
    pub subscription_id_to_subject: RwLock<HashMap<String, HashSet<String>>>,

    pub subscription_id_to_client_id: RwLock<HashMap<String, HashSet<u32>>>,
    pub client_id_to_subscription_id: RwLock<HashMap<u32, HashSet<String>>>,

    pub clients_tx: RwLock<HashMap<u32, (Sender<MainCommand>, ClientState)>>,
    pub main_tx: Sender<MainCommand>,
}

#[derive(Default)]
pub struct ClientState {
    pub connected: bool,
    pub verbose: bool,
}

impl Server {
    pub fn new() -> (Server, Receiver<MainCommand>) {
        let (tx, rx) = sync::mpsc::channel(100);

        (Server {
            client_id: AtomicU32::new(0),
            subscription_subject_to_id: RwLock::new(HashMap::new()),
            subscription_id_to_subject: RwLock::new(HashMap::new()),
            subscription_id_to_client_id: RwLock::new(HashMap::new()),
            client_id_to_subscription_id: RwLock::new(HashMap::new()),
            clients_tx: RwLock::new(HashMap::new()),
            main_tx: tx,
        }, rx)
    }

    pub async fn process_rx(&self, mut rx: Receiver<MainCommand>) {
        while let Some(command) = rx.recv().await {
            info!("received command: {:?}", command);
            match command {
                MainCommand::Noop => {}
                MainCommand::InitClient { client_id, tx } => self.process_init_client(client_id, tx).await,
                MainCommand::Connect { client_id, client_connect_opts } => self.process_connect(client_id, client_connect_opts).await,
                MainCommand::Disconnect { client_id } => self.process_disconnect(client_id).await,
                MainCommand::Subscribe { client_id, subject, subscription_id } => self.process_subscribe(client_id, subject, subscription_id).await,
                MainCommand::Unsubscribe { client_id, subscription_id } => self.process_unsubscribe(client_id, subscription_id).await,
                MainCommand::Publish { subject, msg } => self.process_publish(subject, msg).await,
                MainCommand::PublishedMessage { .. } => warn!("server received published message"),
                MainCommand::ShutDown => {
                    self.process_shutdown().await;
                    break;
                }
            }
        }
        info!("stopping process_rx");
    }
}
