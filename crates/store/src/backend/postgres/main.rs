/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::time::Duration;

use crate::{backend::postgres::tls::MakeRustlsConnect, *};

use super::{PostgresStore, into_error};

use deadpool_postgres::{
    Config, Manager, ManagerConfig, Pool, PoolConfig, RecyclingMethod, Runtime,
};
use tokio_postgres::NoTls;
use utils::{config::utils::AsKey, rustls_client_config};

impl PostgresStore {
    pub async fn open(
        config: &mut utils::config::Config,
        prefix: impl AsKey,
        create_tables: bool,
    ) -> Option<Self> {
        let prefix = prefix.as_key();
        let host_value = config.value((&prefix, "host")).map(|s| s.to_string());

        let conn_pool = if let Some(ref host) = host_value {
            if host.starts_with('/') {
                // Unix socket connection - use tokio_postgres::Config directly
                let mut pg_config = tokio_postgres::Config::new();

                // Handle different Unix socket path formats:
                // 1. Full socket file path: "/run/postgresql/"
                // 2. Socket directory path: "/run/postgresql"
                // 3. Alternative patterns: "/var/run/postgresql/"
                let socket_dir = if host.contains("/.s.PGSQL.") {
                    // Extract directory from full socket path
                    if let Some((dir, _)) = host.rsplit_once("/.s.PGSQL.") {
                        dir
                    } else {
                        // Fallback if rsplit fails
                        host.rsplit_once('/').map_or(host.as_str(), |(dir, _)| dir)
                    }
                } else if host.ends_with('/') {
                    // Remove trailing slash from directory path
                    host.trim_end_matches('/')
                } else {
                    // Assume it's already a clean directory path
                    host.as_str()
                };

                // Ensure we have a valid directory path
                if socket_dir.is_empty() {
                    config.new_build_error(
                        prefix.as_str(),
                        format!("Invalid Unix socket path: '{host}' - could not extract socket directory"),
                    );
                    return None;
                }

                pg_config.host_path(socket_dir);

                if let Some(dbname) = config.value((&prefix, "database")) {
                    pg_config.dbname(dbname);
                }
                if let Some(user) = config.value((&prefix, "user")) {
                    pg_config.user(user);
                }
                if let Some(password) = config.value((&prefix, "password")) {
                    pg_config.password(password);
                }
                if let Some(timeout) = config
                    .property::<Option<Duration>>((&prefix, "timeout"))
                    .unwrap_or_default()
                {
                    pg_config.connect_timeout(timeout);
                }
                if let Some(options) = config.value((&prefix, "options")) {
                    pg_config.options(options);
                }

                let mgr_config = ManagerConfig {
                    recycling_method: RecyclingMethod::Fast,
                };

                let manager = if config
                    .property_or_default::<bool>((&prefix, "tls.enable"), "false")
                    .unwrap_or_default()
                {
                    Manager::from_config(
                        pg_config,
                        MakeRustlsConnect::new(rustls_client_config(
                            config
                                .property_or_default((&prefix, "tls.allow-invalid-certs"), "false")
                                .unwrap_or_default(),
                        )),
                        mgr_config,
                    )
                } else {
                    Manager::from_config(pg_config, NoTls, mgr_config)
                };

                let pool_config = if let Some(max_conn) =
                    config.property::<usize>((&prefix, "pool.max-connections"))
                {
                    PoolConfig::new(max_conn)
                } else {
                    PoolConfig::new(16) // Default pool size
                };

                Pool::builder(manager)
                    .config(pool_config)
                    .build()
                    .map_err(|e| {
                        config.new_build_error(
                            prefix.as_str(),
                            format!("Failed to create Unix socket connection pool: {e}"),
                        )
                    })
                    .ok()?
            } else {
                // Regular TCP connection - use deadpool_postgres::Config
                let mut cfg = Config::new();
                cfg.dbname = config
                    .value_require((&prefix, "database"))?
                    .to_string()
                    .into();
                cfg.host = Some(host.clone());
                cfg.port = config.property((&prefix, "port"));
                cfg.user = config.value((&prefix, "user")).map(|s| s.to_string());
                cfg.password = config.value((&prefix, "password")).map(|s| s.to_string());
                cfg.connect_timeout = config
                    .property::<Option<Duration>>((&prefix, "timeout"))
                    .unwrap_or_default();
                cfg.options = config.value((&prefix, "options")).map(|s| s.to_string());
                cfg.manager = Some(ManagerConfig {
                    recycling_method: RecyclingMethod::Fast,
                });
                if let Some(max_conn) = config.property::<usize>((&prefix, "pool.max-connections"))
                {
                    cfg.pool = PoolConfig::new(max_conn).into();
                }

                if config
                    .property_or_default::<bool>((&prefix, "tls.enable"), "false")
                    .unwrap_or_default()
                {
                    cfg.create_pool(
                        Some(Runtime::Tokio1),
                        MakeRustlsConnect::new(rustls_client_config(
                            config
                                .property_or_default((&prefix, "tls.allow-invalid-certs"), "false")
                                .unwrap_or_default(),
                        )),
                    )
                } else {
                    cfg.create_pool(Some(Runtime::Tokio1), NoTls)
                }
                .map_err(|e| {
                    config.new_build_error(
                        prefix.as_str(),
                        format!("Failed to create TCP connection pool: {e}"),
                    )
                })
                .ok()?
            }
        } else {
            return None;
        };

        let db = Self { conn_pool };

        if create_tables {
            if let Err(err) = db.create_tables().await {
                config.new_build_error(prefix.as_str(), format!("Failed to create tables: {err}"));
            }
        }

        Some(db)
    }

