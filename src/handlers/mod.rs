use std::time::Duration;

use actix_web::{
    post,
    web::{Data, Json},
    Responder, Result,
};
use log::warn;
use rdkafka::producer::{FutureProducer, FutureRecord};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc::Sender, oneshot};
use uuid::Uuid;

use crate::PaymentResult;

pub struct AppState {
    pub database: DatabaseConnection,
    pub producer: FutureProducer,
    pub request_registerer: Sender<(Uuid, oneshot::Sender<PaymentResult>)>,
}

//#[tracing::instrument]
#[post("/payment")]
async fn create_payment(
    data: Data<AppState>,
    payment_transaction: Json<PaymentTransaction>,
) -> Result<impl Responder> {
    let (tx, rx) = oneshot::channel();
    let request_registerer = &data.request_registerer;
    let _ = request_registerer
        .send((payment_transaction.request_id, tx))
        .await;

    let producer = &data.producer;
    let payment_transaction_json = serde_json::to_string(&payment_transaction)?;
    let delivery_status = producer
        .send(
            FutureRecord::to("payment")
                .payload(&payment_transaction_json)
                .key(&payment_transaction.wallet_id.to_string()),
            Duration::from_secs(1),
        )
        .await;

    if delivery_status.is_err() {
        return Err(actix_web::error::ErrorInternalServerError(
            "Failed to deliver message",
        ));
    }
    let resp = rx.await.unwrap();
    warn!("Yeah! Gotta Response: {:?}", resp);

    Ok(Json(payment_transaction.into_inner()))
}

#[derive(Serialize, Deserialize, Debug)]
struct PaymentTransaction {
    id: i32,
    request_id: Uuid,
    wallet_id: i32,
    amount: f64,
    status: String,
}
