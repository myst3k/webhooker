use std::net::SocketAddr;

use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use webhooker::config::{Config, RegistrationMode};

/// A running test server instance with a dedicated test database.
pub struct TestApp {
    pub addr: SocketAddr,
    pub pool: PgPool,
    pub client: Client,
    pub db_name: String,
}

impl TestApp {
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    /// Register the bootstrap user (first user = system admin + owner).
    pub async fn register(&self, email: &str, password: &str, name: &str) -> (Value, StatusCode) {
        let resp = self
            .client
            .post(self.url("/api/v1/auth/register"))
            .json(&json!({ "email": email, "password": password, "name": name }))
            .send()
            .await
            .expect("register request failed");
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(json!(null));
        (body, status)
    }

    /// Login and return the auth response body + status.
    pub async fn login(&self, email: &str, password: &str) -> (Value, StatusCode) {
        let resp = self
            .client
            .post(self.url("/api/v1/auth/login"))
            .json(&json!({ "email": email, "password": password }))
            .send()
            .await
            .expect("login request failed");
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(json!(null));
        (body, status)
    }

    /// Register bootstrap user, return access token.
    pub async fn bootstrap(&self) -> String {
        let (body, status) = self.register("admin@test.com", "password123", "Admin").await;
        assert_eq!(status, StatusCode::OK, "bootstrap register failed: {body}");
        body["access_token"].as_str().unwrap().to_string()
    }

    /// Create a project, return the project JSON.
    pub async fn create_project(&self, token: &str, name: &str, slug: &str) -> Value {
        let resp = self
            .client
            .post(self.url("/api/v1/projects"))
            .bearer_auth(token)
            .json(&json!({ "name": name, "slug": slug }))
            .send()
            .await
            .expect("create project failed");
        assert_eq!(resp.status(), StatusCode::OK, "create project non-200");
        resp.json().await.unwrap()
    }

    /// Create an endpoint under a project, return the endpoint JSON.
    pub async fn create_endpoint(
        &self,
        token: &str,
        project_id: &str,
        name: &str,
        slug: &str,
        fields: Option<Value>,
        settings: Option<Value>,
    ) -> Value {
        let mut body = json!({ "name": name, "slug": slug });
        if let Some(f) = fields {
            body["fields"] = f;
        }
        if let Some(s) = settings {
            body["settings"] = s;
        }
        let resp = self
            .client
            .post(self.url(&format!("/api/v1/projects/{project_id}/endpoints")))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .expect("create endpoint failed");
        assert_eq!(resp.status(), StatusCode::OK, "create endpoint non-200");
        resp.json().await.unwrap()
    }

    /// Submit data to an endpoint (JSON), return (body, status).
    pub async fn submit_json(&self, endpoint_id: &str, data: &Value) -> (Value, StatusCode) {
        let resp = self
            .client
            .post(self.url(&format!("/v1/e/{endpoint_id}")))
            .json(data)
            .send()
            .await
            .expect("submit json failed");
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(json!(null));
        (body, status)
    }

    /// Submit form-urlencoded data to an endpoint, return (body, status).
    pub async fn submit_form(&self, endpoint_id: &str, data: &[(&str, &str)]) -> (Value, StatusCode) {
        let resp = self
            .client
            .post(self.url(&format!("/v1/e/{endpoint_id}")))
            .form(data)
            .send()
            .await
            .expect("submit form failed");
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(json!(null));
        (body, status)
    }

    /// Make an authenticated GET request.
    pub async fn get_auth(&self, path: &str, token: &str) -> (Value, StatusCode) {
        let resp = self
            .client
            .get(self.url(path))
            .bearer_auth(token)
            .send()
            .await
            .expect("get request failed");
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(json!(null));
        (body, status)
    }

    /// Make an authenticated POST request with JSON body.
    pub async fn post_auth(&self, path: &str, token: &str, body: &Value) -> (Value, StatusCode) {
        let resp = self
            .client
            .post(self.url(path))
            .bearer_auth(token)
            .json(body)
            .send()
            .await
            .expect("post request failed");
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(json!(null));
        (body, status)
    }

