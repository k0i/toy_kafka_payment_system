mod handlers;

use std::{collections::HashMap, time::Duration};

use actix_web::{middleware::Logger, web::Data, HttpServer};
use anyhow::Result;
use consumer::consume;
use handlers::AppState;
use log::warn;
use producer::make_producer;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot,
};
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};
use uuid::Uuid;

const DATABASE_URL: &str = "mysql://root@localhost:3306/kwallet";

pub type PaymentResult = Result<(Uuid, usize)>;

#[actix_web::main]
async fn main() -> Result<()> {
    LogTracer::init().expect("Failed to set logger");
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let formatting_layer = BunyanFormattingLayer::new("kwallet".into(), std::io::stdout);
    let subscriber = tracing_subscriber::registry()
        .with(JsonStorageLayer)
        .with(env_filter)
        .with(formatting_layer)
        .with(tracing_subscriber::fmt::layer().with_ansi(true));
    set_global_default(subscriber).expect("Failed to set subscriber");

    let db = init_db().await?;

    let (tx, payment_response_receiver) = tokio::sync::mpsc::channel(100);
    let (request_registerer, rx) = tokio::sync::mpsc::channel(100);
    let consumer = tokio::spawn(spawn_consumer(tx));
    let mut request_response_mapper = RequestResponseMapper::new(rx, payment_response_receiver);

    let task = HttpServer::new(move || {
        actix_web::App::new()
            .app_data(Data::new(AppState {
                database: db.clone(),
                producer: make_producer(),
                request_registerer: request_registerer.clone(),
            }))
            .wrap(Logger::default())
            .service(handlers::create_payment)
    })
    .bind("localhost:3000")
    .expect("Can not bind to port 3000")
    .run();

    tokio::join!(task, consumer, request_response_mapper.start()).0?;

    Ok(())
}

async fn init_db() -> Result<DatabaseConnection> {
    let mut opt = ConnectOptions::new(DATABASE_URL);
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(false)
        .sqlx_logging_level(log::LevelFilter::Info);

    let db = Database::connect(opt).await?;
    Ok(db)
}

async fn spawn_consumer(tx: Sender<PaymentResult>) {
    consume("localhost:9092", "kwallet", &["payment_result"], tx).await;
}

pub struct RequestResponseMapper {
    request_registerer: Receiver<(Uuid, oneshot::Sender<PaymentResult>)>,
    response_receiver: Receiver<PaymentResult>,
    table: HashMap<Uuid, oneshot::Sender<PaymentResult>>,
}

impl RequestResponseMapper {
    pub fn new(
        request_registerer: Receiver<(Uuid, oneshot::Sender<PaymentResult>)>,
        response_receiver: Receiver<PaymentResult>,
    ) -> Self {
        Self {
            request_registerer,
            response_receiver,
            table: HashMap::new(),
        }
    }

    pub async fn start(&mut self) {
        loop {
            tokio::select! {
                Some(result) = self.response_receiver.recv() => {
                    let request_id = result.as_ref().unwrap().0;
                    warn!("Received response for request_id: {:?}", request_id);
                    if let Some(tx) = self.table.remove(&request_id) {
                        tx.send(result).unwrap();
                    }
                }
                Some((request_id, tx)) = self.request_registerer.recv() => {
                    self.register_to_notify(request_id, tx);
                }
            }
        }
    }

    fn register_to_notify(&mut self, request_id: Uuid, tx: oneshot::Sender<PaymentResult>) {
        self.table.insert(request_id, tx);
    }
}
