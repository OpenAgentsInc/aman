//! Agent brain utilities for Aman.

#[cfg(feature = "nostr")]
use std::sync::Arc;

#[cfg(feature = "nostr")]
use nostr_persistence::{NostrIndexer, NostrPublisher};

#[derive(Default)]
pub struct AgentBrain {
    #[cfg(feature = "nostr")]
    pub nostr_publisher: Option<Arc<dyn NostrPublisher>>,
    #[cfg(feature = "nostr")]
    pub nostr_indexer: Option<Arc<dyn NostrIndexer>>,
}

impl AgentBrain {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "nostr")]
    pub fn with_nostr(
        publisher: Option<Arc<dyn NostrPublisher>>,
        indexer: Option<Arc<dyn NostrIndexer>>,
    ) -> Self {
        Self {
            nostr_publisher: publisher,
            nostr_indexer: indexer,
        }
    }
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
