use std::net::SocketAddr;

use rand::Rng;
use sqlx::postgres::PgPoolOptions;
use tokio::signal;
use tracing_subscriber::EnvFilter;

use webhooker::auth::password;
use webhooker::config::Config;
use webhooker::db;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let config = Config::from_env().expect("Failed to load configuration");

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new(&config.log_level)
        }))
        .init();

    tracing::info!("Starting Webhooker");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Migrations applied");

    let user_count = db::users::count_all(&pool)
        .await
        .expect("Failed to count users");

    if user_count == 0 {
        let admin_password: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        let pw_hash = password::hash(&admin_password).expect("Failed to hash password");

        let tenant = db::tenants::create(&pool, "Default", "default")
            .await
            .expect("Failed to create default tenant");

        db::users::create(
            &pool,
            tenant.id,
            "admin@localhost",
            &pw_hash,
            "Admin",
            "owner",
            true,
        )
        .await
        .expect("Failed to create admin user");

        tracing::info!("========================================");
        tracing::info!("  Admin account created!");
        tracing::info!("  Email:    admin@localhost");
        tracing::info!("  Password: {admin_password}");
        tracing::info!("  Change this password after first login.");
        tracing::info!("========================================");
    }

    let addr = SocketAddr::new(config.host, config.port);
    let worker_count = config.worker_count;
    let (app, state) = webhooker::build_app(pool, config);

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let cleanup_state = state.clone();
    let mut cleanup_shutdown = shutdown_rx.clone();
    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(5 * 60);
        let max_age = std::time::Duration::from_secs(30 * 60);
        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {}
                _ = cleanup_shutdown.changed() => break,
            }
            cleanup_state.submission_limiter.cleanup(max_age);
            cleanup_state.login_limiter.cleanup(max_age);
        }
    });

    let worker_pool = webhooker::worker::run_pool(state, shutdown_rx, worker_count);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Listening on {addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    let _ = shutdown_tx.send(true);
    let _ = worker_pool.join();

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown");
}
