use std::net::SocketAddr;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://tcgbars.db".into());
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "web/dist".into());
    let addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;

    let pool = tcgbars_server::db::connect(&database_url).await?;
    tcgbars_server::db::seed_if_empty(&pool).await?;

    let app = tcgbars_server::app(pool, Some(static_dir.into()));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
