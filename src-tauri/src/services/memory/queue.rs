use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use crate::services::ollama::OllamaService;
use crate::services::groq::GroqService;
use super::db::MemoryDb;
use super::qdrant::MemoryQdrant;
use super::extraction::MemoryExtractor;

pub enum MemoryJob {
    ExtractMemory {
        conversation_id: String,
        user_message: String,
        assistant_response: String,
    },
}

pub struct MemoryQueue {
    sender: UnboundedSender<MemoryJob>,
    pub(crate) receiver: Mutex<Option<UnboundedReceiver<MemoryJob>>>,
}

impl MemoryQueue {
    pub fn new() -> Self {
        let (sender, receiver) = unbounded_channel();
        Self {
            sender,
            receiver: Mutex::new(Some(receiver)),
        }
    }

    pub fn enqueue(&self, job: MemoryJob) -> anyhow::Result<()> {
        self.sender.send(job).map_err(|_| anyhow::anyhow!("Failed to enqueue memory job: receiver dropped"))
    }

    pub async fn worker_loop(
        mut receiver: UnboundedReceiver<MemoryJob>,
        db: Arc<MemoryDb>,
        ollama_service: OllamaService,
        groq_service: GroqService,
        qdrant: Arc<MemoryQdrant>,
    ) {
        let extractor = Arc::new(MemoryExtractor::new(
            db.clone(),
            ollama_service.clone(),
            groq_service.clone(),
            qdrant.clone(),
        ));

        tracing::info!("Memory queue worker loop started");

        while let Some(job) = receiver.recv().await {
            match job {
                MemoryJob::ExtractMemory {
                    conversation_id,
                    user_message,
                    assistant_response,
                } => {
                    tracing::info!("Starting background memory extraction for convo {}", conversation_id);
                    if let Err(e) = extractor.extract_and_consolidate(&conversation_id, &user_message, &assistant_response).await {
                        tracing::error!("Memory extraction failed for convo {}: {:?}", conversation_id, e);
                    }
                }
            }
        }
    }
}
