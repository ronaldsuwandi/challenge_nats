use crate::server::{ClientState, Server};
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLockWriteGuard;
use crate::parser::ClientConnectOpts;

#[derive(Debug, PartialEq, Eq)]
pub enum ClientCommand {
    Noop,
    Connect(ClientConnectOpts),
    Pub { subject: String, msg: String },
    Sub { subject: String, id: String },
    Unsub { id: String },
    Ping,
    Pong,
}

#[derive(Debug)]
pub enum MainCommand {
    Noop,
    InitClient { client_id: u32, tx: Sender<MainCommand> },
    Connect { client_id: u32, client_connect_opts: ClientConnectOpts },
    Disconnect { client_id: u32 },
    Subscribe { client_id: u32, subject: String, subscription_id: String },
    Unsubscribe { client_id: u32, subscription_id: String },
    Publish { subject: String, msg: String },
    PublishedMessage { subject: String, msg: String, subscription_id: String },
    ShutDown,
}

impl Server {
    pub async fn process_init_client(&self, client_id: u32, tx: Sender<MainCommand>) {
        let mut clients_tx = self.clients_tx.write().await;
        clients_tx.insert(client_id, (tx, ClientState::default()));
        debug!("client id {} initialised", client_id);
        debug!("clients connected: {}", clients_tx.len());
    }

    pub async fn process_connect(&self, client_id: u32, client_connect_opts: ClientConnectOpts) {
        let mut clients_tx = self.clients_tx.write().await;
        if let Some(pair) = clients_tx.get_mut(&client_id) {
            pair.1 = ClientState {
                connected: true,
                verbose: client_connect_opts.verbose
            }
        } else {
            error!("unable to process connect");
        }
        debug!("client id {} connected", client_id);
    }


    pub async fn process_disconnect(&self, client_id: u32) {
        let mut clients_tx = self.clients_tx.write().await;
        clients_tx.remove(&client_id);

        let mut lock = self.write_locks().await;
        if let Some(subscription_ids) = lock.client_id_to_subscription_id.get(&client_id) {
            for subscription_id in subscription_ids {
                if let Some(client_ids) = lock.subscription_id_to_client_id.get_mut(subscription_id) {
                    client_ids.remove(&client_id);

                    if !client_ids.is_empty() {
                        continue;
                    }

                    // clean up the rest if no more client connected to this id
                    if let Some(subjects) = lock.subscription_id_to_subject.get(subscription_id) {
                        for subject in subjects {
                            if let Some(subscription_ids) = lock.subscription_subject_to_id.get_mut(subject) {
                                subscription_ids.remove(subscription_id);
                            }
                        }
                        lock.subscription_id_to_subject.remove(subscription_id);
                    }
                }
            }
        }
        lock.client_id_to_subscription_id.remove(&client_id);

        debug!("client id {} disconnected", client_id);
        debug!("clients connected: {}", clients_tx.len());
    }

    pub async fn process_subscribe(&self, client_id: u32, subject: String, subscription_id: String) {
        let locks = self.write_locks().await;
        insert_to_subscription_map(locks.subscription_subject_to_id, subject.clone(), subscription_id.clone());
        insert_to_subscription_map(locks.subscription_id_to_subject, subscription_id.clone(), subject.clone());
        insert_to_subscription_map(locks.subscription_id_to_client_id, subscription_id.clone(), client_id);
        insert_to_subscription_map(locks.client_id_to_subscription_id, client_id, subscription_id.clone());
    }

    pub async fn process_unsubscribe(&self, client_id: u32, subscription_id: String) {
        let mut lock = self.write_locks().await;

        if let Some(subscription_ids) = lock.client_id_to_subscription_id.get_mut(&client_id) {
            subscription_ids.remove(&subscription_id);
        }
        if let Some(client_ids) = lock.subscription_id_to_client_id.get_mut(&subscription_id) {
            client_ids.remove(&client_id);

            if !client_ids.is_empty() {
                return;
            }
        }

        // clean up the rest if no more client connected to this id
        if let Some(subjects) = lock.subscription_id_to_subject.get(&subscription_id) {
            for subject in subjects {
                if let Some(subscription_ids) = lock.subscription_subject_to_id.get_mut(subject) {
                    subscription_ids.remove(&subscription_id);
                }
            }
           lock.subscription_id_to_subject.remove(&subscription_id);
        }
    }

    // ensure that locks are obtained in the same order
    async fn write_locks(&self) -> MapWriteLocks {
        let subscription_subject_to_id = self.subscription_subject_to_id.write().await;
        let subscription_id_to_subject = self.subscription_id_to_subject.write().await;
        let subscription_id_to_client_id = self.subscription_id_to_client_id.write().await;
        let client_id_to_subscription_id = self.client_id_to_subscription_id.write().await;
        MapWriteLocks {
            subscription_subject_to_id,
            subscription_id_to_subject,
            subscription_id_to_client_id,
            client_id_to_subscription_id
        }
    }

    pub async fn process_publish(&self, subject: String, msg: String) {
        info!("process_publish");
        let subscription_subject_to_id = self.subscription_subject_to_id.read().await;
        let subscription_id_to_client_id = self.subscription_id_to_client_id.read().await;
        let clients_tx = self.clients_tx.read().await;

        if let Some(subscription_ids) = subscription_subject_to_id.get(&subject) {
            for subscription_id in subscription_ids {
                if let Some(client_ids) = subscription_id_to_client_id.get(subscription_id) {
                    for client_id in client_ids {
                        if let Some((tx, _)) = clients_tx.get(client_id) {
                            send_message(
                                *client_id,
                                tx.clone(),
                                subscription_id.clone(),
                                subject.clone(),
                                msg.clone(),
                            );
                        } else {
                            warn!("unable to find client tx for client id {}", client_id);
                        }
                    }
                } else {
                    warn!("unable to find client ids for subscription id {}", subscription_id);
                }
            }
        } else {
            warn!("unable to find subscription id for subject: {}", subject);
        }
    }

    pub async fn process_shutdown(&self) {
        info!("process shutdown");
        if let Ok(clients_tx) = self.clients_tx.try_read() {
            for (client_id, (tx, _)) in clients_tx.iter() {
                if let Err(e) = tx.try_send(MainCommand::ShutDown) {
                    warn!("error sending shutdown message to client {}: {}", client_id, e);
                }
            }
        }
    }
}

struct MapWriteLocks<'a> {
    subscription_subject_to_id: RwLockWriteGuard<'a, HashMap<String, HashSet<String>>>,
    subscription_id_to_subject: RwLockWriteGuard<'a, HashMap<String, HashSet<String>>>,
    subscription_id_to_client_id: RwLockWriteGuard<'a, HashMap<String, HashSet<u32>>>,
    client_id_to_subscription_id: RwLockWriteGuard<'a, HashMap<u32, HashSet<String>>>,
}

fn insert_to_subscription_map<K, V>(mut map: RwLockWriteGuard<HashMap<K, HashSet<V>>>, key: K, value: V)
where
    K: Eq + std::hash::Hash,
    V: Eq + std::hash::Hash,
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

fn send_message(client_id: u32, client_tx: Sender<MainCommand>, subscription_id: String, subject: String, msg: String) {
    info!("publishing message to client id {} for subject {}", client_id, subject);
    tokio::spawn(async move {
        if let Err(e) = client_tx.send(MainCommand::PublishedMessage { subject, msg, subscription_id }).await {
            error!("error sending message to client {}: {}", client_id, e);
        }
    });
}
