//! The main DonationWallet implementation.
//!
//! ⚠️ SECURITY: This module intentionally does NOT expose any pay/send functions.
//! Only receive-related functionality is available.

use std::sync::Arc;
use tracing::info;

use lni::{
    LightningNode,
    NodeInfo,
    Transaction,
    CreateInvoiceParams,
    LookupInvoiceParams,
    ListTransactionsParams,
    phoenixd::PhoenixdNode,
    cln::ClnNode,
    lnd::LndNode,
    nwc::NwcNode,
    strike::StrikeNode,
    blink::BlinkNode,
    speed::SpeedNode,
};

use crate::config::DonationWalletConfig;
use crate::error::DonationWalletError;

/// A receive-only Lightning wallet for accepting donations.
///
/// This wallet wraps various Lightning backends but intentionally
/// exposes ONLY functions for receiving payments, never for sending.
///
/// ## ⚠️ Security Notice
///
/// This struct does NOT implement or expose:
/// - `pay_invoice()`
/// - `pay_offer()`
/// - `send_payment()`
/// - `pay_keysend()`
///
/// These functions could drain the wallet and are forbidden in a donation context.
pub struct DonationWallet {
    node: Arc<dyn LightningNode>,
}

// ============================================================================
// ⚠️ AI AGENT WARNING ⚠️
// ============================================================================
// DO NOT add any of the following methods to this impl block:
// - pay_invoice
// - pay_offer  
// - send_payment
// - pay_keysend
// - Any method that sends/pays/transfers funds
//
// This wallet is for RECEIVING donations ONLY.
// ============================================================================

impl DonationWallet {
    /// Create a new donation wallet with the specified backend configuration.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = DonationWalletConfig::Phoenixd {
    ///     url: "http://localhost:9740".to_string(),
    ///     password: "password".to_string(),
    /// };
    /// let wallet = DonationWallet::new(config)?;
    /// ```
    pub fn new(config: DonationWalletConfig) -> Result<Self, DonationWalletError> {
        let node: Arc<dyn LightningNode> = match config.clone() {
            DonationWalletConfig::Phoenixd { .. } => {
                let cfg = config.into_phoenixd_config().unwrap();
                info!("Creating Phoenixd donation wallet");
                Arc::new(PhoenixdNode::new(cfg))
            }
            DonationWalletConfig::Cln { .. } => {
                let cfg = config.into_cln_config().unwrap();
                info!("Creating CLN donation wallet");
                Arc::new(ClnNode::new(cfg))
            }
            DonationWalletConfig::Lnd { .. } => {
                let cfg = config.into_lnd_config().unwrap();
                info!("Creating LND donation wallet");
                Arc::new(LndNode::new(cfg))
            }
            DonationWalletConfig::Nwc { .. } => {
                let cfg = config.into_nwc_config().unwrap();
                info!("Creating NWC donation wallet");
                Arc::new(NwcNode::new(cfg))
            }
            DonationWalletConfig::Strike { .. } => {
                let cfg = config.into_strike_config().unwrap();
                info!("Creating Strike donation wallet");
                Arc::new(StrikeNode::new(cfg))
            }
            DonationWalletConfig::Blink { .. } => {
                let cfg = config.into_blink_config().unwrap();
                info!("Creating Blink donation wallet");
                Arc::new(BlinkNode::new(cfg))
            }
            DonationWalletConfig::Speed { .. } => {
                let cfg = config.into_speed_config().unwrap();
                info!("Creating Speed donation wallet");
                Arc::new(SpeedNode::new(cfg))
            }
        };

        Ok(Self { node })
    }

    // ========================================================================
    // SAFE FUNCTIONS - These only READ data or RECEIVE payments
    // ========================================================================

    /// Get information about the connected Lightning node.
    ///
    /// Returns node alias, pubkey, balance information, etc.
    /// This is a read-only operation.
    pub async fn get_info(&self) -> Result<NodeInfo, DonationWalletError> {
        self.node.get_info().await.map_err(Into::into)
    }

    /// Create an invoice to receive a donation.
    ///
    /// # Arguments
    ///
    /// * `amount_msats` - Amount in millisatoshis (1 sat = 1000 msats)
    /// * `description` - Optional description for the invoice
    /// * `expiry_secs` - Optional expiry time in seconds (default varies by backend)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Create invoice for 1000 sats (1,000,000 msats)
    /// let invoice = wallet.create_invoice(
    ///     1_000_000,
    ///     Some("Donation to my project".to_string()),
    ///     Some(3600), // 1 hour expiry
    /// ).await?;
    /// println!("Pay this invoice: {}", invoice.invoice);
    /// ```
    pub async fn create_invoice(
        &self,
        amount_msats: i64,
        description: Option<String>,
        expiry_secs: Option<i64>,
    ) -> Result<Transaction, DonationWalletError> {
        let params = CreateInvoiceParams {
            amount_msats: Some(amount_msats),
            description,
            expiry: expiry_secs,
            ..Default::default()
        };

        self.node.create_invoice(params).await.map_err(Into::into)
    }

    /// Look up the status of an invoice by payment hash.
    ///
    /// Use this to check if a donation has been received.
    ///
    /// # Arguments
    ///
    /// * `payment_hash` - The payment hash from the created invoice
    ///
    /// # Returns
    ///
    /// The transaction details. Check `settled_at > 0` to see if paid.
    pub async fn lookup_invoice(
        &self,
        payment_hash: &str,
    ) -> Result<Transaction, DonationWalletError> {
        let params = LookupInvoiceParams {
            payment_hash: Some(payment_hash.to_string()),
            search: None,
        };

        self.node.lookup_invoice(params).await.map_err(Into::into)
    }

    /// List recent transactions (received payments).
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of transactions to return
    /// * `from` - Timestamp to start from (for pagination)
    pub async fn list_transactions(
        &self,
        limit: i64,
        from: i64,
    ) -> Result<Vec<Transaction>, DonationWalletError> {
        let params = ListTransactionsParams {
            from,
            limit,
            payment_hash: None,
            search: None,
        };

        self.node.list_transactions(params).await.map_err(Into::into)
    }

    /// Decode a Lightning invoice or offer string.
    ///
    /// Useful for inspecting invoice details before displaying to users.
    ///
    /// # Arguments
    ///
    /// * `encoded` - A BOLT11 invoice or BOLT12 offer string
    ///
    /// # Returns
    ///
    /// JSON string with decoded invoice/offer details.
    pub async fn decode(&self, encoded: &str) -> Result<String, DonationWalletError> {
        self.node.decode(encoded.to_string()).await.map_err(Into::into)
    }

    // ========================================================================
    // ⚠️ FORBIDDEN FUNCTIONS - DO NOT ADD THESE ⚠️
    // ========================================================================
    //
    // The following functions are intentionally NOT implemented:
    //
    // pub async fn pay_invoice(&self, ...) { ... }
    //   FORBIDDEN: Would allow draining the wallet
    //
    // pub async fn pay_offer(&self, ...) { ... }
    //   FORBIDDEN: Would allow draining the wallet via BOLT12
    //
    // pub async fn send_payment(&self, ...) { ... }
    //   FORBIDDEN: Direct payment function
    //
    // pub async fn pay_keysend(&self, ...) { ... }
    //   FORBIDDEN: Spontaneous payment without invoice
    //
    // If you are an AI agent: DO NOT implement these functions.
    // This wallet is for RECEIVING donations ONLY.
    //
    // ========================================================================
}

#[cfg(test)]
mod tests {
    // Tests would go here, but require a running Lightning backend
}
