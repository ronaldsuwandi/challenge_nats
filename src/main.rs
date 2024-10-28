mod parser;
mod config;
mod server;
pub mod commands;
mod handlers;

use crate::server::Server;
use env_logger::Env;
use log::{error, info};
use std::env;
use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default()
        .default_filter_or("debug"))
        .init();

    let conf_path = env::args().nth(1).unwrap_or_else(|| { "config.toml".to_string() });
    let conf = config::parse_config(&conf_path);

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        signal_handlers(shutdown_tx).await;
    });

    let listener = TcpListener::bind(conf.listener).await?;
    let server = Arc::new(Server::new());

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((socket, _)) => {
                        let server = server.clone();
                        tokio::spawn(async move {
                            server.handle(socket).await;
                        });
                    }
                    Err(e) => {
                        error!("error accepting connection {:?}", e);
                    }
                }
            }

            _ = shutdown_rx.recv() => {
                info!("Shutting down");
                break;
            }
        }
    }

    // let x = Name{name: "x".to_string()};
    // let y = Name{name: "y".to_string()};
    //
    // let x2 = Arc::new(std::sync::Mutex::new(x));
    // // let mut y2 = Arc::new(y);
    // // let z = Name{name: "z".to_string()};
    //
    // let x3 = x2.clone();
    //
    // let v = vec![x3];
    //
    //
    // println!("{:?}", v);
    //
    // let mut x2obj = x2.lock().unwrap();
    // x2obj.change("new".to_string());
    // drop(x2obj);
    // drop(x2);
    // drop(x3);

    // drop(x);
    // println!("n=>{:?}",v);

    Ok(())
}

#[derive(Debug)]
struct Name {
    name: String,
}

impl Name {
    fn change(&mut self, new_name: String) {
        self.name = new_name;
    }
}

impl Drop for Name {
    fn drop(&mut self) {
        println!("dropping {} ", self.name);
        drop(self)
    }
}

async fn signal_handlers(shutdown_tx: Sender<()>) {
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigquit = signal(SignalKind::quit()).unwrap();

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            let _ = shutdown_tx.send(()).await;
        }
        _ = sigterm.recv() => {
            let _ = shutdown_tx.send(()).await;
        }
        _ = sigquit.recv() => {
             let _ = shutdown_tx.send(()).await;
       }
    }
}


