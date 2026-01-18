//! Integration tests for signal-daemon.
//!
//! These tests require either:
//! 1. A running signal-cli daemon (for connection tests)
//! 2. AMAN_NUMBER env var set (for spawn tests)
//!
//! Run all integration tests:
//!   cargo test --test integration_tests
//!
//! Run only tests that don't need a daemon:
//!   cargo test --test integration_tests -- --skip daemon
//!
//! Run ignored tests (require daemon):
//!   cargo test --test integration_tests -- --ignored

use signal_daemon::{DaemonConfig, DaemonError, ProcessConfig, SignalClient};
use std::env;
use std::time::Duration;

/// Helper to get test account from environment.
fn get_test_account() -> Option<String> {
    env::var("AMAN_NUMBER").ok()
}

/// Helper to get JAR path.
fn get_jar_path() -> String {
    env::var("SIGNAL_CLI_JAR").unwrap_or_else(|_| "../../build/signal-cli.jar".to_string())
}

/// Helper to check if JAR exists.
fn jar_exists() -> bool {
    std::path::Path::new(&get_jar_path()).exists()
}

// ============================================================================
// Unit tests (no daemon required)
// ============================================================================

mod config_tests {
    use super::*;

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert_eq!(config.base_url, "http://localhost:8080");
        assert!(config.account.is_none());
    }

    #[test]
    fn test_daemon_config_new() {
        let config = DaemonConfig::new("http://127.0.0.1:9000");
        assert_eq!(config.base_url, "http://127.0.0.1:9000");
        assert!(config.account.is_none());
    }

    #[test]
    fn test_daemon_config_with_account() {
        let config = DaemonConfig::with_account("http://localhost:8080", "+1234567890");
        assert_eq!(config.base_url, "http://localhost:8080");
        assert_eq!(config.account, Some("+1234567890".to_string()));
    }

    #[test]
    fn test_daemon_config_urls() {
        let config = DaemonConfig::new("http://localhost:8080");
        assert_eq!(config.rpc_url(), "http://localhost:8080/api/v1/rpc");
        assert_eq!(config.check_url(), "http://localhost:8080/api/v1/check");
        assert_eq!(config.events_url(), "http://localhost:8080/api/v1/events");
    }

    #[test]
    fn test_daemon_config_events_url_with_account() {
        let config = DaemonConfig::with_account("http://localhost:8080", "+1234567890");
        assert_eq!(
            config.events_url(),
            "http://localhost:8080/api/v1/events?account=%2B1234567890"
        );
    }
}

mod process_config_tests {
    use super::*;

    #[test]
    fn test_process_config_new() {
        let config = ProcessConfig::new("/path/to/signal-cli.jar", "+1234567890");
        assert_eq!(config.jar_path.to_str().unwrap(), "/path/to/signal-cli.jar");
        assert_eq!(config.account, "+1234567890");
        assert_eq!(config.http_addr, "127.0.0.1:8080");
        assert!(config.send_read_receipts);
        assert!(config.trust_new_identities);
    }

    #[test]
    fn test_process_config_with_http_addr() {
        let config = ProcessConfig::new("/path/to/jar", "+1234567890")
            .with_http_addr("0.0.0.0:9000");
        assert_eq!(config.http_addr, "0.0.0.0:9000");
    }

    #[test]
    fn test_process_config_base_url() {
        let config = ProcessConfig::new("/path/to/jar", "+1234567890");
        assert_eq!(config.base_url(), "http://127.0.0.1:8080");
    }

    #[test]
    fn test_process_config_to_daemon_config() {
        let config = ProcessConfig::new("/path/to/jar", "+1234567890");
        let daemon_config = config.to_daemon_config();
        assert_eq!(daemon_config.base_url, "http://127.0.0.1:8080");
        assert_eq!(daemon_config.account, Some("+1234567890".to_string()));
    }
}

mod send_params_tests {
    use signal_daemon::SendParams;

    #[test]
    fn test_send_params_text() {
        let params = SendParams::text("+1234567890", "Hello");
        assert_eq!(params.recipient, vec!["+1234567890".to_string()]);
        assert_eq!(params.message, Some("Hello".to_string()));
        assert!(params.group_id.is_empty());
    }

    #[test]
    fn test_send_params_group() {
        let params = SendParams::group("GROUP_ID", "Hello group");
        assert!(params.recipient.is_empty());
        assert_eq!(params.group_id, vec!["GROUP_ID".to_string()]);
        assert_eq!(params.message, Some("Hello group".to_string()));
    }

    #[test]
    fn test_send_params_with_attachment() {
        let params = SendParams::text("+1234567890", "Check this")
            .with_attachment("/path/to/file.jpg");
        assert_eq!(params.attachments, vec!["/path/to/file.jpg".to_string()]);
    }

    #[test]
    fn test_send_params_with_quote() {
        let params = SendParams::text("+1234567890", "Reply")
            .with_quote(12345, "+0987654321");
        assert_eq!(params.quote_timestamp, Some(12345));
        assert_eq!(params.quote_author, Some("+0987654321".to_string()));
    }
}

mod reconnect_config_tests {
    use signal_daemon::ReconnectConfig;
    use std::time::Duration;

