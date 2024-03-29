use core::time;
use std::time::{SystemTime, UNIX_EPOCH};

use helium_proto::services::multi_buy::{
    multi_buy_client::MultiBuyClient, MultiBuyIncReqV1, MultiBuyIncResV1,
};
use rand::distributions::{Alphanumeric, DistString};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub type Result<T = (), E = anyhow::Error> = anyhow::Result<T, E>;

include!("../src/settings.rs");

#[tokio::main]
async fn main() -> Result {
    let settings = Settings::new(Some("settings.toml".to_string()))?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(&settings.log))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let port = settings.grpc_listen.port();
    let url = format!("http://127.0.0.1:{port}");

    info!("connecting to {url}");

    let mut client = MultiBuyClient::connect(url).await?;
    let key = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);

    loop {
        let req = MultiBuyIncReqV1 {
            key: key.to_string(),
        };
        let b = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        let res: MultiBuyIncResV1 = client.inc(req).await?.into_inner();

        let a = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        info!("Key={} Count={} in {}", key, res.count, a - b);

        tokio::time::sleep(time::Duration::from_millis(1000)).await;
    }
}
