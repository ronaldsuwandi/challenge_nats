use crate::server::Server;
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc::Sender;
use tokio::sync::{RwLockWriteGuard};

#[derive(Debug, PartialEq, Eq)]
pub enum ClientCommand {
    Noop,
    Connect(String),
    Pub { subject: String, msg: String },
    Sub { subject: String, id: String },
    Ping,
    Pong,
}

#[derive(Debug)]
pub enum MainCommand {
    Noop,
    Connect { client_id: u32, tx: Sender<MainCommand> },
    Disconnect { client_id: u32 },
    Subscribe { client_id: u32, subject: String, subscription_id: String },
    Publish { subject: String, msg: String },
    PublishedMessage { subject: String, msg: String, subscription_id: String },
    ShutDown,
}

impl Server {
    pub async fn process_connect(&self, client_id: u32, tx: Sender<MainCommand>) {
        let mut clients_tx = self.clients_tx.write().await;
        clients_tx.insert(client_id, tx);
        debug!("client id {} connected", client_id);
        debug!("clients connected: {}", clients_tx.len());
    }

    pub async fn process_disconnect(&self, client_id: u32) {
        let mut clients_tx = self.clients_tx.write().await;
        clients_tx.remove(&client_id);

        // TODO do we need to update atomically? having slight inconsistency is fine right? some warning message but is this staff quality?
        // cid->sid
        //c1 -> [s1]
        //c2 -> [s1,s2]
        // subj->sids
        //s -> [s1,s2]
        // sid->subj
        //s1 -> [s]
        //s2 -> [s]
        // sid->cid
        //s1 -> [c1,c2]
        //s2 -> [c2]
        
        // TODO for now just focus on cid
        let mut subscription_id_to_client_id = self.subscription_id_to_client_id.write().await;
        let mut client_id_to_subscription_id = self.client_id_to_subscription_id.write().await;
        if let Some(subscription_ids) = client_id_to_subscription_id.get(&client_id) {
            for subscription_id in subscription_ids {
                if let Some(client_ids) = subscription_id_to_client_id.get_mut(subscription_id) {
                    client_ids.remove(&client_id);
                }
            }
        }
        client_id_to_subscription_id.remove(&client_id);

        debug!("client id {} disconnected", client_id);
        debug!("clients connected: {}", clients_tx.len());
    }

    pub async fn process_subscribe(&self, client_id: u32, subject: String, subscription_id: String) {
        let _ = self.lock.lock().await;
        // update 4 maps atomically
        let subscription_subject_to_id = self.subscription_subject_to_id.write().await;
        let subscription_id_to_subject = self.subscription_id_to_subject.write().await;
        let subscription_id_to_client_id = self.subscription_id_to_client_id.write().await;
        let client_id_to_subscription_id = self.client_id_to_subscription_id.write().await;
        insert_to_subscription_map(subscription_subject_to_id, subject.clone(), subscription_id.clone());
        insert_to_subscription_map(subscription_id_to_subject, subscription_id.clone(), subject.clone());
        insert_to_subscription_map(subscription_id_to_client_id, subscription_id.clone(), client_id);
        insert_to_subscription_map(client_id_to_subscription_id, client_id, subscription_id.clone());
    }

    pub async fn process_publish(&self, subject: String, msg: String) {
        info!("process_publish");
        let subscription_subject_to_id = self.subscription_subject_to_id.read().await;
        let subscription_id_to_client_id = self.subscription_id_to_client_id.read().await;
        let clients_tx = self.clients_tx.read().await;

        match subscription_subject_to_id.get(&subject) {
            Some(subscription_ids) => {
                for subscription_id in subscription_ids {
                    match subscription_id_to_client_id.get(subscription_id) {
                        Some(client_ids) => {
                            for client_id in client_ids {
                                match clients_tx.get(client_id) {
                                    Some(tx) => {
                                        let client_id = *client_id;
                                        let client_tx = tx.clone();
                                        let subscription_id = subscription_id.clone();
                                        let subject = subject.clone();
                                        let msg = msg.clone();
                                        info!("publishing message to client id {} for subject {}", client_id, subject);
                                        tokio::spawn(async move {
                                            if let Err(e) = client_tx.send(MainCommand::PublishedMessage { subject, msg, subscription_id }).await {
                                                error!("error sending message to client {}: {}", client_id, e);
                                            }
                                        });
                                    }
                                    None => {
                                        warn!("unable to find client tx for client id {}", client_id);
                                    }
                                }
                            }
                        }
                        None => {
                            warn!("unable to find client ids for subscription id {}", subscription_id);
                        }
                    }
               }
            }
            None => {
                warn!("unable to find subscription id for subject: {}", subject);
            }
        }
    }

    pub async fn process_shutdown(&self) {
        info!("process shutdown");
        if let Ok(clients_tx) = self.clients_tx.try_read() {
            for (client_id, tx) in clients_tx.iter() {
                if let Err(e) = tx.try_send(MainCommand::ShutDown) {
                    warn!("error sending shutdown message to client {}: {}", client_id, e);
                }
            }
        }
    }
}

fn insert_to_subscription_map<K,V>(mut map: RwLockWriteGuard<HashMap<K, HashSet<V>>>, key: K, value: V) where
K: Eq + std::hash::Hash, V: Eq + std::hash::Hash
{
    match map.get_mut(&key) {
        Some(values) => {
            values.insert(value);
        }
        None => {
            let values = HashSet::from([value]);
            map.insert(key, values);
        }
    }
}
