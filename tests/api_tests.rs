mod common;

use reqwest::StatusCode;
use serde_json::json;

// ── Health ──────────────────────────────────────────────────────

#[tokio::test]
async fn health_returns_ok() {
    let app = common::spawn_app().await;

    let resp = app.client.get(app.url("/health")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.text().await.unwrap(), "ok");

    common::cleanup(app).await;
}

// ── Registration & Auth ─────────────────────────────────────────

#[tokio::test]
async fn register_bootstrap_user() {
    let app = common::spawn_app().await;

    let (body, status) = app.register("admin@test.com", "password123", "Admin").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["access_token"].is_string());
    assert!(body["refresh_token"].is_string());

    common::cleanup(app).await;
}

#[tokio::test]
async fn register_rejects_second_user() {
    let app = common::spawn_app().await;
    app.bootstrap().await;

    let (body, status) = app.register("other@test.com", "password123", "Other").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body["error"].as_str().unwrap().contains("disabled"));

    common::cleanup(app).await;
}

#[tokio::test]
async fn register_rejects_short_password() {
    let app = common::spawn_app().await;

    let (_, status) = app.register("admin@test.com", "short", "Admin").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    common::cleanup(app).await;
}

#[tokio::test]
async fn login_valid_credentials() {
    let app = common::spawn_app().await;
    app.bootstrap().await;

    let (body, status) = app.login("admin@test.com", "password123").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["access_token"].is_string());
    assert!(body["refresh_token"].is_string());

    common::cleanup(app).await;
}

#[tokio::test]
async fn login_invalid_credentials() {
    let app = common::spawn_app().await;
    app.bootstrap().await;

    let (_, status) = app.login("admin@test.com", "wrongpassword").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    common::cleanup(app).await;
}

#[tokio::test]
async fn login_nonexistent_user() {
    let app = common::spawn_app().await;
    app.bootstrap().await;

    let (_, status) = app.login("nobody@test.com", "password123").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    common::cleanup(app).await;
}

// ── Token Refresh ───────────────────────────────────────────────

