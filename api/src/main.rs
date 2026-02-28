use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod api;
pub mod db;
pub mod error;
pub mod video;

#[tokio::main]
async fn main() -> Result<(), crate::error::ApiError> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug")))
        .with(tracing_subscriber::fmt::layer())
        .init();
    let db = db::Db::open_default()?;
    api::ApiServer::serve(db).await
}
