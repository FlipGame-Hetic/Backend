use api::app;
use api::config::ApiConfig;
use sqlx::SqlitePool;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt};

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

    let pool = match SqlitePool::connect(&config.database_url).await {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, url = %config.database_url, "Failed to connect to database");
            std::process::exit(1);
        }
    };

    if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
        error!(error = %e, "Failed to run database migrations");
        std::process::exit(1);
    }

    info!(url = %config.database_url, "Database ready");

    let addr = config.socket_addr();
    info!(port = config.port, "Starting api server");

    let server = app::build(&config, pool);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!(error = %e, addr = %addr, "Failed to bind");
            std::process::exit(1);
        }
    };

    info!(addr = %addr, "Listening");

    if let Err(e) = axum::serve(listener, server).await {
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
