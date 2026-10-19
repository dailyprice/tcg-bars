use std::{net::SocketAddr, time::Duration};

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://tcgbars.db".into());
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "web/dist".into());
    // Hosts like Railway pass the port to listen on in PORT.
    let addr: SocketAddr = match (std::env::var("BIND_ADDR"), std::env::var("PORT")) {
        (Ok(addr), _) => addr.parse()?,
        (Err(_), Ok(port)) => format!("0.0.0.0:{port}").parse()?,
        _ => "127.0.0.1:8080".parse()?,
    };

    let pool = tcgbars_server::db::connect(&database_url).await?;
    tcgbars_server::db::seed_if_empty(&pool).await?;

    let app = tcgbars_server::app(pool, Some(static_dir.into()));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            shutdown_signal().await;
            // Live-update streams never end on their own, so don't wait on them for long.
            tokio::spawn(async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                std::process::exit(0);
            });
        })
        .await?;
    Ok(())
}

/// Resolves on Ctrl+C or SIGTERM, which is how hosts stop the container.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