    /// Make an authenticated PUT request with JSON body.
    pub async fn put_auth(&self, path: &str, token: &str, body: &Value) -> (Value, StatusCode) {
        let resp = self
            .client
            .put(self.url(path))
            .bearer_auth(token)
            .json(body)
            .send()
            .await
            .expect("put request failed");
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(json!(null));
        (body, status)
    }

    /// Make an authenticated DELETE request.
    pub async fn delete_auth(&self, path: &str, token: &str) -> (Value, StatusCode) {
        let resp = self
            .client
            .delete(self.url(path))
            .bearer_auth(token)
            .send()
            .await
            .expect("delete request failed");
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(json!(null));
        (body, status)
    }
}

/// Spawn a test app with a fresh temporary database.
pub async fn spawn_app() -> TestApp {
    let _ = dotenvy::dotenv();

    let base_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for tests");

    // Create a unique test database
    let db_name = format!("webhooker_test_{}", Uuid::now_v7().to_string().replace('-', ""));

    // Connect to default postgres DB to create test DB
    let admin_url = base_url
        .rsplit_once('/')
        .map(|(base, _)| format!("{base}/postgres"))
        .unwrap_or_else(|| base_url.clone());

    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
        .expect("Failed to connect to postgres for test DB creation");

    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin_pool)
        .await
        .expect("Failed to create test database");

    admin_pool.close().await;

    // Connect to test DB and run migrations
    let test_url = base_url
        .rsplit_once('/')
        .map(|(base, _)| format!("{base}/{db_name}"))
        .unwrap_or_else(|| base_url.clone());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&test_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations on test database");

    let config = Config {
        database_url: test_url,
        jwt_secret: "test-jwt-secret-that-is-long-enough".to_string(),
        encryption_key: "test-encryption-key-32-chars-ok!".to_string(),
        host: "127.0.0.1".parse().unwrap(),
        port: 0, // unused, we bind to random port
        base_url: "http://localhost:0".to_string(),
        registration: RegistrationMode::Closed,
        max_body_size: 1_048_576,
        trusted_proxies: vec![],
        webhook_ssrf_mode: webhooker::config::SsrfMode::Relaxed,
        allowed_webhook_cidrs: vec![],
        worker_count: 1,
        log_level: "warn".to_string(),
        smtp: None,
    };

    let (app, _state) = webhooker::build_app(pool.clone(), config);

    // Bind to random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to random port");
    let addr = listener.local_addr().unwrap();

    // Spawn server in background
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("Server failed");
    });

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    TestApp {
        addr,
        pool,
        client,
        db_name,
    }
}

/// Drop stale test databases (useful after test crashes).
#[allow(dead_code)]
pub async fn cleanup_stale_test_dbs() {
    let base_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for tests");
    let admin_url = base_url
        .rsplit_once('/')
        .map(|(base, _)| format!("{base}/postgres"))
        .unwrap_or_else(|| base_url.clone());

    if let Ok(admin_pool) = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
    {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT datname FROM pg_database WHERE datname LIKE 'webhooker_test_%'",
        )
        .fetch_all(&admin_pool)
        .await
        .unwrap_or_default();

        for db_name in rows {
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{db_name}\" WITH (FORCE)"))
                .execute(&admin_pool)
                .await;
        }
        admin_pool.close().await;
    }
}

/// Drop the test database after tests complete.
pub async fn cleanup(app: TestApp) {
    let db_name = app.db_name.clone();
    app.pool.close().await;

    let base_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for tests");
    let admin_url = base_url
        .rsplit_once('/')
        .map(|(base, _)| format!("{base}/postgres"))
        .unwrap_or_else(|| base_url.clone());

    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
        .expect("Failed to connect for cleanup");

    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{db_name}\" WITH (FORCE)"))
        .execute(&admin_pool)
        .await;

    admin_pool.close().await;
}
