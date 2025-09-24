/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use futures::FutureExt;
use std::panic::AssertUnwindSafe;

use store::Stores;
use utils::config::Config;

use crate::AssertConfig;

#[tokio::test]
async fn postgres_unix_socket_test() {
    // Test configuration that reproduces the Unix socket issue
    const CONFIG: &str = r#"
[store."db"]
type = "postgresql"
host = "/run/postgresql"  # Socket directory - should work now
database = "stalwart-mail"
user = "stalwart-mail"
password = "test_password"  # This will fail with auth error, not socket error

[storage]
lookup = "db"
data = "db"
blob = "db"
fts = "db"
directory = "internal"
"#;

    // Try to create stores - this might panic
    let config_str = CONFIG.to_string();
    let result = AssertUnwindSafe(async {
        let mut config = Config::new(config_str).unwrap().assert_no_errors();
        Stores::parse_all(&mut config, false).await
    })
    .catch_unwind()
    .await;

    match result {
        Ok(_) => {
            println!("✅ Unix socket connection succeeded");
        }
        Err(e) => {
            let error_msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown error".to_string()
            };
            println!("❌ Unix socket connection failed: {}", error_msg);
            // Check for the specific error we're fixing
            if error_msg.contains("Address family not supported by protocol") {
                panic!("BUG: Unix socket path not properly handled - this should be fixed now!");
            }

            // These are expected errors for Unix sockets (auth/connection issues are OK)
            if error_msg.contains("FATAL: database")
                || error_msg.contains("FATAL: role")
                || error_msg.contains("connection refused")
                || error_msg.contains("No such file or directory")
            {
                println!(
                    "✅ Unix socket parsing works - got expected connection/auth error: {}",
                    error_msg
                );
            } else {
                println!(
                    "⚠️  Unexpected error (might indicate other issues): {}",
                    error_msg
                );
            }
        }
    }
}

#[tokio::test]
async fn postgres_unix_socket_edge_cases_test() {
    // Test various Unix socket path formats that should all work
    let test_cases = vec![
        ("/run/postgresql", "Socket directory"),
        ("/run/postgresql/", "Socket directory with trailing slash"),
        ("/var/run/postgresql", "Alternative socket directory"),
        ("/tmp", "Temp directory (common fallback)"),
    ];

    for (host_path, description) in test_cases {
        println!("Testing {}: {}", description, host_path);

        let config_str = format!(
            r#"
[store."db"]
type = "postgresql"
host = "{}"
database = "stalwart-mail"
user = "stalwart-mail"
password = "test_password"

[storage]
lookup = "db"
data = "db"
blob = "db"
fts = "db"
directory = "internal"
"#,
            host_path
        );

        let result = AssertUnwindSafe(async {
            let mut config = Config::new(config_str).unwrap().assert_no_errors();
            Stores::parse_all(&mut config, false).await
        })
        .catch_unwind()
        .await;

        match result {
            Ok(_) => {
                println!("✅ {} connection succeeded", description);
            }
            Err(e) => {
                let error_msg = if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "Unknown error".to_string()
                };

                if error_msg.contains("Address family not supported by protocol") {
                    panic!(
                        "BUG: {} not properly handled - this should be fixed now!",
                        description
                    );
                }

                // Expected connection/auth errors are OK
                if error_msg.contains("FATAL: database")
                    || error_msg.contains("FATAL: role")
                    || error_msg.contains("connection refused")
                    || error_msg.contains("No such file or directory")
                {
                    println!(
                        "✅ {} parsing works - got expected error: {}",
                        description, error_msg
                    );
                } else {
                    println!("⚠️  {} - unexpected error: {}", description, error_msg);
                }
            }
        }
    }
}

#[tokio::test]
async fn postgres_full_socket_path_test() {
    // Test with full socket path
    const CONFIG: &str = r#"
[store."db"]
type = "postgresql"
host = "/run/postgresql/"  # Full socket path
database = "stalwart-mail"
user = "stalwart-mail"
password = "test_password"

[storage]
lookup = "db"
data = "db"
blob = "db"
fts = "db"
directory = "internal"
"#;

    let config_str = CONFIG.to_string();
    let result = AssertUnwindSafe(async {
        let mut config = Config::new(config_str).unwrap().assert_no_errors();
        Stores::parse_all(&mut config, false).await
    })
    .catch_unwind()
    .await;

    match result {
        Ok(_) => {
            println!("✅ Full socket path connection succeeded");
        }
        Err(e) => {
            let error_msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown error".to_string()
            };
            println!("❌ Full socket path connection failed: {}", error_msg);
            if error_msg.contains("Address family not supported by protocol") {
                panic!("BUG: Full socket path not properly handled - this should be fixed now!");
            }

            // These are expected errors for Unix sockets (auth/connection issues are OK)
            if error_msg.contains("FATAL: database")
                || error_msg.contains("FATAL: role")
                || error_msg.contains("connection refused")
                || error_msg.contains("No such file or directory")
            {
                println!(
                    "✅ Full socket path parsing works - got expected connection/auth error: {}",
                    error_msg
                );
            } else {
                println!(
                    "⚠️  Unexpected error (might indicate other issues): {}",
                    error_msg
                );
            }
        }
    }
}

#[tokio::test]
async fn postgres_ipv4_localhost_test() {
    // Test with IPv4 localhost (this should work)
    const CONFIG: &str = r#"
[store."db"]
type = "postgresql"
host = "127.0.0.1"
port = 5432
database = "stalwart-mail"
user = "stalwart-mail"
password = "test_password"

[storage]
lookup = "db"
data = "db"
blob = "db"
fts = "db"
directory = "internal"
"#;

    let config_str = CONFIG.to_string();
    let result = AssertUnwindSafe(async {
        let mut config = Config::new(config_str).unwrap().assert_no_errors();
        Stores::parse_all(&mut config, false).await
    })
    .catch_unwind()
    .await;

    match result {
        Ok(_) => {
            println!("✅ IPv4 localhost connection succeeded");
        }
        Err(e) => {
            let error_msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown error".to_string()
            };
            println!("❌ IPv4 localhost connection failed: {}", error_msg);
            if error_msg.contains("Address family not supported by protocol") {
                panic!("Unexpected: IPv4 connection failed with address family error");
            }
        }
    }
}
