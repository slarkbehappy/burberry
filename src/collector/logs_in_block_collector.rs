use std::sync::Arc;

use alloy::{
    primitives::B256,
    providers::Provider,
    pubsub::PubSubFrontend,
    rpc::types::eth::{Block, Filter, Log},
};
use async_trait::async_trait;
use futures::StreamExt;

use crate::types::{Collector, CollectorStream};

pub struct LogsInBlockCollector<T> {
    provider: Arc<dyn Provider<T>>,
    filter: Filter,
}

impl<T> LogsInBlockCollector<T> {
    pub fn new(provider: Arc<dyn Provider<T>>, filter: Filter) -> Self {
        Self { provider, filter }
    }
}

impl LogsInBlockCollector<PubSubFrontend> {
    async fn block_to_logs(&self, block_hash: B256) -> Option<Vec<Log>> {
        let logs = self
            .provider
            .get_logs(&self.filter.clone().at_block_hash(block_hash))
            .await;

        match logs {
            Ok(logs) => Some(logs),
            Err(e) => {
                tracing::error!(?block_hash, "fail to get logs: {e:#}");
                None
            }
        }
    }
}

#[async_trait]
impl Collector<(Block, Vec<Log>)> for LogsInBlockCollector<PubSubFrontend> {
    async fn get_event_stream(&self) -> eyre::Result<CollectorStream<'_, (Block, Vec<Log>)>> {
        let mut stream = self.provider.subscribe_blocks().await?.into_stream();

        let stream = async_stream::stream! {
            while let Some(block) = stream.next().await {
                let block_hash = match block.header.hash {
                    Some(block_hash) => block_hash,
                    None => continue,
                };

                let logs = match self.block_to_logs(block_hash).await {
                    Some(logs) => logs,
                    None => continue,
                };

                yield (block, logs);
            }
        };

        Ok(Box::pin(stream))
    }
}
