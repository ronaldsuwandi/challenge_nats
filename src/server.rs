use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock};

pub struct Server {
    pub clients_map: Arc<RwLock<HashMap<u32, Arc<Mutex<TcpStream>>>>>,
    pub subscription_map: Arc<RwLock<HashMap<String, RwLock<HashSet<u32>>>>>,
    pub client_id_to_subscriptions: Arc<RwLock<HashMap<u32, RwLock<HashSet<String>>>>>,
    pub client_id: AtomicU32,
    pub lock: Mutex<()>,
}

impl Server {
    pub fn new() -> Server {
        Server {
            // submap: Mutex::new(HashMap::new()),
            clients_map: Arc::new(RwLock::new(HashMap::new())),
            subscription_map: Arc::new(RwLock::new(HashMap::new())),
            client_id_to_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            client_id: AtomicU32::new(0),
            lock: Mutex::new(()),
        }
    }
}