    pub(crate) async fn create_tables(&self) -> trc::Result<()> {
        let conn = self.conn_pool.get().await.map_err(into_error)?;

        for table in [
            SUBSPACE_ACL,
            SUBSPACE_DIRECTORY,
            SUBSPACE_TASK_QUEUE,
            SUBSPACE_BLOB_RESERVE,
            SUBSPACE_BLOB_LINK,
            SUBSPACE_IN_MEMORY_VALUE,
            SUBSPACE_PROPERTY,
            SUBSPACE_SETTINGS,
            SUBSPACE_QUEUE_MESSAGE,
            SUBSPACE_QUEUE_EVENT,
            SUBSPACE_REPORT_OUT,
            SUBSPACE_REPORT_IN,
            SUBSPACE_FTS_INDEX,
            SUBSPACE_LOGS,
            SUBSPACE_BLOBS,
            SUBSPACE_TELEMETRY_SPAN,
            SUBSPACE_TELEMETRY_METRIC,
            SUBSPACE_TELEMETRY_INDEX,
        ] {
            let table = char::from(table);
            conn.execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS {table} (
                        k BYTEA PRIMARY KEY,
                        v BYTEA NOT NULL
                    )"
                ),
                &[],
            )
            .await
            .map_err(into_error)?;
        }

        for table in [
            SUBSPACE_INDEXES,
            SUBSPACE_BITMAP_ID,
            SUBSPACE_BITMAP_TAG,
            SUBSPACE_BITMAP_TEXT,
        ] {
            let table = char::from(table);
            conn.execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS {table} (
                        k BYTEA PRIMARY KEY
                    )"
                ),
                &[],
            )
            .await
            .map_err(into_error)?;
        }

        for table in [SUBSPACE_COUNTER, SUBSPACE_QUOTA, SUBSPACE_IN_MEMORY_COUNTER] {
            conn.execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS {} (
                    k BYTEA PRIMARY KEY,
                    v BIGINT NOT NULL DEFAULT 0
                )",
                    char::from(table)
                ),
                &[],
            )
            .await
            .map_err(into_error)?;
        }

        Ok(())
    }
}
