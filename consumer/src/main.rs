use std::time::Duration;

use log::{info, warn};

use rdkafka::client::ClientContext;
use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::{CommitMode, Consumer, ConsumerContext, Rebalance};
use rdkafka::error::KafkaResult;
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::topic_partition_list::TopicPartitionList;
use uuid::Uuid;

struct CustomContext;

impl ClientContext for CustomContext {}

impl ConsumerContext for CustomContext {
    fn pre_rebalance(&self, rebalance: &Rebalance) {
        info!("Pre rebalance {:?}", rebalance);
    }

    fn post_rebalance(&self, rebalance: &Rebalance) {
        info!("Post rebalance {:?}", rebalance);
    }

    fn commit_callback(&self, result: KafkaResult<()>, _offsets: &TopicPartitionList) {
        info!("Committing offsets: {:?}", result);
    }
}

type LoggingConsumer = StreamConsumer<CustomContext>;

#[tokio::main]
pub async fn main() {
    let context = CustomContext;
    let response_producer = make_producer();

    let consumer: LoggingConsumer = ClientConfig::new()
        .set("group.id", "kwallet")
        .set("bootstrap.servers", "localhost:9092")
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set_log_level(RDKafkaLogLevel::Debug)
        .create_with_context(context)
        .expect("Consumer creation failed");

    consumer
        .subscribe(&["payment"])
        .expect("Can't subscribe to specified topics");

    loop {
        match consumer.recv().await {
            Err(e) => {
                panic!("Error while receiving message: {:?}", e);
            }
            Ok(m) => {
                let payload = match m.payload_view::<str>() {
                    None => "",
                    Some(Ok(s)) => s,
                    Some(Err(e)) => {
                        warn!("Error while deserializing message payload: {:?}", e);
                        ""
                    }
                };

                let payload_json =
                    serde_json::from_str::<serde_json::Value>(payload).unwrap_or_default();

                let request_id = payload_json
                    .get("request_id")
                    .and_then(|v| v.as_str())
                    .and_then(|v| Uuid::parse_str(v).ok())
                    .unwrap();

                let wallet_id = payload_json
                    .get("wallet_id")
                    .and_then(|v| v.as_i64())
                    .unwrap();

                println!("request_id: {:?}", request_id);

                response_producer
                    .send(
                        FutureRecord::to("payment_result")
                            .payload(&payload_json.to_string())
                            .key(wallet_id.to_string().as_str()),
                        Duration::from_secs(1),
                    )
                    .await
                    .unwrap();
                consumer.commit_message(&m, CommitMode::Async).unwrap();
            }
        };
    }
}

pub fn make_producer() -> FutureProducer {
    let mut config = ClientConfig::new();
    config
        .set("bootstrap.servers", "localhost:9092")
        .set("message.timeout.ms", "10000")
        .create::<FutureProducer>()
        .expect("Producer creation error")
}
