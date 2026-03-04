use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod video;

#[tokio::main]
async fn main() -> Result<(), crate::error::ApiError> {
    println!("[table-tv] main: starting");
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("debug,tower_http=debug,reqwest=warn,hyper=warn,hyper_util=warn")
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let _config = config::init();
    tracing::info!("[table-tv] main: opening db");
    let db = db::Db::open_default()?;
    tracing::info!("[table-tv] main: db ok, starting server");
    api::ApiServer::serve(db).await?;
    tracing::info!("[table-tv] exiting");
    Ok(())
}
