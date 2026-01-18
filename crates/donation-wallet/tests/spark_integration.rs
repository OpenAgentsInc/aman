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

use std::env;

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
}
