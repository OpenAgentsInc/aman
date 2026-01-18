use std::sync::Arc;

use tracing::warn;

#[cfg(feature = "nostr")]
use nostr_persistence::{MemoryPublisherConfig, NostrMemoryPublisher, NostrMemoryPublisherImpl};

#[cfg(feature = "nostr")]
pub type MemoryPublisher = Arc<dyn NostrMemoryPublisher>;

#[cfg(not(feature = "nostr"))]
pub type MemoryPublisher = ();

pub async fn memory_publisher_from_env() -> Option<MemoryPublisher> {
    #[cfg(feature = "nostr")]
    {
        match MemoryPublisherConfig::from_env() {
            Ok(Some(config)) => match NostrMemoryPublisherImpl::new(config).await {
                Ok(publisher) => Some(Arc::new(publisher)),
                Err(err) => {
                    warn!("Failed to initialize Nostr memory publisher: {}", err);
                    None
                }
            },
            Ok(None) => None,
            Err(err) => {
                warn!("Failed to load Nostr memory config: {}", err);
                None
            }
        }
    }

    #[cfg(not(feature = "nostr"))]
    {
        None
    }
}
