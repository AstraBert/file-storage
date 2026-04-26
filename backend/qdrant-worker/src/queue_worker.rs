mod chunking;
mod embeddings;
mod ingestion;
mod vectordb;

use futures::StreamExt;
use observability::init_tracing_subscriber;
use rabbitmq_stream_client::error::StreamCreateError;
use rabbitmq_stream_client::types::{ByteCapacity, OffsetSpecification, ResponseCode};
use utils::{MessageAction, MessageData, STATUS_COMPLETED, STATUS_STARTED};

use crate::ingestion::Pipeline;

const STREAM_NAME: &str = "worker_queue";
const CHUNK_SIZE: usize = 1024;
const QDRANT_URL: &str = "http://qdrant:6334";
const COLLECTION_NAME: &str = "file_storage_search";

#[tokio::main]
#[tracing::instrument]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use rabbitmq_stream_client::Environment;
    let _guard = init_tracing_subscriber();
    let environment = Environment::builder()
        .host("rabbitmq")
        .port(5552)
        .username(
            &std::env::var("RABBITMQ_DEFAULT_USER")
                .expect("Should have RABBITMQ_DEFAULT_USER set in env"),
        )
        .password(
            &std::env::var("RABBITMQ_DEFAULT_PASS")
                .expect("Should have RABBITMQ_DEFAULT_PASS set in env"),
        )
        .build()
        .await?;
    log::info!("Connected to RabbitMQ");
    let pipeline = Pipeline::new(
        CHUNK_SIZE,
        QDRANT_URL.to_string(),
        COLLECTION_NAME.to_string(),
    )?;

    // create the rabbitmq stream if it does not already exist
    let create_response = environment
        .stream_creator()
        .max_length(ByteCapacity::GB(5))
        .create(STREAM_NAME)
        .await;

    if let Err(StreamCreateError::Create { stream: _, status }) = create_response {
        match status {
            // we can ignore this error because the stream already exists
            ResponseCode::StreamAlreadyExists => {
                log::info!("Stream already exists")
            }
            err => {
                log::error!("Error creating stream: {:?} {:?}", STREAM_NAME, err);
            }
        }
    }

    let mut consumer = environment
        .consumer()
        .offset(OffsetSpecification::First)
        .build(STREAM_NAME)
        .await?;

    while let Some(Ok(delivery)) = consumer.next().await {
        tracing::info!(event = "queue_message", status = STATUS_STARTED);
        let message = delivery
            .message()
            .data()
            .map(|data| String::from_utf8(data.to_vec()).unwrap())
            .unwrap();
        let data: MessageData = serde_json::from_str(&message)?;
        log::debug!(
            "Got message: {:#?} from stream: {} with offset: {}",
            &message,
            delivery.stream(),
            delivery.offset()
        );
        match data.action {
            MessageAction::Create => {
                pipeline.run(&data.content, &data.user_identity).await?;
            }
            MessageAction::Delete => {
                pipeline.delete(&data.content, &data.user_identity).await?;
            }
        }
        tracing::info!(event = "queue_message", status = STATUS_COMPLETED);
        log::debug!("Successfully processed message for Qdrant");
    }

    let _ = consumer.handle().close().await;

    log::info!("Stream consumer stopped");
    Ok(())
}
