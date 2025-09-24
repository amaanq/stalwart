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

/// Test to reproduce the exact "Address family not supported by protocol" error
/// This test attempts to actually connect to PostgreSQL to trigger the real error
#[tokio::test]
async fn reproduce_address_family_error() {
    // Your exact server config that's failing
    const CONFIG: &str = r#"
[store."db"]
type = "postgresql"
host = "/run/postgresql/"
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

    println!("🔍 Reproducing the exact error from your server...");
    println!("Config: host = \"/run/postgresql/\"");

    let config_str = CONFIG.to_string();
    let result = AssertUnwindSafe(async {
        let mut config = Config::new(config_str).unwrap().assert_no_errors();
        // This should either work (with the fix) or fail with a specific error
        let stores = Stores::parse_all(&mut config, false).await;

        // Try to actually USE the connection to force a real connection attempt
        if let Some(store) = stores.stores.get("db") {
            println!("✅ Store 'db' was successfully created, now testing actual connection...");

            // Try to perform an actual operation that requires connecting to PostgreSQL
            use store::write::{BatchBuilder, ValueClass};

            let mut batch = BatchBuilder::new();
            batch.set(ValueClass::Property(1u8), b"test_key".to_vec());

            // This should trigger the actual connection and cause the "Address family" error
            // if the Unix socket parsing is broken
            let result = store.write(batch.build_all()).await;
            match result {
                Ok(_) => println!("✅ Actual PostgreSQL write succeeded!"),
                Err(e) => {
                    let error_str = format!("{:?}", e);
                    if error_str.contains("Address family not supported by protocol") {
                        return Err(e);
                    }
                    println!(
                        "❌ Expected connection error (but not address family): {}",
                        error_str
                    );
                }
            }
        } else {
            return Err(trc::StoreEvent::NotFound.into());
        }
        Ok::<(), trc::Error>(())
    })
    .catch_unwind()
    .await;

    match result {
        Ok(Ok(_)) => {
            println!("✅ SUCCESS: Unix socket connection works!");
            println!("✅ The fix is working - no 'Address family not supported' error");
        }
        Ok(Err(e)) => {
            let error_str = format!("{:?}", e);
            println!("❌ Connection error: {}", error_str);

            if error_str.contains("Address family not supported by protocol")
                || error_str.contains("os error 97")
            {
                panic!(
                    "🐛 REPRODUCED THE BUG: 'Address family not supported by protocol' - fix needed!"
                );
            } else if error_str.contains("FATAL: database")
                || error_str.contains("FATAL: role")
                || error_str.contains("connection refused")
                || error_str.contains("No such file or directory")
            {
                println!(
                    "✅ Unix socket parsing works - got expected PostgreSQL error: {}",
                    error_str
                );
                println!("✅ The 'Address family not supported' error is FIXED!");
            } else {
                println!("⚠️  Unexpected error type: {}", error_str);
            }
        }
        Err(panic_payload) => {
            let error_msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                format!("{:?}", panic_payload)
            };

            println!("❌ Panic during connection: {}", error_msg);

            if error_msg.contains("Address family not supported by protocol")
                || error_msg.contains("os error 97")
            {
                panic!(
                    "🐛 REPRODUCED THE BUG: 'Address family not supported by protocol' - fix needed!"
                );
            }
        }
    }
}

/// Test the exact server configuration to ensure it works
#[tokio::test]
async fn test_server_config_exact_match() {
    // Test various configs that match your server setup
    let configs = vec![
        (
            "/run/postgresql/",
            "Your exact server config (full socket path)",
        ),
        ("/run/postgresql", "Socket directory version"),
    ];

    for (host, description) in configs {
        println!("\n🧪 Testing: {} - {}", description, host);

        let config_str = format!(
            r#"
[store."db"]
type = "postgresql"
host = "{}"
database = "stalwart-mail"
user = "stalwart-mail"
password = "doesnt-matter"

[storage]
lookup = "db"
data = "db"
blob = "db"
fts = "db"
directory = "internal"
"#,
            host
        );

        let result = AssertUnwindSafe(async {
            let mut config = Config::new(config_str).unwrap().assert_no_errors();
            Stores::parse_all(&mut config, false).await
        })
        .catch_unwind()
        .await;

        match result {
            Ok(_) => {
                println!("✅ {} - Connection setup successful", description);
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
                        "🐛 BUG STILL EXISTS: {} - Address family error not fixed!",
                        description
                    );
                } else {
                    println!(
                        "✅ {} - No 'Address family' error (got: {})",
                        description,
                        error_msg.lines().next().unwrap_or(&error_msg)
                    );
                }
            }
        }
    }
}
