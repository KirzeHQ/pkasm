use std::sync::Arc;

use askama::Template;
use axum::{
    Router,
    extract::{Form, Multipart, Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
};
use package_manager::{
    ApiKey, ApiKeySummary, InMemoryRegistry, Package, PackageId, PackageManager,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

type Registry = PackageManager<InMemoryRegistry>;

#[derive(Clone)]
struct AppState {
    registry: Arc<RwLock<Registry>>,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[derive(Deserialize)]
struct AccountForm {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct ApiKeyForm {
    name: String,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    title: String,
    query: String,
    packages: Vec<Package>,
    logged_in: bool,
}

#[derive(Template)]
#[template(path = "package.html")]
struct PackageTemplate {
    title: String,
    package: Package,
    logged_in: bool,
}

#[derive(Template)]
#[template(path = "account.html")]
struct AccountTemplate {
    title: String,
    action: String,
    action_label: String,
    message: String,
    logged_in: bool,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    title: String,
    logged_in: bool,
    username: String,
    api_keys: Vec<ApiKeySummary>,
    message: String,
    new_key: Option<ApiKey>,
}

#[derive(Serialize)]
struct PublishResponse {
    name: String,
    version: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState {
        registry: Arc::new(RwLock::new(
            PackageManager::new(InMemoryRegistry::default()),
        )),
    };
    let app = Router::new()
        .route("/", get(index))
        .route("/style.css", get(style))
        .route("/search", get(search))
        .route("/register", get(register_form).post(register))
        .route("/login", get(login_form).post(login))
        .route("/logout", get(logout))
        .route("/dashboard", get(dashboard))
        .route("/dashboard/api-keys", post(create_api_key))
        .route("/dashboard/api-keys/{id}/delete", post(delete_api_key))
        .route("/packages/{name}/{version}", get(package))
        .route("/packages/{name}/{version}/download", get(download))
        .route("/api/v1/packages", post(api_publish))
        .route(
            "/api/v1/packages/{name}/{version}/download",
            get(api_download),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!(
        "PkASM package registry listening on http://{}/",
        listener.local_addr()?
    );
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index(State(state): State<AppState>, headers: axum::http::HeaderMap) -> Response {
    render_index(&state, String::new(), &headers).await
}

async fn style() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../static/style.css"),
    )
}

async fn search(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<SearchQuery>,
) -> Response {
    render_index(&state, query.q.unwrap_or_default(), &headers).await
}

async fn render_index(
    state: &AppState,
    query: String,
    headers: &axum::http::HeaderMap,
) -> Response {
    let registry = state.registry.read().await;
    let packages = registry.search(&query);
    let logged_in = session_from_headers(headers)
        .is_some_and(|session| registry.account_for_session(&session).is_some());
    render(IndexTemplate {
        title: "Packages".to_owned(),
        query,
        packages,
        logged_in,
    })
}

async fn register_form() -> Response {
    render(AccountTemplate {
        title: "Register".to_owned(),
        action: "register".to_owned(),
        action_label: "Register".to_owned(),
        message: String::new(),
        logged_in: false,
    })
}

async fn register(State(state): State<AppState>, Form(form): Form<AccountForm>) -> Response {
    let result = state
        .registry
        .write()
        .await
        .register_session(form.username, form.password);
    let session = match result {
        Ok(session) => session,
        Err(error) => {
            return render(AccountTemplate {
                title: "Register".to_owned(),
                action: "register".to_owned(),
                action_label: "Register".to_owned(),
                message: error.to_string(),
                logged_in: false,
            });
        }
    };
    redirect_with_session(&session.token)
}

async fn login_form() -> Response {
    render(AccountTemplate {
        title: "Log in".to_owned(),
        action: "login".to_owned(),
        action_label: "Log in".to_owned(),
        message: String::new(),
        logged_in: false,
    })
}

async fn login(State(state): State<AppState>, Form(form): Form<AccountForm>) -> Response {
    let result = state
        .registry
        .write()
        .await
        .login(&form.username, &form.password);
    let (message, token) = match result {
        Ok(session) => (
            format!("Logged in as {}.", session.account.username),
            Some(session.token),
        ),
        Err(error) => (error.to_string(), None),
    };
    let response = render(AccountTemplate {
        title: "Log in".to_owned(),
        action: "login".to_owned(),
        action_label: "Log in".to_owned(),
        message,
        logged_in: token.is_some(),
    });
    match token {
        Some(token) => with_session_cookie(response, &token),
        None => response,
    }
}

async fn logout(State(state): State<AppState>, headers: axum::http::HeaderMap) -> Response {
    if let Some(token) = session_from_headers(&headers) {
        state.registry.write().await.logout(&token);
    }
    let mut response = render(AccountTemplate {
        title: "Log in".to_owned(),
        action: "login".to_owned(),
        action_label: "Log in".to_owned(),
        message: "Logged out.".to_owned(),
        logged_in: false,
    });
    if let Ok(value) = HeaderValue::from_str("pkasm_session=; HttpOnly; Path=/; Max-Age=0") {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

async fn dashboard(State(state): State<AppState>, headers: axum::http::HeaderMap) -> Response {
    let Some(session) = session_from_headers(&headers) else {
        return (StatusCode::UNAUTHORIZED, "Log in to view your dashboard.").into_response();
    };
    let registry = state.registry.read().await;
    match (
        registry.account_for_session(&session),
        registry.api_keys(&session),
    ) {
        (Some(account), Ok(api_keys)) => render(DashboardTemplate {
            title: "Dashboard".to_owned(),
            logged_in: true,
            username: account.username,
            api_keys,
            message: String::new(),
            new_key: None,
        }),
        _ => (StatusCode::UNAUTHORIZED, "Your session is no longer valid.").into_response(),
    }
}

async fn create_api_key(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Form(form): Form<ApiKeyForm>,
) -> Response {
    let Some(session) = session_from_headers(&headers) else {
        return (StatusCode::UNAUTHORIZED, "Log in to create an API key.").into_response();
    };
    let mut registry = state.registry.write().await;
    match registry.create_api_key(&session, form.name) {
        Ok(api_key) => render_dashboard(
            &registry,
            &session,
            "Save this key now; it will not be shown again.",
            Some(api_key),
        ),
        Err(error) => render_dashboard(&registry, &session, &error.to_string(), None),
    }
}

async fn delete_api_key(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let Some(session) = session_from_headers(&headers) else {
        return (StatusCode::UNAUTHORIZED, "Log in to manage API keys.").into_response();
    };
    let mut registry = state.registry.write().await;
    let message = match registry.revoke_api_key(&session, &id) {
        Ok(true) => "API key deleted.".to_owned(),
        Ok(false) => "API key not found.".to_owned(),
        Err(error) => error.to_string(),
    };
    render_dashboard(&registry, &session, &message, None)
}

fn render_dashboard(
    registry: &Registry,
    session: &str,
    message: &str,
    new_key: Option<ApiKey>,
) -> Response {
    match (
        registry.account_for_session(session),
        registry.api_keys(session),
    ) {
        (Some(account), Ok(api_keys)) => render(DashboardTemplate {
            title: "Dashboard".to_owned(),
            logged_in: true,
            username: account.username,
            api_keys,
            message: message.to_owned(),
            new_key,
        }),
        _ => (StatusCode::UNAUTHORIZED, "Your session is no longer valid.").into_response(),
    }
}

async fn package(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path((name, version)): Path<(String, String)>,
) -> Response {
    let id = PackageId::new(name, version);
    let package = state.registry.read().await.get(&id);
    let logged_in = match session_from_headers(&headers) {
        Some(session) => state
            .registry
            .read()
            .await
            .account_for_session(&session)
            .is_some(),
        None => false,
    };

    match package {
        Some(package) => render(PackageTemplate {
            title: format!("{} {}", package.id.name, package.id.version),
            package,
            logged_in,
        }),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn download(
    State(state): State<AppState>,
    Path((name, version)): Path<(String, String)>,
) -> Response {
    let id = PackageId::new(name, version);
    match state.registry.read().await.download(&id) {
        Some(download) => (
            [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
            download.archive,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn api_publish(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let Some(api_key) = bearer_token(&headers) else {
        return (StatusCode::UNAUTHORIZED, "Bearer API key required").into_response();
    };

    let mut name = None;
    let mut version = None;
    let mut archive = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
        };
        let field_name = field.name().unwrap_or_default().to_owned();
        match field_name.as_str() {
            "name" => name = field.text().await.ok(),
            "version" => version = field.text().await.ok(),
            "archive" => archive = field.bytes().await.ok(),
            _ => {}
        }
    }

    let (Some(name), Some(version), Some(archive)) = (name, version, archive) else {
        return (
            StatusCode::BAD_REQUEST,
            "multipart fields name, version, and archive are required",
        )
            .into_response();
    };
    let result = state.registry.write().await.publish_basic_with_api_key(
        &api_key,
        name.clone(),
        version.clone(),
        archive.to_vec(),
    );

    match result {
        Ok(()) => Json(PublishResponse { name, version }).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    }
}

async fn api_download(
    State(state): State<AppState>,
    Path((name, version)): Path<(String, String)>,
) -> Response {
    download(State(state), Path((name, version))).await
}

fn render<T: Template>(template: T) -> Response {
    match template.render() {
        Ok(body) => Html(body).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("template rendering failed: {error}"),
        )
            .into_response(),
    }
}

fn session_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|cookie| cookie.strip_prefix("pkasm_session="))
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
}

fn with_session_cookie(mut response: Response, token: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(&format!(
        "pkasm_session={token}; HttpOnly; Path=/; SameSite=Lax"
    )) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

fn redirect_with_session(token: &str) -> Response {
    let response = (StatusCode::SEE_OTHER, [(header::LOCATION, "/dashboard")]).into_response();
    with_session_cookie(response, token)
}

fn bearer_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
}
