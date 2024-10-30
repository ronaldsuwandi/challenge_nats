mod parser;
mod config;
mod server;
pub mod commands;
mod handlers;

use crate::server::Server;
use env_logger::Env;
use log::{error, info, warn};
use std::env;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::time::timeout;
use crate::commands::MainCommand;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default()
        .default_filter_or("warn"))
        .init();

    let conf_path = env::args().nth(1).unwrap_or_else(|| { "config.toml".to_string() });
    let conf = config::parse_config(&conf_path);

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        signal_handlers(shutdown_tx).await;
    });

    let listener = TcpListener::bind(conf.listener).await?;
    let (server, main_rx) = Server::new();
    let server = Arc::new(server);

    let server_arc = server.clone();
    let mut handles = vec![];
    let handle = tokio::spawn(async move {
        server_arc.process_rx(main_rx).await;
    });
    handles.push(handle);

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((socket, _)) => {
                        let server = server.clone();
                        let handle = tokio::spawn(async move {
                            server.handle(socket).await;
                        });
                        handles.push(handle);
                    }
                    Err(e) => {
                        error!("error accepting connection {:?}", e);
                    }
                }
            }

            _ = shutdown_rx.recv() => {
                info!("Shutting down");
                let _ = server.main_tx.clone().send(MainCommand::ShutDown).await;
                break;
            }
        }
    }

    for handle in handles {
        match timeout(Duration::from_secs(5), handle).await {
            Ok(result) => {
                if let Err(e) = result {
                    error!("error in shutting down client: {:?}", e);
                }
            }
            Err(_) => {
                warn!("client task timed out during shutdown");
            }
        }
    }

    Ok(())
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