    #[test]
    fn test_reconnect_config_default() {
        let config = ReconnectConfig::default();
        assert!(config.max_retries.is_none());
        assert_eq!(config.initial_delay, Duration::from_millis(500));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_reconnect_delay_calculation() {
        let config = ReconnectConfig::default();
        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(500));
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(1000));
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(2000));
    }

    #[test]
    fn test_reconnect_delay_max() {
        let config = ReconnectConfig::default();
        // After many attempts, should cap at max_delay
        assert_eq!(config.delay_for_attempt(10), Duration::from_secs(30));
    }

    #[test]
    fn test_should_retry_infinite() {
        let config = ReconnectConfig::default();
        assert!(config.should_retry(0));
        assert!(config.should_retry(100));
        assert!(config.should_retry(1000));
    }

    #[test]
    fn test_should_retry_limited() {
        let config = ReconnectConfig {
            max_retries: Some(3),
            ..Default::default()
        };
        assert!(config.should_retry(0));
        assert!(config.should_retry(2));
        assert!(!config.should_retry(3));
        assert!(!config.should_retry(4));
    }
}

// ============================================================================
// Integration tests (require running daemon or AMAN_NUMBER)
// ============================================================================

mod daemon_connection_tests {
    use super::*;

    /// Test connecting to a running daemon.
    /// Requires daemon to be running on localhost:8080.
    #[tokio::test]
    #[ignore = "requires running daemon"]
    async fn test_connect_to_daemon() {
        let config = DaemonConfig::default();
        let client = SignalClient::connect(config).await;
        assert!(client.is_ok(), "Failed to connect: {:?}", client.err());
    }

    /// Test health check against running daemon.
    #[tokio::test]
    #[ignore = "requires running daemon"]
    async fn test_health_check() {
        let config = DaemonConfig::default();
        let client = SignalClient::connect(config).await.unwrap();
        let healthy = client.health_check().await.unwrap();
        assert!(healthy);
    }

    /// Test getting version from daemon.
    #[tokio::test]
    #[ignore = "requires running daemon"]
    async fn test_version() {
        let config = DaemonConfig::default();
        let client = SignalClient::connect(config).await.unwrap();
        let version = client.version().await.unwrap();
        assert!(!version.is_empty());
        println!("signal-cli version: {}", version);
    }

    /// Test connection failure to non-existent daemon.
    #[tokio::test]
    async fn test_connect_failure() {
        let config = DaemonConfig::new("http://127.0.0.1:59999");
        let result = SignalClient::connect(config).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            DaemonError::Http(_) => {} // Expected
            e => panic!("Unexpected error type: {:?}", e),
        }
    }
}

mod daemon_spawn_tests {
    use super::*;

    /// Test spawning daemon from JAR.
    /// Requires AMAN_NUMBER env var and build/signal-cli.jar.
    #[tokio::test]
    #[ignore = "requires AMAN_NUMBER and JAR"]
    async fn test_spawn_daemon() {
        let account = get_test_account().expect("AMAN_NUMBER not set");
        let jar_path = get_jar_path();

        if !jar_exists() {
            panic!("JAR not found at {}. Run scripts/build-signal-cli.sh", jar_path);
        }

        let config = ProcessConfig::new(&jar_path, &account);
        let result = signal_daemon::spawn_and_connect(config, Duration::from_secs(30)).await;

        assert!(result.is_ok(), "Failed to spawn: {:?}", result.err());

        let (mut process, client) = result.unwrap();
        assert!(process.is_running());
        assert!(client.is_connected());

        let version = client.version().await.unwrap();
        assert!(!version.is_empty());

        // Process killed on drop
    }

    /// Test spawn with invalid JAR path.
    #[tokio::test]
    async fn test_spawn_invalid_jar() {
        let config = ProcessConfig::new("/nonexistent/path/signal-cli.jar", "+1234567890");
        let result = signal_daemon::spawn_and_connect(config, Duration::from_secs(5)).await;
        assert!(result.is_err());
    }
}

mod message_tests {
    use super::*;

    /// Test sending a message.
    /// Requires running daemon and a valid recipient.
    /// Set TEST_RECIPIENT env var to run.
    #[tokio::test]
    #[ignore = "requires daemon and TEST_RECIPIENT"]
    async fn test_send_message() {
        let recipient = env::var("TEST_RECIPIENT").expect("TEST_RECIPIENT not set");

        let config = DaemonConfig::default();
        let client = SignalClient::connect(config).await.unwrap();

        let result = client
            .send_text(&recipient, "Test message from integration tests")
            .await;

        assert!(result.is_ok(), "Failed to send: {:?}", result.err());
        let send_result = result.unwrap();
        assert!(send_result.timestamp > 0);
        println!("Sent message with timestamp: {}", send_result.timestamp);
    }
}

mod sse_tests {
    use super::*;
    use futures::StreamExt;

    /// Test SSE subscription.
    /// Requires running daemon.
    #[tokio::test]
    #[ignore = "requires running daemon"]
    async fn test_subscribe() {
        let config = DaemonConfig::default();
        let client = SignalClient::connect(config).await.unwrap();

        let mut stream = signal_daemon::subscribe(&client).unwrap();

        // Just verify the stream can be created and polled
        // In a real test, you'd send a message from another device
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                println!("No messages received (expected in automated test)");
            }
            result = stream.next() => {
                if let Some(Ok(envelope)) = result {
                    println!("Received message from: {}", envelope.source);
                }
            }
        }
    }
}