#[tokio::test]
async fn refresh_token_rotation() {
    let app = common::spawn_app().await;
    app.bootstrap().await;
    let (login_body, _) = app.login("admin@test.com", "password123").await;
    let refresh = login_body["refresh_token"].as_str().unwrap();

    // Use refresh token — should succeed and return new tokens
    let resp = app
        .client
        .post(app.url("/api/v1/auth/refresh"))
        .header("cookie", format!("refresh_token={refresh}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    let new_refresh = body["refresh_token"].as_str().unwrap();

    // New refresh token should be different
    assert_ne!(new_refresh, refresh);

    // New refresh token should also work
    let resp2 = app
        .client
        .post(app.url("/api/v1/auth/refresh"))
        .header("cookie", format!("refresh_token={new_refresh}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);

    common::cleanup(app).await;
}

#[tokio::test]
async fn refresh_token_reuse_detection() {
    let app = common::spawn_app().await;
    app.bootstrap().await;
    let (login_body, _) = app.login("admin@test.com", "password123").await;
    let refresh = login_body["refresh_token"].as_str().unwrap();

    // First refresh - should succeed
    let resp1 = app
        .client
        .post(app.url("/api/v1/auth/refresh"))
        .header("cookie", format!("refresh_token={refresh}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), StatusCode::OK);

    // Replay same token - should detect reuse and nuke all sessions
    let resp2 = app
        .client
        .post(app.url("/api/v1/auth/refresh"))
        .header("cookie", format!("refresh_token={refresh}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);
    let body: serde_json::Value = resp2.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("reuse"));

    common::cleanup(app).await;
}

// ── Logout ──────────────────────────────────────────────────────

#[tokio::test]
async fn logout_invalidates_refresh_token() {
    let app = common::spawn_app().await;
    app.bootstrap().await;
    let (login_body, _) = app.login("admin@test.com", "password123").await;
    let refresh = login_body["refresh_token"].as_str().unwrap();

    // Logout
    let resp = app
        .client
        .post(app.url("/api/v1/auth/logout"))
        .header("cookie", format!("refresh_token={refresh}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Try to use old refresh token
    let resp2 = app
        .client
        .post(app.url("/api/v1/auth/refresh"))
        .header("cookie", format!("refresh_token={refresh}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);

    common::cleanup(app).await;
}

// ── Projects CRUD ───────────────────────────────────────────────

#[tokio::test]
async fn projects_crud() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;

    // Create
    let project = app.create_project(&token, "My Project", "my-project").await;
    let project_id = project["id"].as_str().unwrap();
    assert_eq!(project["name"], "My Project");
    assert_eq!(project["slug"], "my-project");

    // List
    let (list, status) = app.get_auth("/api/v1/projects", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Get
    let (got, status) = app
        .get_auth(&format!("/api/v1/projects/{project_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(got["name"], "My Project");

    // Update
    let (updated, status) = app
        .put_auth(
            &format!("/api/v1/projects/{project_id}"),
            &token,
            &json!({ "name": "Updated", "slug": "updated" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], "Updated");

    // Delete
    let (_, status) = app
        .delete_auth(&format!("/api/v1/projects/{project_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK);

    // Verify deleted
    let (_, status) = app
        .get_auth(&format!("/api/v1/projects/{project_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    common::cleanup(app).await;
}

#[tokio::test]
async fn project_slug_validation() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;

    // Invalid slug with uppercase
    let (_, status) = app
        .post_auth(
            "/api/v1/projects",
            &token,
            &json!({ "name": "Bad", "slug": "Bad-Slug" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Empty slug
    let (_, status) = app
        .post_auth(
            "/api/v1/projects",
            &token,
            &json!({ "name": "Bad", "slug": "" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    common::cleanup(app).await;
}

#[tokio::test]
async fn project_duplicate_slug_conflict() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;

    app.create_project(&token, "First", "same-slug").await;
    let (_, status) = app
        .post_auth(
            "/api/v1/projects",
            &token,
            &json!({ "name": "Second", "slug": "same-slug" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    common::cleanup(app).await;
}

// ── Endpoints CRUD ──────────────────────────────────────────────

#[tokio::test]
async fn endpoints_crud() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let project_id = project["id"].as_str().unwrap();

    let fields = json!([
        { "name": "email", "type": "email", "required": true },
        { "name": "message", "type": "text" }
    ]);

    let settings = json!({
        "rate_limit": 5,
        "honeypot_field": "website"
    });

    // Create
    let endpoint = app
        .create_endpoint(
            &token,
            project_id,
            "Contact Form",
            "contact",
            Some(fields.clone()),
            Some(settings.clone()),
        )
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();
    assert_eq!(endpoint["name"], "Contact Form");

    // List by project
    let (list, status) = app
        .get_auth(
            &format!("/api/v1/projects/{project_id}/endpoints"),
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Get
    let (got, status) = app
        .get_auth(&format!("/api/v1/endpoints/{endpoint_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(got["name"], "Contact Form");

    // Update
    let (updated, status) = app
        .put_auth(
            &format!("/api/v1/endpoints/{endpoint_id}"),
            &token,
            &json!({
                "name": "Updated Form",
                "slug": "updated-form",
                "fields": fields,
                "settings": settings
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], "Updated Form");

    // Delete
    let (_, status) = app
        .delete_auth(&format!("/api/v1/endpoints/{endpoint_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK);

    common::cleanup(app).await;
}

// ── Submission Ingestion ────────────────────────────────────────

#[tokio::test]
async fn submit_json_data() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let endpoint = app
        .create_endpoint(&token, project["id"].as_str().unwrap(), "Form", "form", None, None)
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    let (body, status) = app
        .submit_json(endpoint_id, &json!({ "name": "Alice", "email": "alice@test.com" }))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["status"], "created");
    assert!(body["submission_id"].is_string());

    common::cleanup(app).await;
}

#[tokio::test]
async fn submit_form_urlencoded() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let endpoint = app
        .create_endpoint(&token, project["id"].as_str().unwrap(), "Form", "form", None, None)
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    let (body, status) = app
        .submit_form(endpoint_id, &[("name", "Bob"), ("email", "bob@test.com")])
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["status"], "created");

    common::cleanup(app).await;
}

#[tokio::test]
async fn submit_to_nonexistent_endpoint() {
    let app = common::spawn_app().await;

    let fake_id = uuid::Uuid::now_v7();
    let (_, status) = app.submit_json(&fake_id.to_string(), &json!({"x": 1})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    common::cleanup(app).await;
}

#[tokio::test]
async fn honeypot_silently_accepts_spam() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let endpoint = app
        .create_endpoint(
            &token,
            project["id"].as_str().unwrap(),
            "Form",
            "form",
            None,
            Some(json!({ "honeypot_field": "website" })),
        )
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    // Submit with honeypot field filled → spam
    let (body, status) = app
        .submit_json(
            endpoint_id,
            &json!({ "name": "Spammer", "website": "http://spam.com" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    // Submit without honeypot field → legit
    let (body, status) = app
        .submit_json(endpoint_id, &json!({ "name": "Legit" }))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["status"], "created");

    common::cleanup(app).await;
}

#[tokio::test]
async fn field_sorting_data_and_extras() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;

    let fields = json!([
        { "name": "name", "type": "text" },
        { "name": "email", "type": "email" }
    ]);
    let endpoint = app
        .create_endpoint(
            &token,
            project["id"].as_str().unwrap(),
            "Form",
            "form",
            Some(fields),
            None,
        )
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    // Submit with known + unknown fields
    let (body, status) = app
        .submit_json(
            endpoint_id,
            &json!({ "name": "Alice", "email": "a@b.com", "extra_field": "surprise" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let sub_id = body["submission_id"].as_str().unwrap();

    // Fetch submission and verify data/extras split
    let (sub, status) = app
        .get_auth(&format!("/api/v1/submissions/{sub_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sub["data"]["name"], "Alice");
    assert_eq!(sub["data"]["email"], "a@b.com");
    assert_eq!(sub["extras"]["extra_field"], "surprise");

    common::cleanup(app).await;
}

// ── Submissions API ─────────────────────────────────────────────

#[tokio::test]
async fn list_submissions_paginated() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let endpoint = app
        .create_endpoint(&token, project["id"].as_str().unwrap(), "Form", "form", None, None)
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    // Submit 3 entries
    for i in 0..3 {
        app.submit_json(endpoint_id, &json!({ "index": i })).await;
    }

    // List page 1, per_page 2
    let (body, status) = app
        .get_auth(
            &format!("/api/v1/endpoints/{endpoint_id}/submissions?page=1&per_page=2"),
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["submissions"].as_array().unwrap().len(), 2);
    assert_eq!(body["total"], 3);
    assert_eq!(body["total_pages"], 2);

    // List page 2
    let (body, status) = app
        .get_auth(
            &format!("/api/v1/endpoints/{endpoint_id}/submissions?page=2&per_page=2"),
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["submissions"].as_array().unwrap().len(), 1);

    common::cleanup(app).await;
}

#[tokio::test]
async fn delete_submission() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let endpoint = app
        .create_endpoint(&token, project["id"].as_str().unwrap(), "Form", "form", None, None)
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    let (body, _) = app
        .submit_json(endpoint_id, &json!({ "name": "delete-me" }))
        .await;
    let sub_id = body["submission_id"].as_str().unwrap();

    let (_, status) = app
        .delete_auth(&format!("/api/v1/submissions/{sub_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK);

    let (_, status) = app
        .get_auth(&format!("/api/v1/submissions/{sub_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    common::cleanup(app).await;
}

#[tokio::test]
async fn bulk_delete_submissions() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let endpoint = app
        .create_endpoint(&token, project["id"].as_str().unwrap(), "Form", "form", None, None)
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    for i in 0..5 {
        app.submit_json(endpoint_id, &json!({ "i": i })).await;
    }

    let (body, status) = app
        .delete_auth(
            &format!("/api/v1/endpoints/{endpoint_id}/submissions"),
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["deleted"], 5);

    common::cleanup(app).await;
}

#[tokio::test]
async fn export_submissions_csv() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let endpoint = app
        .create_endpoint(&token, project["id"].as_str().unwrap(), "Form", "form", None, None)
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    app.submit_json(endpoint_id, &json!({ "name": "Alice", "email": "a@b.com" }))
        .await;

    let resp = app
        .client
        .get(app.url(&format!(
            "/api/v1/endpoints/{endpoint_id}/submissions/export?format=csv"
        )))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let csv = resp.text().await.unwrap();
    assert!(csv.contains("name"));
    assert!(csv.contains("Alice"));

    common::cleanup(app).await;
}

// ── Rate Limiting ───────────────────────────────────────────────

#[tokio::test]
async fn submission_rate_limiting() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let endpoint = app
        .create_endpoint(
            &token,
            project["id"].as_str().unwrap(),
            "Form",
            "form",
            None,
            Some(json!({ "rate_limit": 3, "rate_limit_window_secs": 60 })),
        )
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    // 3 should succeed
    for _ in 0..3 {
        let (_, status) = app.submit_json(endpoint_id, &json!({ "x": 1 })).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    // 4th should be rate limited
    let (_, status) = app.submit_json(endpoint_id, &json!({ "x": 1 })).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

    common::cleanup(app).await;
}

#[tokio::test]
async fn login_brute_force_protection() {
    let app = common::spawn_app().await;
    app.bootstrap().await;

    // 5 bad logins should pass (incrementing counter)
    for _ in 0..5 {
        let (_, status) = app.login("admin@test.com", "wrong").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // 6th should be rate limited
    let (_, status) = app.login("admin@test.com", "wrong").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

    common::cleanup(app).await;
}

// ── Tenant Isolation ────────────────────────────────────────────

#[tokio::test]
async fn tenant_isolation() {
    let app = common::spawn_app().await;
    let admin_token = app.bootstrap().await;

    // Admin creates a project
    let project = app
        .create_project(&admin_token, "Admin Project", "admin-project")
        .await;
    let project_id = project["id"].as_str().unwrap();

    // Admin creates a second tenant + user via admin API
    let (tenant2, status) = app
        .post_auth(
            "/api/v1/admin/tenants",
            &admin_token,
            &json!({ "name": "Tenant 2", "slug": "tenant-2" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let tenant2_id = tenant2["id"].as_str().unwrap();

    let (_, status) = app
        .post_auth(
            "/api/v1/admin/users",
            &admin_token,
            &json!({
                "email": "user2@test.com",
                "password": "password123",
                "name": "User 2",
                "tenant_id": tenant2_id,
                "role": "owner"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Login as user2
    let (login_body, _) = app.login("user2@test.com", "password123").await;
    let user2_token = login_body["access_token"].as_str().unwrap();

    // User2 should NOT see admin's project
    let (projects, status) = app.get_auth("/api/v1/projects", user2_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(projects.as_array().unwrap().len(), 0);

    // User2 should NOT be able to access admin's project directly
    let (_, status) = app
        .get_auth(&format!("/api/v1/projects/{project_id}"), user2_token)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    common::cleanup(app).await;
}

// ── Admin API ───────────────────────────────────────────────────

#[tokio::test]
async fn admin_tenant_crud() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;

    // Create tenant
    let (tenant, status) = app
        .post_auth(
            "/api/v1/admin/tenants",
            &token,
            &json!({ "name": "New Tenant", "slug": "new-tenant" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let tenant_id = tenant["id"].as_str().unwrap();

    // List tenants
    let (list, status) = app.get_auth("/api/v1/admin/tenants", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().unwrap().len() >= 2); // bootstrap tenant + new one

    // Delete tenant
    let (_, status) = app
        .delete_auth(&format!("/api/v1/admin/tenants/{tenant_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK);

    common::cleanup(app).await;
}

#[tokio::test]
async fn non_admin_cannot_access_admin_routes() {
    let app = common::spawn_app().await;
    let admin_token = app.bootstrap().await;

    // Create a second tenant with a non-admin user
    let (tenant, _) = app
        .post_auth(
            "/api/v1/admin/tenants",
            &admin_token,
            &json!({ "name": "T2", "slug": "t2" }),
        )
        .await;
    let (_, _) = app
        .post_auth(
            "/api/v1/admin/users",
            &admin_token,
            &json!({
                "email": "regular@test.com",
                "password": "password123",
                "name": "Regular",
                "tenant_id": tenant["id"],
                "role": "member"
            }),
        )
        .await;

    let (login_body, _) = app.login("regular@test.com", "password123").await;
    let user_token = login_body["access_token"].as_str().unwrap();

    // Non-admin should be forbidden from admin routes
    let (_, status) = app.get_auth("/api/v1/admin/tenants", user_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    common::cleanup(app).await;
}

// ── Modules ─────────────────────────────────────────────────────

#[tokio::test]
async fn list_available_modules() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;

    let (body, status) = app.get_auth("/api/v1/modules", &token).await;
    assert_eq!(status, StatusCode::OK);
    let modules = body["modules"].as_array().unwrap();
    let ids: Vec<&str> = modules
        .iter()
        .filter_map(|m| m["id"].as_str())
        .collect();
    assert!(ids.contains(&"email"));
    assert!(ids.contains(&"webhook"));

    common::cleanup(app).await;
}

// ── Actions CRUD ────────────────────────────────────────────────

#[tokio::test]
async fn actions_crud() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let endpoint = app
        .create_endpoint(&token, project["id"].as_str().unwrap(), "Form", "form", None, None)
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    // Create a webhook action
    let (action, status) = app
        .post_auth(
            &format!("/api/v1/endpoints/{endpoint_id}/actions"),
            &token,
            &json!({
                "action_type": "webhook",
                "config": {
                    "url": "https://httpbin.org/post",
                    "method": "POST"
                },
                "position": 0
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let action_id = action["id"].as_str().unwrap();
    assert_eq!(action["action_type"], "webhook");

    // List actions for endpoint
    let (list, status) = app
        .get_auth(
            &format!("/api/v1/endpoints/{endpoint_id}/actions"),
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Update action (disable it)
    let (updated, status) = app
        .put_auth(
            &format!("/api/v1/actions/{action_id}"),
            &token,
            &json!({
                "action_type": "webhook",
                "config": { "url": "https://httpbin.org/post" },
                "enabled": false,
                "position": 1
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["enabled"], false);
    assert_eq!(updated["position"], 1);

    // Delete action
    let (_, status) = app
        .delete_auth(&format!("/api/v1/actions/{action_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK);

    common::cleanup(app).await;
}

// ── Unauthenticated Access ──────────────────────────────────────

#[tokio::test]
async fn unauthenticated_requests_rejected() {
    let app = common::spawn_app().await;
    app.bootstrap().await;

    let (_, status) = app.get_auth("/api/v1/projects", "invalid-token").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (_, status) = app.get_auth("/api/v1/projects", "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    common::cleanup(app).await;
}

// ── Security Headers ────────────────────────────────────────────

#[tokio::test]
async fn security_headers_present() {
    let app = common::spawn_app().await;

    let resp = app.client.get(app.url("/health")).send().await.unwrap();
    assert_eq!(
        resp.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
    assert_eq!(
        resp.headers().get("referrer-policy").unwrap(),
        "strict-origin-when-cross-origin"
    );

    common::cleanup(app).await;
}

// ── CORS Preflight ──────────────────────────────────────────────

#[tokio::test]
async fn cors_preflight_options() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;
    let project = app.create_project(&token, "Project", "project").await;
    let endpoint = app
        .create_endpoint(&token, project["id"].as_str().unwrap(), "Form", "form", None, None)
        .await;
    let endpoint_id = endpoint["id"].as_str().unwrap();

    let resp = app
        .client
        .request(
            reqwest::Method::OPTIONS,
            app.url(&format!("/v1/e/{endpoint_id}")),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(resp.headers().contains_key("access-control-allow-origin"));
    assert!(resp.headers().contains_key("access-control-allow-methods"));

    common::cleanup(app).await;
}

// ── Password Reset ──────────────────────────────────────────────

#[tokio::test]
async fn forgot_password_always_200() {
    let app = common::spawn_app().await;
    app.bootstrap().await;

    // Existing email
    let resp = app
        .client
        .post(app.url("/api/v1/auth/forgot-password"))
        .json(&json!({ "email": "admin@test.com" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Non-existing email (should still return 200)
    let resp = app
        .client
        .post(app.url("/api/v1/auth/forgot-password"))
        .json(&json!({ "email": "nobody@test.com" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    common::cleanup(app).await;
}

// ── Tenant Members ──────────────────────────────────────────────

#[tokio::test]
async fn tenant_member_management() {
    let app = common::spawn_app().await;
    let token = app.bootstrap().await;

    // Add member
    let (member, status) = app
        .post_auth(
            "/api/v1/tenant/members",
            &token,
            &json!({
                "email": "member@test.com",
                "password": "password123",
                "name": "Member",
                "role": "member"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let member_id = member["id"].as_str().unwrap();

    // List members
    let (list, status) = app.get_auth("/api/v1/tenant/members", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().unwrap().len() >= 2); // owner + member

    // Update member role
    let (_, status) = app
        .put_auth(
            &format!("/api/v1/tenant/members/{member_id}"),
            &token,
            &json!({ "role": "admin" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Remove member
    let (_, status) = app
        .delete_auth(&format!("/api/v1/tenant/members/{member_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK);

    // Verify removed member can't login
    let (_, status) = app.login("member@test.com", "password123").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    common::cleanup(app).await;
}
