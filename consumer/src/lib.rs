use log::{info, warn};

use anyhow::Result;
use rdkafka::client::ClientContext;
use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::{CommitMode, Consumer, ConsumerContext, Rebalance};
use rdkafka::error::KafkaResult;
use rdkafka::message::{Headers, Message};
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

pub async fn consume(
    brokers: &str,
    group_id: &str,
    topics: &[&str],
    consumed_message_sender: tokio::sync::mpsc::Sender<Result<(Uuid, usize)>>,
) {
    let context = CustomContext;

    let consumer: LoggingConsumer = ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set_log_level(RDKafkaLogLevel::Debug)
        .create_with_context(context)
        .expect("Consumer creation failed");

    consumer
        .subscribe(&topics.to_vec())
        .expect("Can't subscribe to specified topics");

    info!("Subscribed to topics: {:?}", topics);

    loop {
        match consumer.recv().await {
            Err(e) => {
                consumed_message_sender
                    .send(Err(anyhow::anyhow!(
                        "Error while receiving message: {:?}",
                        e
                    )))
                    .await
                    .unwrap();
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
                info!("key: '{:?}', payload: '{}', topic: {}, partition: {}, offset: {}, timestamp: {:?}",
                      m.key(), payload, m.topic(), m.partition(), m.offset(), m.timestamp());
                //if let Some(headers) = m.headers() {
                //    for header in headers.iter() {
                //        info!("Header {:#?}: {:?}", header.key, header.value);
                //    }
                //}

                let payload_json =
                    serde_json::from_str::<serde_json::Value>(payload).unwrap_or_default();

                let request_id = payload_json
                    .get("request_id")
                    .and_then(|v| v.as_str())
                    .and_then(|v| Uuid::parse_str(v).ok())
                    .unwrap_or_else(Uuid::new_v4);

                warn!("request_id: {:?}", request_id);

                consumed_message_sender
                    .send(Ok((request_id, 1)))
                    .await
                    .unwrap();
                consumer.commit_message(&m, CommitMode::Async).unwrap();
            }
        };
    }
}
