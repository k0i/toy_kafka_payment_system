use rdkafka::{producer::FutureProducer, ClientConfig};

pub fn make_producer() -> FutureProducer {
    let mut config = ClientConfig::new();
    config
        .set("bootstrap.servers", "localhost:9092")
        .set("message.timeout.ms", "10000")
        .create::<FutureProducer>()
        .expect("Producer creation error")
}
