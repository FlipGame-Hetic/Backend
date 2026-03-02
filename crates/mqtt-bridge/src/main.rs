use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt};

mod bridge;
mod client;
mod config;
mod errors;
mod handler;

use bridge::Bridge;
use config::BridgeConfig;

#[tokio::main]
async fn main() {
    init_tracing();

    let config = match BridgeConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to load config");
            std::process::exit(1);
        }
    };

    info!(
        mqtt_host = %config.mqtt_host,
        mqtt_port = config.mqtt_port,
        backend_ws_url = %config.backend_ws_url,
        "Starting mqtt-bridge"
    );

    let bridge = Bridge::new(config);
    bridge.run().await;
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .json()
        .init();
}
