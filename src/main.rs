mod parser;
mod config;

use std::env;
use std::error::Error;
use std::sync::Arc;
use env_logger::Env;
use log::info;
use tokio::net::TcpListener;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default()
        .default_filter_or("info"))
        .init();

    let conf_path = env::args().nth(1).unwrap_or_else(|| { "config.toml".to_string() });
    let conf = config::parse_config(&conf_path);

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        signal_handlers(shutdown_tx).await;
    });

    let listener = TcpListener::bind(conf.listener).await?;
    // let lb = Arc::new(LoadBalancer::new(conf.servers));

    // trigger initial healthcheck
    // lb.health_check().await;

    // let lb_clone = lb.clone();

    // tokio::spawn(async move {
    //     loop {
    //         tokio::select! {
    //             _ = sleep(Duration::from_secs(5)) => {
    //                 debug!("Executing health check...");
    //                 lb_clone.health_check().await;
    //             }
    //         }
    //     }
    // });

    loop {
        tokio::select! {
            // result = listener.accept() => {
            //     match result {
            //         Ok((socket, _)) => {
            //             let lb = lb.clone();
            //             tokio::spawn(async move {
            //                 if let Err(e) = lb.handle(socket).await {
            //                     error!("Error handling request: {}", e);
            //                 }
            //             });
            //         }
            //         Err(e) => {
            //             error!("Error accepting connection {:?}", e);
            //         }
            //     }
            // }

            _ = shutdown_rx.recv() => {
                info!("Shutting down");
                break;
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
