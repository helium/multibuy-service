use helium_proto::services::multi_buy::{
    multi_buy_server::{self, MultiBuyServer},
    MultiBuyGetReqV1, MultiBuyGetResV1,
};
use tokio::task;
use core::time;
use std::{collections::HashMap, thread};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tonic::Request;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub type Result<T = (), E = anyhow::Error> = anyhow::Result<T, E>;

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
    async fn get(
        &self,
        request: Request<MultiBuyGetReqV1>,
    ) -> Result<tonic::Response<MultiBuyGetResV1>, tonic::Status> {
        let multibuy_req = request.into_inner();
        let key = multibuy_req.key;
        let mut cache = self.cache.lock().unwrap();
        
        let key2 = key.clone();
        let cache2 = self.cache.clone();

        let old_count: u32 = match cache.get(&key) {
            None => {
                task::spawn(async move {
                    thread::sleep(time::Duration::from_millis(5000));
                    let mut cache3 = cache2.lock().unwrap();
                    cache3.remove(&key2);
                    info!("cleaned {}", key2);
                });
                0
            },
            Some(c) => c.clone(),
        };
        let new_count = old_count+1;

        cache.insert(key.clone(), new_count);

        info!("Key={} Count={}", key, new_count);

        Ok(tonic::Response::new(MultiBuyGetResV1 { count: new_count }))
    }
}

#[tokio::main]
async fn main() -> Result {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(&"INFO"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let server_details = "0.0.0.0:8080";
    let server: SocketAddr = server_details
        .parse()
        .expect("Unable to parse socket address");

    info!("Server started @ {server_details}");

    let grpc_state = State::new()?;

    let grpc_thread = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(MultiBuyServer::new(grpc_state))
            .serve(server)
            .await
            .unwrap();
    });

    let _ = tokio::try_join!(grpc_thread);

    Ok(())
}
