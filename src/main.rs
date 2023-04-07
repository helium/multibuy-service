use clap::Parser;
use core::time;
use helium_proto::services::multi_buy::{
    multi_buy_server::{self, MultiBuyServer},
    MultiBuyIncReqV1, MultiBuyIncResV1,
};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::{collections::HashMap, path::PathBuf};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use tonic::Request;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::settings::Settings;

mod settings;

pub type Result<T = (), E = anyhow::Error> = anyhow::Result<T, E>;

#[derive(Debug, Parser)]
struct Cli {
    #[arg(short, long)]
    config_file: Option<PathBuf>,
}

#[derive(Debug, Copy, Clone)]
struct CacheValue {
    count: u32,
    timestamp: u128,
}

struct State {
    cache: Arc<Mutex<HashMap<String, CacheValue>>>,
}

impl State {
    fn new() -> Result<Self> {
        Ok(Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

#[tonic::async_trait]
impl multi_buy_server::MultiBuy for State {
    async fn inc(
        &self,
        request: Request<MultiBuyIncReqV1>,
    ) -> Result<tonic::Response<MultiBuyIncResV1>, tonic::Status> {
        metrics::increment_counter!("multi_buy_service_hit");

        let multi_buy_req = request.into_inner();
        let key = multi_buy_req.key;
        let mut cache = self.cache.lock().await;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        let cached_value: CacheValue = match cache.get(&key) {
            None => {
                let size = cache.len() as f64;
                metrics::gauge!("multi_buy_service_cache_size", size + 1.0);
                CacheValue {
                    count: 0,
                    timestamp: now,
                }
            }
            Some(&cached_value) => cached_value,
        };

        let new_count = cached_value.count + 1;

        cache.insert(
            key.clone(),
            CacheValue {
                count: new_count,
                timestamp: cached_value.timestamp,
            },
        );

        info!("Key={} Count={}", key, new_count);

        Ok(tonic::Response::new(MultiBuyIncResV1 { count: new_count }))
    }
}

#[tokio::main]
async fn main() -> Result {
    let cli = Cli::parse();
    let settings = Settings::new(cli.config_file)?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(&settings.log))
        .with(tracing_subscriber::fmt::layer())
        .init();

    if let Err(e) = PrometheusBuilder::new()
        .with_http_listener(settings.metrics_listen)
        .install()
    {
        error!("Failed to install Prometheus scrape endpoint: {e}");
    } else {
        info!(endpoint = %settings.metrics_listen, "Metrics listening");
    }

    info!("Server started @ {:?}", settings.grpc_listen);

    let grpc_state = State::new()?;
    let grpc_state_cache = grpc_state.cache.clone();

    let grpc_thread = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(MultiBuyServer::new(grpc_state))
            .serve(settings.grpc_listen)
            .await
            .unwrap();
    });

    tokio::spawn(async move {
        loop {
            // Sleep 30min
            let duration = time::Duration::from_secs(60 * 30);

            tokio::time::sleep(duration).await;

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis();

            let mut cache = grpc_state_cache.lock().await;
            let size_before = cache.len() as f64;

            cache.retain(|_, v| v.timestamp > now - duration.as_millis());

            let size_after = cache.len() as f64;
            info!("cleaned {}", size_before - size_after);
        }
    });

    let _ = tokio::try_join!(grpc_thread);

    Ok(())
}
