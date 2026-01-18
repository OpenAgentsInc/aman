//! Integration tests for donation-wallet using Spark backend
//!
//! Run with: cargo test -p donation-wallet --test spark_integration -- --nocapture
//!
//! Required environment variables:
//! - SPARK_MNEMONIC: 12 or 24 word BIP39 mnemonic
//! - SPARK_API_KEY: Breez API key (for mainnet)
//!
//! Optional:
//! - SPARK_STORAGE_DIR: Storage directory (default: ./spark_test_data)
//! - SPARK_NETWORK: mainnet or regtest (default: mainnet)
//! - SPARK_TEST_PAYMENT_HASH: Payment hash for testing invoice event monitoring

use std::env;
use std::sync::{Arc, Mutex};

fn should_skip() -> bool {
    env::var("SPARK_MNEMONIC").is_err() || env::var("SPARK_API_KEY").is_err()
}

/// Direct test using LNI SparkNode
/// Tests: get_info (balance), create_invoice, lookup_invoice, list_transactions
mod spark_tests {
    use super::*;
    use lni::spark::{SparkConfig, SparkNode};
    use lni::CreateInvoiceParams;

    async fn get_spark_node() -> Result<SparkNode, lni::ApiError> {
        let config = SparkConfig {
            mnemonic: env::var("SPARK_MNEMONIC").expect("SPARK_MNEMONIC required"),
            passphrase: env::var("SPARK_PASSPHRASE").ok(),
            api_key: env::var("SPARK_API_KEY").ok(),
            storage_dir: env::var("SPARK_STORAGE_DIR")
                .unwrap_or_else(|_| "./spark_test_data".to_string()),
            network: Some(env::var("SPARK_NETWORK").unwrap_or_else(|_| "mainnet".to_string())),
        };
        SparkNode::new(config).await
    }

    #[tokio::test]
    async fn test_get_wallet_balance() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_spark_node().await.expect("Failed to connect to Spark");
        
