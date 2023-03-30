use clap::Parser;
use core::time;
use helium_proto::services::multi_buy::{
    multi_buy_server::{self, MultiBuyServer},
    MultiBuyIncReqV1, MultiBuyIncResV1,
};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, path::PathBuf};
use tokio::task;
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

struct State {
    cache: Arc<Mutex<HashMap<String, u32>>>,
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
        let multi_buy_req = request.into_inner();
        let key = multi_buy_req.key;
        let mut cache = self.cache.lock().unwrap();

        let key2 = key.clone();
        let cache2 = self.cache.clone();

        metrics::increment_counter!("multi_buy_service_hit");

        let old_count: u32 = match cache.get(&key) {
            None => {
                // TODO: cleanup
                task::spawn(async move {
                    tokio::time::sleep(time::Duration::from_millis(3000)).await;
                    let mut cache3 = cache2.lock().unwrap();
                    cache3.remove(&key2);
                    info!("cleaned {}", key2);
                });
                0
            }
            Some(&c) => c
        };
        let new_count = old_count + 1;

        cache.insert(key.clone(), new_count);

        let size = cache.len() as f64;
        metrics::gauge!("multi_buy_service_cache_size", size);

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

    let grpc_thread = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(MultiBuyServer::new(grpc_state))
            .serve(settings.grpc_listen)
            .await
            .unwrap();
    });

    let _ = tokio::try_join!(grpc_thread);

    Ok(())
}
