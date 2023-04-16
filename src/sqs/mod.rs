use crate::errors::{self, Error, Result};
use aws_sdk_sqs::{
    error::ProvideErrorMetadata,
    {
        operation::{create_queue::CreateQueueError, delete_queue::DeleteQueueError},
        types::QueueAttributeName,
        Client,
    },
};
use aws_smithy_client::SdkError;
use aws_types::SdkConfig as AwsSdkConfig;

/// Implements AWS SQS manager.
#[derive(Debug, Clone)]
pub struct Manager {
    pub region: String,
    pub cli: Client,
}

impl Manager {
    pub fn new(shared_config: &AwsSdkConfig) -> Self {
        Self {
            region: shared_config.region().unwrap().to_string(),
            cli: Client::new(shared_config),
        }
    }

    /// Creates a FIFO SQS queue.
    /// ref. <https://docs.aws.amazon.com/AWSSimpleQueueService/latest/APIReference/API_CreateQueue.html>
    pub async fn create_fifo(&self, name: &str) -> Result<String> {
        log::info!("creating a FIFO queue '{name}'");
        if name.len() > 80 {
            return Err(Error::Other {
                message: format!("queue name '{name}' exceeds >80"),
                retryable: false,
            });
        }
        // FIFO queue name must end with the .fifo suffix.
        if !name.ends_with(".fifo") {
            return Err(Error::Other {
                message: format!("queue name '{name}' does not end with .fifo"),
                retryable: false,
            });
        }

        let resp = self
            .cli
            .create_queue()
            .queue_name(name)
            .attributes(QueueAttributeName::MaximumMessageSize, "262144") // 256-KiB
            .attributes(QueueAttributeName::MessageRetentionPeriod, "345600") // 4-day in seconds
            //
            // 30-second; prevent other consumers from processing the message again
            // When a consumer receives and processes a message from a queue, the message remains in the queue.
            // Amazon SQS doesn't automatically delete the message.
            // Because Amazon SQS is a distributed system, there's no guarantee that the consumer actually receives the message.
            // Thus, the consumer must delete the message from the queue after receiving and processing it.
            // ref. <https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/sqs-visibility-timeout.html>
            .attributes(QueueAttributeName::VisibilityTimeout, "30")
            //
            // ref. <https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/high-throughput-fifo.html>
            .attributes(QueueAttributeName::FifoQueue, "true")
            .attributes(QueueAttributeName::SqsManagedSseEnabled, "true")
            .attributes(QueueAttributeName::DeduplicationScope, "messageGroup")
            .attributes(QueueAttributeName::ContentBasedDeduplication, "true")
            .attributes(QueueAttributeName::FifoThroughputLimit, "perMessageGroupId")
            //
            // ref. <https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/sqs-server-side-encryption.html>
            .attributes(QueueAttributeName::SqsManagedSseEnabled, "true")
            .tags("Name", name)
            .send()
            .await
            .map_err(|e| Error::API {
                message: format!("failed create_queue '{}'", explain_create_queue_error(&e)),
                retryable: errors::is_sdk_err_retryable(&e),
            })?;

        if let Some(queue_url) = resp.queue_url() {
            log::info!("created a FIFO queue '{queue_url}");
            Ok(queue_url.to_string())
        } else {
            Err(Error::API {
                message: "no queue URL found".to_string(),
                retryable: false,
            })
        }
    }

    /// Delete a queue.
    /// ref. <https://docs.aws.amazon.com/AWSSimpleQueueService/latest/APIReference/API_DeleteQueue.html>
    pub async fn delete(&self, queue_url: &str) -> Result<()> {
        log::info!("deleting a queue '{queue_url}'");
        self.cli
            .delete_queue()
            .queue_url(queue_url)
            .send()
            .await
            .map_err(|e| {
                log::warn!("failed to delete queue {:?}", e);
                Error::API {
                    message: format!("failed delete_queue {:?}", e),
                    retryable: errors::is_sdk_err_retryable(&e)
                        || is_err_retryable_delete_queue(&e),
                }
            })?;

        log::info!("successfully deleted '{queue_url}'");
        Ok(())
    }

    /// Sends a message to an FIFO queue.
    /// ref. <https://docs.aws.amazon.com/AWSSimpleQueueService/latest/APIReference/API_SendMessage.html>
    pub async fn send_msg_fifo(&self, queue_url: &str, msg_group_id: &str) -> Result<()> {
        log::info!("sending msg to '{queue_url}' with group id '{msg_group_id}'");

        // TODO

        Ok(())
    }
}

#[inline]
pub fn explain_create_queue_error(e: &SdkError<CreateQueueError>) -> String {
    match e {
        SdkError::ServiceError(err) => format!(
            "create_queue [code '{:?}', message '{:?}']",
            err.err().meta().code(),
            err.err().meta().message(),
        ),
        _ => e.to_string(),
    }
}

#[inline]
pub fn is_err_retryable_delete_queue(e: &SdkError<DeleteQueueError>) -> bool {
    match e {
        SdkError::ServiceError(err) => {
            // TODO: handle this...
            log::info!("message {}", err.err().message().unwrap());
            false
        }
        _ => false,
    }
}