        match node.get_info().await {
            Ok(info) => {
                println!("=== Wallet Balance ===");
                println!("Alias: {}", info.alias);
                println!("Pubkey: {}", info.pubkey);
                println!("Network: {}", info.network);
                println!("Block Height: {}", info.block_height);
                println!();
                println!("Send Balance: {} msats ({} sats)", 
                    info.send_balance_msat, 
                    info.send_balance_msat / 1000);
                println!("Receive Balance: {} msats ({} sats)", 
                    info.receive_balance_msat,
                    info.receive_balance_msat / 1000);
                println!("Fee Credit Balance: {} msats ({} sats)", 
                    info.fee_credit_balance_msat,
                    info.fee_credit_balance_msat / 1000);
                println!();
                println!("Unsettled Send: {} msats", info.unsettled_send_balance_msat);
                println!("Unsettled Receive: {} msats", info.unsettled_receive_balance_msat);
                println!("Pending Open Send: {} msats", info.pending_open_send_balance);
                println!("Pending Open Receive: {} msats", info.pending_open_receive_balance);
            }
            Err(e) => {
                panic!("Failed to get wallet info: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_list_transactions() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_spark_node().await.expect("Failed to connect to Spark");

        let params = lni::ListTransactionsParams {
            from: 0,
            limit: 10,
            payment_hash: None,
            search: None,
        };

        match node.list_transactions(params).await {
            Ok(transactions) => {
                println!("=== Transaction History ===");
                println!("Found {} transactions", transactions.len());
                
                for (i, tx) in transactions.iter().enumerate() {
                    println!("\n--- Transaction {} ---", i + 1);
                    println!("Type: {}", tx.type_);
                    println!("Amount: {} msats ({} sats)", tx.amount_msats, tx.amount_msats / 1000);
                    println!("Payment Hash: {}", tx.payment_hash);
                    println!("Description: {}", tx.description);
                    println!("Created: {}", tx.created_at);
                    println!("Settled: {} (0 = pending)", tx.settled_at);
                }
            }
            Err(e) => {
                panic!("Failed to list transactions: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_create_invoice() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_spark_node().await.expect("Failed to connect to Spark");

        let params = CreateInvoiceParams {
            amount_msats: Some(1000), // 1 sat
            description: Some("Test donation invoice".to_string()),
            expiry: Some(3600), // 1 hour
            ..Default::default()
        };

        match node.create_invoice(params).await {
            Ok(invoice) => {
                println!("=== Created Invoice ===");
                println!("Invoice: {}", invoice.invoice);
                println!("Payment Hash: {}", invoice.payment_hash);
                println!("Amount: {} msats ({} sats)", invoice.amount_msats, invoice.amount_msats / 1000);
                println!("Description: {}", invoice.description);
                println!("Created At: {}", invoice.created_at);
                println!("Expires At: {}", invoice.expires_at);
                
                assert!(!invoice.invoice.is_empty(), "Invoice string should not be empty");
                assert!(!invoice.payment_hash.is_empty(), "Payment hash should not be empty");
                assert_eq!(invoice.amount_msats, 1000, "Amount should be 1000 msats");
            }
            Err(e) => {
                panic!("Failed to create invoice: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_lookup_invoice() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_spark_node().await.expect("Failed to connect to Spark");

        // First create an invoice to look up
        let params = CreateInvoiceParams {
            amount_msats: Some(2000), // 2 sats
            description: Some("Invoice to lookup".to_string()),
            expiry: Some(3600),
            ..Default::default()
        };

        let created = node.create_invoice(params).await.expect("Failed to create invoice");
        println!("Created invoice with payment_hash: {}", created.payment_hash);

        // Now look it up
        let lookup_params = lni::LookupInvoiceParams {
            payment_hash: Some(created.payment_hash.clone()),
            search: None,
        };

        match node.lookup_invoice(lookup_params).await {
            Ok(invoice) => {
                println!("=== Looked Up Invoice ===");
                println!("Invoice: {}", invoice.invoice);
                println!("Payment Hash: {}", invoice.payment_hash);
                println!("Amount: {} msats ({} sats)", invoice.amount_msats, invoice.amount_msats / 1000);
                println!("Settled At: {} (0 = not paid)", invoice.settled_at);
                
                assert_eq!(invoice.payment_hash, created.payment_hash, "Payment hash should match");
                println!("✓ Payment hash matches!");
            }
            Err(e) => {
                panic!("Failed to lookup invoice: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_create_zero_amount_invoice() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_spark_node().await.expect("Failed to connect to Spark");

        // Create a zero-amount invoice (amount not specified)
        // This allows the payer to choose how much to send
        let params = CreateInvoiceParams {
            amount_msats: None, // Zero/any amount - payer decides
            description: Some("Donate any amount".to_string()),
            expiry: Some(3600), // 1 hour
            ..Default::default()
        };

        match node.create_invoice(params).await {
            Ok(invoice) => {
                println!("=== Created Zero-Amount Invoice ===");
                println!("Invoice: {}", invoice.invoice);
                println!("Payment Hash: {}", invoice.payment_hash);
                println!("Amount: {} msats (0 = any amount)", invoice.amount_msats);
                println!("Description: {}", invoice.description);
                println!("Created At: {}", invoice.created_at);
                println!("Expires At: {}", invoice.expires_at);
                
                assert!(!invoice.invoice.is_empty(), "Invoice string should not be empty");
                assert!(!invoice.payment_hash.is_empty(), "Payment hash should not be empty");
                assert_eq!(invoice.amount_msats, 0, "Amount should be 0 for zero-amount invoice");
                println!("✓ Zero-amount invoice created successfully!");
            }
            Err(e) => {
                panic!("Failed to create zero-amount invoice: {}", e);
            }
        }
    }

    /// Test invoice event monitoring to check if an invoice was paid.
    ///
    /// This test demonstrates how to use `on_invoice_events` to monitor an invoice
    /// for payment status changes. The callback receives events:
    /// - `success`: Invoice has been paid and settled
    /// - `pending`: Invoice exists but payment not yet confirmed  
    /// - `failure`: Invoice not found or monitoring timed out
    ///
    /// Requires `SPARK_TEST_PAYMENT_HASH` env var set to a valid payment hash.
    /// Use a payment hash from a previously paid invoice to see SUCCESS,
    /// or an unpaid invoice's hash to see PENDING events until timeout.
    #[tokio::test]
    async fn test_on_invoice_events() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let test_payment_hash = match env::var("SPARK_TEST_PAYMENT_HASH") {
            Ok(hash) => hash,
            Err(_) => {
                println!("Skipping test: SPARK_TEST_PAYMENT_HASH not set");
                return;
            }
        };

        println!("=== Testing Invoice Event Monitoring ===");
        println!("Monitoring payment hash: {}", test_payment_hash);

        let node = get_spark_node().await.expect("Failed to connect to Spark");

        // Track events received
        #[derive(Debug, Clone)]
        enum InvoiceEvent {
            Success(Option<lni::Transaction>),
            Pending(Option<lni::Transaction>),
            Failure(Option<lni::Transaction>),
        }

        struct TestCallback {
            events: Arc<Mutex<Vec<InvoiceEvent>>>,
        }

        impl lni::types::OnInvoiceEventCallback for TestCallback {
            fn success(&self, transaction: Option<lni::Transaction>) {
                println!("✓ SUCCESS event received!");
                if let Some(ref tx) = transaction {
                    println!("  Payment Hash: {}", tx.payment_hash);
                    println!("  Amount: {} msats ({} sats)", tx.amount_msats, tx.amount_msats / 1000);
                    println!("  Description: {}", tx.description);
                    println!("  Settled At: {}", tx.settled_at);
                }
                let mut events = self.events.lock().unwrap();
                events.push(InvoiceEvent::Success(transaction));
            }

            fn pending(&self, transaction: Option<lni::Transaction>) {
                println!("⏳ PENDING event received");
                if let Some(ref tx) = transaction {
                    println!("  Payment Hash: {}", tx.payment_hash);
                    println!("  Amount: {} msats", tx.amount_msats);
                }
                let mut events = self.events.lock().unwrap();
                events.push(InvoiceEvent::Pending(transaction));
            }

            fn failure(&self, transaction: Option<lni::Transaction>) {
                println!("✗ FAILURE event received");
                if let Some(ref tx) = transaction {
                    println!("  Payment Hash: {}", tx.payment_hash);
                }
                let mut events = self.events.lock().unwrap();
                events.push(InvoiceEvent::Failure(transaction));
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let callback = TestCallback {
            events: events.clone(),
        };

        // Set up polling parameters
        let params = lni::types::OnInvoiceEventParams {
            payment_hash: Some(test_payment_hash.clone()),
            search: None,
            polling_delay_sec: 2,  // Check every 2 seconds
            max_polling_sec: 10,   // Poll for up to 10 seconds
        };

        println!("Starting invoice event monitoring...");
        println!("Polling every {} seconds for up to {} seconds", 
            params.polling_delay_sec, params.max_polling_sec);

        // Start monitoring - this will block until timeout or payment received
        node.on_invoice_events(params, Arc::new(callback)).await;

        // Check results
        let captured_events = events.lock().unwrap();
        println!("\n=== Event Summary ===");
        println!("Total events captured: {}", captured_events.len());

        for (i, event) in captured_events.iter().enumerate() {
            match event {
                InvoiceEvent::Success(_) => println!("  Event {}: SUCCESS (paid!)", i + 1),
                InvoiceEvent::Pending(_) => println!("  Event {}: PENDING", i + 1),
                InvoiceEvent::Failure(_) => println!("  Event {}: FAILURE/NOT_FOUND", i + 1),
            }
        }

        // We should have received at least one event
        assert!(
            !captured_events.is_empty(),
            "Should capture at least one invoice event"
        );

        println!("✓ Invoice event monitoring test complete!");
    }
}
