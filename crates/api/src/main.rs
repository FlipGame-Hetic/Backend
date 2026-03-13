use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt};

mod app;
mod config;
mod docs;
mod errors;
mod modules;
mod router;
mod state;

use config::ApiConfig;

#[tokio::main]
async fn main() {
    init_tracing();

    let config = match ApiConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to load config");
            std::process::exit(1);
        }
    };

    let addr = config.socket_addr();

    info!(port = config.port, "Starting api server");

    let app = app::build(&config);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!(error = %e, addr = %addr, "Failed to bind");
            std::process::exit(1);
        }
    };

    info!(addr = %addr, "Listening");

    if let Err(e) = axum::serve(listener, app).await {
        error!(error = %e, "Server exited with error");
        std::process::exit(1);
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .json()
        .init();
}
