use core::time;
use std::{thread, time::{SystemTime, UNIX_EPOCH}};

use helium_proto::services::multi_buy::{
    multi_buy_client::MultiBuyClient, MultiBuyGetReqV1, MultiBuyGetResV1,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub type Result<T = (), E = anyhow::Error> = anyhow::Result<T, E>;

#[tokio::main]
async fn main() -> Result {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(&"INFO"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let port = 8080;
    let url = format!("http://127.0.0.1:{port}");

    info!("connecting to {url}");

    let mut client = MultiBuyClient::connect(url).await?;
    let sleep_timer = time::Duration::from_millis(1000);

    loop {
        let key = "test";
        let req = MultiBuyGetReqV1 {
            key: key.to_string(),
        };
        let b = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        let res: MultiBuyGetResV1 = client.get(req).await?.into_inner();

        let a = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        info!("Key={} Count={} in {}", key, res.count, a-b);

        thread::sleep(sleep_timer);
    }
}
