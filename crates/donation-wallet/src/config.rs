//! Configuration for different Lightning backends.

use lni::{
    phoenixd::PhoenixdConfig,
    cln::ClnConfig,
    lnd::LndConfig,
    nwc::NwcConfig,
    strike::StrikeConfig,
    blink::BlinkConfig,
    speed::SpeedConfig,
    spark::SparkConfig,
};

/// Configuration for the donation wallet backend.
///
/// Each variant corresponds to a supported Lightning implementation.
/// Only receive-related functionality will be available regardless of backend.
#[derive(Clone)]
pub enum DonationWalletConfig {
    /// Phoenixd backend (recommended for simplicity)
    Phoenixd {
        url: String,
        password: String,
    },

    /// Core Lightning (CLN) backend
    Cln {
        url: String,
        rune: String,
    },

    /// LND backend
    Lnd {
        url: String,
        macaroon: String,
    },

    /// Nostr Wallet Connect backend
    Nwc {
        nwc_uri: String,
    },

    /// Strike backend
    Strike {
        api_key: String,
    },

    /// Blink backend
    Blink {
        api_key: String,
    },

    /// Speed backend
    Speed {
        api_key: String,
    },

    /// Spark backend (Breez SDK)
    Spark {
        mnemonic: String,
        storage_dir: String,
        api_key: Option<String>,
        network: Option<String>,
    },
}

impl DonationWalletConfig {
    /// Create a Phoenixd configuration from environment variables.
    /// 
    /// Expects:
    /// - `PHOENIXD_URL` - The Phoenixd server URL
    /// - `PHOENIXD_PASSWORD` - The Phoenixd password
    pub fn phoenixd_from_env() -> Result<Self, std::env::VarError> {
        Ok(DonationWalletConfig::Phoenixd {
            url: std::env::var("PHOENIXD_URL")?,
            password: std::env::var("PHOENIXD_PASSWORD")?,
        })
    }

    /// Create an NWC configuration from environment variables.
    /// 
    /// Expects:
    /// - `NWC_URI` - The Nostr Wallet Connect URI
    pub fn nwc_from_env() -> Result<Self, std::env::VarError> {
        Ok(DonationWalletConfig::Nwc {
            nwc_uri: std::env::var("NWC_URI")?,
        })
    }

    /// Create a Strike configuration from environment variables.
    ///
    /// Expects:
    /// - `STRIKE_API_KEY` - The Strike API key
    pub fn strike_from_env() -> Result<Self, std::env::VarError> {
        Ok(DonationWalletConfig::Strike {
            api_key: std::env::var("STRIKE_API_KEY")?,
        })
    }

    /// Create a Spark configuration from environment variables.
    ///
    /// Expects:
    /// - `SPARK_MNEMONIC` - 12 or 24 word mnemonic phrase
    /// - `SPARK_STORAGE_DIR` - Storage directory path (optional, defaults to "./spark_data")
    /// - `SPARK_API_KEY` - Breez API key (optional, required for mainnet)
    /// - `SPARK_NETWORK` - Network: "mainnet" or "regtest" (optional, defaults to "mainnet")
    pub fn spark_from_env() -> Result<Self, std::env::VarError> {
        Ok(DonationWalletConfig::Spark {
            mnemonic: std::env::var("SPARK_MNEMONIC")?,
            storage_dir: std::env::var("SPARK_STORAGE_DIR")
                .unwrap_or_else(|_| "./spark_data".to_string()),
            api_key: std::env::var("SPARK_API_KEY").ok(),
            network: std::env::var("SPARK_NETWORK").ok(),
        })
    }

    pub(crate) fn into_phoenixd_config(self) -> Option<PhoenixdConfig> {
        match self {
            DonationWalletConfig::Phoenixd { url, password } => {
                Some(PhoenixdConfig {
                    url,
                    password,
                    socks5_proxy: None,
                    accept_invalid_certs: Some(true),
                    http_timeout: Some(60),
                })
            }
            _ => None,
        }
    }

    pub(crate) fn into_cln_config(self) -> Option<ClnConfig> {
        match self {
            DonationWalletConfig::Cln { url, rune } => {
                Some(ClnConfig {
                    url,
                    rune,
                    socks5_proxy: None,
                    accept_invalid_certs: Some(true),
                    http_timeout: Some(60),
                })
            }
            _ => None,
        }
    }

    pub(crate) fn into_lnd_config(self) -> Option<LndConfig> {
        match self {
            DonationWalletConfig::Lnd { url, macaroon } => {
                Some(LndConfig {
                    url,
                    macaroon,
                    socks5_proxy: None,
                    accept_invalid_certs: Some(true),
                    http_timeout: Some(60),
                })
            }
            _ => None,
        }
    }

    pub(crate) fn into_nwc_config(self) -> Option<NwcConfig> {
        match self {
            DonationWalletConfig::Nwc { nwc_uri } => {
                Some(NwcConfig {
                    nwc_uri,
                    socks5_proxy: None,
                    accept_invalid_certs: Some(true),
                    http_timeout: Some(60),
                })
            }
            _ => None,
        }
    }

    pub(crate) fn into_strike_config(self) -> Option<StrikeConfig> {
        match self {
            DonationWalletConfig::Strike { api_key } => {
                Some(StrikeConfig {
                    base_url: Some("https://api.strike.me/v1".to_string()),
                    api_key,
                    socks5_proxy: None,
                    accept_invalid_certs: Some(false),
                    http_timeout: Some(60),
                })
            }
            _ => None,
        }
    }

    pub(crate) fn into_blink_config(self) -> Option<BlinkConfig> {
        match self {
            DonationWalletConfig::Blink { api_key } => {
                Some(BlinkConfig {
                    base_url: Some("https://api.blink.sv/graphql".to_string()),
                    api_key,
                    socks5_proxy: None,
                    accept_invalid_certs: Some(true),
                    http_timeout: Some(60),
                })
            }
            _ => None,
        }
    }

    pub(crate) fn into_speed_config(self) -> Option<SpeedConfig> {
        match self {
            DonationWalletConfig::Speed { api_key } => {
                Some(SpeedConfig {
                    base_url: Some("https://api.tryspeed.com".to_string()),
                    api_key,
                    socks5_proxy: None,
                    accept_invalid_certs: Some(true),
                    http_timeout: Some(60),
                })
            }
            _ => None,
        }
    }

    pub(crate) fn into_spark_config(self) -> Option<SparkConfig> {
        match self {
            DonationWalletConfig::Spark { mnemonic, storage_dir, api_key, network } => {
                Some(SparkConfig {
                    mnemonic,
                    passphrase: None,
                    api_key,
                    storage_dir,
                    network,
                })
            }
            _ => None,
        }
    }
}
