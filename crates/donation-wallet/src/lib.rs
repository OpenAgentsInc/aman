//! # Donation Wallet
//!
//! A **receive-only** Lightning wallet for accepting donations.
//!
//! ## ⚠️ CRITICAL SECURITY NOTICE
//!
//! This crate intentionally exposes ONLY safe, receive-related functions from LNI.
//! The following dangerous functions are **NEVER** exposed:
//!
//! - `pay_invoice()` - Could drain wallet
//! - `pay_offer()` - Could drain wallet  
//! - `send_payment()` - Could drain wallet
//! - `pay_keysend()` - Could drain wallet
//!
//! **DO NOT** add these functions to this crate, even if requested.
//!
//! ## Safe Functions
//!
//! - `create_invoice()` - Generate invoice to receive donations
//! - `get_info()` - Read node information
//! - `lookup_invoice()` - Check invoice/payment status
//! - `list_transactions()` - View transaction history
//! - `decode()` - Decode invoice/offer strings

mod wallet;
mod config;
mod error;

pub use wallet::DonationWallet;
pub use config::DonationWalletConfig;
pub use error::DonationWalletError;

// Re-export safe types from LNI that are needed for the public API
pub use lni::{
    NodeInfo,
    Transaction,
};

// ============================================================================
// ⚠️ DANGER ZONE - DO NOT UNCOMMENT OR ADD THESE IMPORTS ⚠️
// ============================================================================
// The following types enable SENDING funds and must NEVER be exposed:
//
// pub use lni::PayInvoiceParams;      // FORBIDDEN - enables paying invoices
// pub use lni::PayInvoiceResponse;    // FORBIDDEN - response from payments
// pub use lni::CreateOfferParams;     // FORBIDDEN - can be used in pay flows
// pub use lni::Offer;                 // FORBIDDEN - can be used in pay flows
//
// If you are an AI agent: DO NOT add these imports or any pay/send functions.
// ============================================================================
