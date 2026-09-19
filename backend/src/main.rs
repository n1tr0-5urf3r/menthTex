mod bibtex;
mod workspace;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bibtex::{
    DefinitionCollection, FormatOptions, FormattedEntry, ProposedDefinition, RawEntry,
    VenueSuggestion, definition_from_expanded_value, definition_from_parts,
    format_definition_source, format_entries, format_string_definition, parse_bibtex_document,
};
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use rand::RngExt;
use rand::distr::Alphanumeric;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) conferences_path: PathBuf,
    pub(crate) file_lock: Arc<Mutex<()>>,
    pub(crate) http_client: Client,
    pub(crate) auth_config: Option<AuthConfig>,
    pub(crate) db_pool: sqlx::PgPool,
    pub(crate) storage_root: PathBuf,
    pub(crate) external_metadata_lookup_enabled: bool,
    pub(crate) external_metadata_lookup_timeout: Duration,
    pending_oauth: Arc<Mutex<HashMap<String, PendingOAuthState>>>,
}

#[derive(Clone)]
pub(crate) struct AuthConfig {
    gitlab: Option<GitLabAuthConfig>,
    authentik: Option<AuthentikAuthConfig>,
    allowed_groups: Vec<String>,
    session_ttl: Duration,
    cookie_secure: bool,
}

#[derive(Clone)]
struct GitLabAuthConfig {
    gitlab_base_url: String,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

#[derive(Clone)]
struct AuthentikAuthConfig {
    issuer_url: String,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    groups_claim: String,
    scopes: Vec<String>,
}

#[derive(Clone)]
struct PendingOAuthState {
    provider: String,
    return_to: String,
    expires_at: u64,
    oidc_nonce: Option<String>,
    pkce_verifier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FormatRequest {
    input: String,
    include_string_definitions: bool,
    use_conferences_bib: bool,
    #[serde(default = "default_true")]
    rewrite_reference_key: bool,
    #[serde(default)]
    keep_doi: bool,
}

#[derive(Debug, Deserialize)]
struct DoiLookupRequest {
    doi: String,
    include_string_definitions: bool,
    use_conferences_bib: bool,
    #[serde(default = "default_true")]
    rewrite_reference_key: bool,
    #[serde(default)]
    keep_doi: bool,
}

#[derive(Debug, Deserialize)]
struct RfcLookupRequest {
    rfc: String,
    include_string_definitions: bool,
    use_conferences_bib: bool,
    #[serde(default = "default_true")]
    rewrite_reference_key: bool,
    #[serde(default)]
    keep_doi: bool,
}

#[derive(Debug, Deserialize)]
struct AddDefinitionRequest {
    token: String,
    raw_value: String,
}

#[derive(Debug, Deserialize)]
struct DefinitionMutationRequest {
    token: String,
    expanded_value: String,
}

#[derive(Debug, Deserialize)]
struct AuthLoginQuery {
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitLabTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GitLabUserResponse {
    id: u64,
    email: String,
    username: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AuthProviderOption {
    id: &'static str,
    label: &'static str,
}

#[derive(Debug, Serialize)]
struct ProposedDefinitionResponse {
    token: String,
    raw_value: String,
    expanded_value: String,
    definition_text: String,
    suggestions: Vec<VenueSuggestionResponse>,
}

#[derive(Debug, Serialize)]
struct VenueSuggestionResponse {
    token: String,
    raw_value: String,
    expanded_value: String,
    definition_text: String,
    score: u16,
}

#[derive(Debug, Serialize)]
pub(crate) struct FormatResponse {
    output: String,
    warnings: Vec<String>,
    added_abbreviations: Vec<String>,
    proposed_definitions: Vec<ProposedDefinitionResponse>,
    output_definitions: Vec<ProposedDefinitionResponse>,
    entry_count: usize,
    formatted_count: usize,
    untouched_count: usize,
    entries: Vec<FormattedEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retrieved_input: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized_doi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized_rfc: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PdfMetadataCandidate {
    pub(crate) title: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) year: Option<String>,
    pub(crate) doi: Option<String>,
    pub(crate) venue: Option<String>,
    pub(crate) venue_kind: Option<String>,
    pub(crate) abstract_text: Option<String>,
    pub(crate) extracted_text: String,
}

#[derive(Debug, Clone, Default)]
struct FrontMatterMetadata {
    title: Option<String>,
    author: Option<String>,
    year: Option<String>,
    doi: Option<String>,
    venue: Option<String>,
    venue_kind: Option<String>,
    abstract_text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RfcEditorMetadata {
    title: Option<String>,
    author: Option<String>,
    year: Option<String>,
    month: Option<String>,
    pagetotal: Option<String>,
}

#[derive(Debug)]
struct PdfUploadRequest {
    bytes: Vec<u8>,
    filename: Option<String>,
    include_string_definitions: bool,
    use_conferences_bib: bool,
    rewrite_reference_key: bool,
    keep_doi: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ErrorResponse {
    error: String,
}

/// Turns an unexpected panic in a handler into a JSON error response. Without
/// this the connection is closed mid-response, which a reverse proxy reports
/// as a 502 and the frontend can only surface as a JSON parse failure.
fn handle_panic(panic: Box<dyn std::any::Any + Send + 'static>) -> Response {
    let details = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic");
    eprintln!("Request handler panicked: {details}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "The request failed because of an internal error. Please report the input that triggered it.".to_string(),
        }),
    )
        .into_response()
}

#[derive(Debug, Serialize)]
struct AddDefinitionResponse {
    added_abbreviation: String,
    definition_text: String,
}

#[derive(Debug, Serialize)]
struct DefinitionRecordResponse {
    token: String,
    raw_value: String,
    expanded_value: String,
    definition_text: String,
}

#[derive(Debug, Serialize)]
struct DefinitionListResponse {
    definitions: Vec<DefinitionRecordResponse>,
}

#[derive(Debug, Serialize)]
struct DefinitionMutationResponse {
    definition: DefinitionRecordResponse,
}

#[derive(Debug, Serialize)]
struct AuthStatusResponse {
    enabled: bool,
    authenticated: bool,
    can_manage_abbreviations: bool,
    providers: Vec<AuthProviderOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    app_version: &'static str,
    backend_version: &'static str,
}

#[tokio::main]
async fn main() {
    let default_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend directory should have a parent")
        .to_path_buf();
    let conferences_path = env::var("CONFERENCES_BIB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_root.join("conferences.bib"));
    let frontend_dist = env::var("FRONTEND_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_root.join("frontend").join("dist"));
    let http_client = Client::builder()
        .user_agent("menthbibtexformatter/1.0 (+https://github.com/)")
        .build()
        .expect("failed to construct HTTP client");
    let auth_config = build_auth_config();
    let database_url = non_empty_env("DATABASE_URL").expect("DATABASE_URL must be configured");
    let storage_root = env::var("STORAGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_root.join("storage"));
    let external_metadata_lookup_enabled = env::var("EXTERNAL_METADATA_LOOKUP_ENABLED")
        .ok()
        .map(|value| parse_boolean_env(&value))
        .unwrap_or(true);
    let external_metadata_lookup_timeout = env::var("EXTERNAL_METADATA_LOOKUP_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(6));
    let db_pool = workspace::connect_database(&database_url)
        .await
        .expect("failed to connect to PostgreSQL");
    workspace::migrate_database(&db_pool)
        .await
        .expect("failed to apply PostgreSQL migrations");
    workspace::ensure_storage_root(&storage_root)
        .await
        .expect("failed to initialize storage root");

    let app_state = AppState {
        conferences_path,
        file_lock: Arc::new(Mutex::new(())),
        http_client,
        auth_config,
        db_pool,
        storage_root,
        external_metadata_lookup_enabled,
        external_metadata_lookup_timeout,
        pending_oauth: Arc::new(Mutex::new(HashMap::new())),
    };

    let mut app = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/me", get(auth_me))
        .route("/api/auth/login", get(auth_login))
        .route("/api/auth/login/{provider}", get(auth_login_provider))
        .route("/api/auth/callback", get(auth_callback))
        .route("/api/auth/callback/{provider}", get(auth_callback_provider))
        .route("/api/auth/logout", post(auth_logout))
        .route("/api/format-entry", post(format_entry))
        .route("/api/lookup-doi", post(lookup_doi))
        .route("/api/lookup-pdf", post(lookup_pdf))
        .route("/api/lookup-rfc", post(lookup_rfc))
        .route("/api/definition-options", get(list_definition_options))
        .route(
            "/api/definitions",
            get(list_definitions).post(create_definition_entry),
        )
        .route(
            "/api/definitions/{token}",
            put(update_definition_entry).delete(delete_definition_entry),
        )
        .route("/api/add-definition", post(add_definition))
        .merge(workspace::router())
        .with_state(app_state)
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .layer(CatchPanicLayer::custom(handle_panic));

    if frontend_dist.exists() {
        let frontend_index = frontend_dist.join("index.html");
        app = app.fallback_service(
            ServeDir::new(&frontend_dist).not_found_service(ServeFile::new(frontend_index)),
        );
    } else {
        eprintln!(
            "Frontend dist directory '{}' not found. Serving API routes only.",
            frontend_dist.display()
        );
    }

    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("failed to bind backend listener");

    println!("Backend listening on http://{address}");
    axum::serve(listener, app)
        .await
        .expect("backend server crashed");
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        app_version: env!("CARGO_PKG_VERSION"),
        backend_version: env!("CARGO_PKG_VERSION"),
    })
}

async fn auth_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthStatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    let providers = auth_provider_options(&state);
    if providers.is_empty() {
        return Ok(Json(AuthStatusResponse {
            enabled: false,
            authenticated: false,
            can_manage_abbreviations: false,
            providers,
            email: None,
        }));
    }

    let session = session_from_headers(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(AuthStatusResponse {
        enabled: true,
        authenticated: session.is_some(),
        can_manage_abbreviations: session.is_some(),
        providers,
        email: session.map(|session| session.email),
    }))
}

async fn auth_login(
    State(state): State<AppState>,
    Query(query): Query<AuthLoginQuery>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    begin_auth_login(state, "gitlab", query).await
}

async fn auth_login_provider(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(query): Query<AuthLoginQuery>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    begin_auth_login(state, &provider, query).await
}

async fn begin_auth_login(
    state: AppState,
    provider: &str,
    query: AuthLoginQuery,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    match provider {
        "gitlab" => begin_gitlab_login(state, query).await,
        "authentik" => begin_authentik_login(state, query).await,
        _ => Err(ApiError::bad_request("Unknown authentication provider.").into_response()),
    }
}

async fn begin_gitlab_login(
    state: AppState,
    query: AuthLoginQuery,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let config = state
        .auth_config
        .as_ref()
        .and_then(|config| config.gitlab.as_ref())
        .ok_or_else(|| ApiError::service_unavailable("GitLab sign-in is not configured."))
        .map_err(|error| error.into_response())?;

    let state_token = random_token(32);
    let now = current_timestamp_secs();
    let return_to = sanitize_return_to(query.return_to.as_deref());

    {
        let mut pending = state.pending_oauth.lock().map_err(|_| {
            ApiError::internal("Failed to acquire OAuth state lock.").into_response()
        })?;
        pending.retain(|_, value| value.expires_at > now);
        pending.insert(
            state_token.clone(),
            PendingOAuthState {
                provider: "gitlab".to_string(),
                return_to,
                expires_at: now + 600,
                oidc_nonce: None,
                pkce_verifier: None,
            },
        );
    }

    let mut authorize_url = reqwest::Url::parse(&format!(
        "{}/oauth/authorize",
        config.gitlab_base_url.trim_end_matches('/')
    ))
    .map_err(|error| {
        ApiError::internal(format!("Failed to construct GitLab authorize URL: {error}"))
            .into_response()
    })?;
    authorize_url
        .query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", "read_api")
        .append_pair("state", &state_token);

    Ok(Redirect::temporary(authorize_url.as_ref()).into_response())
}

async fn begin_authentik_login(
    state: AppState,
    query: AuthLoginQuery,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let config = state
        .auth_config
        .as_ref()
        .and_then(|config| config.authentik.as_ref())
        .ok_or_else(|| ApiError::service_unavailable("Authentik sign-in is not configured."))
        .map_err(|error| error.into_response())?;

    let oidc_http_client = oidc_http_client().map_err(|error| error.into_response())?;
    let provider_metadata = CoreProviderMetadata::discover_async(
        IssuerUrl::new(config.issuer_url.clone()).map_err(|error| {
            ApiError::internal(format!("Invalid Authentik issuer URL: {error}")).into_response()
        })?,
        &oidc_http_client,
    )
    .await
    .map_err(|error| {
        ApiError::bad_gateway(format!(
            "Failed to discover Authentik OIDC metadata: {error}"
        ))
        .into_response()
    })?;

    let client = CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(config.client_id.clone()),
        Some(ClientSecret::new(config.client_secret.clone())),
    )
    .set_redirect_uri(
        RedirectUrl::new(config.redirect_uri.clone()).map_err(|error| {
            ApiError::internal(format!("Invalid Authentik redirect URI: {error}")).into_response()
        })?,
    );

    let state_token = random_token(32);
    let authorize_state_token = state_token.clone();
    let now = current_timestamp_secs();
    let return_to = sanitize_return_to(query.return_to.as_deref());
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let mut authorize_request = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            move || CsrfToken::new(authorize_state_token),
            Nonce::new_random,
        )
        .set_pkce_challenge(pkce_challenge);

    for scope in &config.scopes {
        authorize_request = authorize_request.add_scope(Scope::new(scope.clone()));
    }

    let (authorize_url, _, nonce) = authorize_request.url();

    {
        let mut pending = state.pending_oauth.lock().map_err(|_| {
            ApiError::internal("Failed to acquire OAuth state lock.").into_response()
        })?;
        pending.retain(|_, value| value.expires_at > now);
        pending.insert(
            state_token,
            PendingOAuthState {
                provider: "authentik".to_string(),
                return_to,
                expires_at: now + 600,
                oidc_nonce: Some(nonce.secret().to_string()),
                pkce_verifier: Some(pkce_verifier.secret().to_string()),
            },
        );
    }

    Ok(Redirect::temporary(authorize_url.as_ref()).into_response())
}

async fn auth_callback(
    State(state): State<AppState>,
    Query(query): Query<AuthCallbackQuery>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let provider =
        query
            .state
            .as_deref()
            .and_then(|state_token| {
                state.pending_oauth.lock().ok().and_then(|pending| {
                    pending.get(state_token).map(|value| value.provider.clone())
                })
            })
            .unwrap_or_else(|| "gitlab".to_string());

    finish_auth_callback(state, &provider, query).await
}

async fn auth_callback_provider(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(query): Query<AuthCallbackQuery>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    finish_auth_callback(state, &provider, query).await
}

async fn finish_auth_callback(
    state: AppState,
    provider: &str,
    query: AuthCallbackQuery,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    match provider {
        "gitlab" => finish_gitlab_callback(state, query).await,
        "authentik" => finish_authentik_callback(state, query).await,
        _ => Err(ApiError::bad_request("Unknown authentication provider.").into_response()),
    }
}

async fn finish_gitlab_callback(
    state: AppState,
    query: AuthCallbackQuery,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let config = state
        .auth_config
        .as_ref()
        .and_then(|config| config.gitlab.as_ref())
        .ok_or_else(|| ApiError::service_unavailable("GitLab sign-in is not configured."))
        .map_err(|error| error.into_response())?;

    let Some(return_to_state) = query.state.as_deref() else {
        return Ok(Redirect::temporary("/").into_response());
    };
    let pending = {
        let mut pending = state.pending_oauth.lock().map_err(|_| {
            ApiError::internal("Failed to acquire OAuth state lock.").into_response()
        })?;
        pending.remove(return_to_state)
    };
    let return_to = pending
        .filter(|value| value.expires_at > current_timestamp_secs())
        .map(|value| value.return_to)
        .unwrap_or_else(|| "/".to_string());

    if let Some(error) = query.error {
        let location = append_auth_error(&return_to, &query.error_description.unwrap_or(error));
        return Ok(Redirect::temporary(&location).into_response());
    }

    let code = query
        .code
        .ok_or_else(|| ApiError::bad_request("Missing GitLab OAuth code."))
        .map_err(|error| error.into_response())?;

    let token_url = format!(
        "{}/oauth/token",
        config.gitlab_base_url.trim_end_matches('/')
    );
    let token_response = state
        .http_client
        .post(token_url)
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("code", code.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", config.redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!("Failed to exchange GitLab OAuth code: {error}"))
                .into_response()
        })?;

    if !token_response.status().is_success() {
        let message = format!(
            "GitLab OAuth token exchange failed with status {}.",
            token_response.status()
        );
        let location = append_auth_error(&return_to, &message);
        return Ok(Redirect::temporary(&location).into_response());
    }

    let token_payload: GitLabTokenResponse = token_response.json().await.map_err(|error| {
        ApiError::bad_gateway(format!("Failed to parse GitLab token response: {error}"))
            .into_response()
    })?;

    let gitlab_user = fetch_gitlab_user(&state, &token_payload.access_token)
        .await
        .map_err(|error| error.into_response())?;
    let is_allowed =
        user_is_allowed_group_member(&state, &token_payload.access_token, gitlab_user.id)
            .await
            .map_err(|error| error.into_response())?;

    if !is_allowed {
        let location = append_auth_error(
            &return_to,
            "Only members of the allowed groups may access protected features.",
        );
        return Ok(Redirect::temporary(&location).into_response());
    }

    let user = workspace::upsert_authenticated_user(
        &state,
        &workspace::ExternalAuthUser {
            provider: "gitlab",
            provider_subject: gitlab_user.id.to_string(),
            email: gitlab_user.email,
            username: gitlab_user.username,
            display_name: gitlab_user.name,
            avatar_url: gitlab_user.avatar_url,
        },
    )
    .await
    .map_err(|error| error.into_response())?;
    let auth_config = state
        .auth_config
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Authentication is not configured."))
        .map_err(|error| error.into_response())?;
    let session_id = workspace::create_db_session(&state, &user.user_id, auth_config.session_ttl)
        .await
        .map_err(|error| error.into_response())?;

    let mut response = Redirect::temporary(&append_auth_success(&return_to)).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        session_cookie_header(&session_id, auth_config)
            .map_err(|error| ApiError::internal(error).into_response())?,
    );
    Ok(response)
}

async fn finish_authentik_callback(
    state: AppState,
    query: AuthCallbackQuery,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let config = state
        .auth_config
        .as_ref()
        .and_then(|config| config.authentik.as_ref())
        .ok_or_else(|| ApiError::service_unavailable("Authentik sign-in is not configured."))
        .map_err(|error| error.into_response())?;

    let Some(return_to_state) = query.state.as_deref() else {
        return Ok(Redirect::temporary("/").into_response());
    };
    let pending = {
        let mut pending = state.pending_oauth.lock().map_err(|_| {
            ApiError::internal("Failed to acquire OAuth state lock.").into_response()
        })?;
        pending.remove(return_to_state)
    };
    let pending = pending.filter(|value| value.expires_at > current_timestamp_secs());
    let return_to = pending
        .as_ref()
        .map(|value| value.return_to.clone())
        .unwrap_or_else(|| "/".to_string());

    if let Some(error) = query.error {
        let location = append_auth_error(&return_to, &query.error_description.unwrap_or(error));
        return Ok(Redirect::temporary(&location).into_response());
    }

    let Some(pending) = pending else {
        let location =
            append_auth_error(&return_to, "The sign-in request expired. Please try again.");
        return Ok(Redirect::temporary(&location).into_response());
    };

    let code = query
        .code
        .ok_or_else(|| ApiError::bad_request("Missing Authentik authorization code."))
        .map_err(|error| error.into_response())?;
    let nonce = pending
        .oidc_nonce
        .ok_or_else(|| ApiError::bad_request("Missing Authentik sign-in nonce."))
        .map_err(|error| error.into_response())?;
    let pkce_verifier = pending
        .pkce_verifier
        .ok_or_else(|| ApiError::bad_request("Missing Authentik PKCE verifier."))
        .map_err(|error| error.into_response())?;

    let oidc_http_client = oidc_http_client().map_err(|error| error.into_response())?;
    let provider_metadata = CoreProviderMetadata::discover_async(
        IssuerUrl::new(config.issuer_url.clone()).map_err(|error| {
            ApiError::internal(format!("Invalid Authentik issuer URL: {error}")).into_response()
        })?,
        &oidc_http_client,
    )
    .await
    .map_err(|error| {
        ApiError::bad_gateway(format!(
            "Failed to discover Authentik OIDC metadata: {error}"
        ))
        .into_response()
    })?;

    let client = CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(config.client_id.clone()),
        Some(ClientSecret::new(config.client_secret.clone())),
    )
    .set_redirect_uri(
        RedirectUrl::new(config.redirect_uri.clone()).map_err(|error| {
            ApiError::internal(format!("Invalid Authentik redirect URI: {error}")).into_response()
        })?,
    );

    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "Authentik token endpoint is not available: {error}"
            ))
            .into_response()
        })?
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(&oidc_http_client)
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "Failed to exchange Authentik authorization code: {error}"
            ))
            .into_response()
        })?;

    let id_token = token_response.id_token().ok_or_else(|| {
        ApiError::bad_gateway("Authentik did not return an ID token.").into_response()
    })?;
    let id_token_verifier = client.id_token_verifier();
    let claims = id_token
        .claims(&id_token_verifier, &Nonce::new(nonce))
        .map_err(|error| {
            ApiError::unauthorized(format!("Failed to verify Authentik ID token: {error}"))
                .into_response()
        })?;

    if let Some(expected_access_token_hash) = claims.access_token_hash() {
        let actual_access_token_hash = AccessTokenHash::from_token(
            token_response.access_token(),
            id_token.signing_alg().map_err(|error| {
                ApiError::unauthorized(format!("Failed to inspect Authentik ID token: {error}"))
                    .into_response()
            })?,
            id_token.signing_key(&id_token_verifier).map_err(|error| {
                ApiError::unauthorized(format!("Failed to inspect Authentik signing key: {error}"))
                    .into_response()
            })?,
        )
        .map_err(|error| {
            ApiError::unauthorized(format!("Failed to verify Authentik access token: {error}"))
                .into_response()
        })?;
        if actual_access_token_hash != *expected_access_token_hash {
            let location =
                append_auth_error(&return_to, "Authentik access token verification failed.");
            return Ok(Redirect::temporary(&location).into_response());
        }
    }

    let id_token_claims =
        decode_jwt_claims(&id_token.to_string()).map_err(|error| error.into_response())?;
    let auth_config = state
        .auth_config
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Authentication is not configured."))
        .map_err(|error| error.into_response())?;
    if !oidc_claim_has_allowed_group(
        &id_token_claims,
        &config.groups_claim,
        &auth_config.allowed_groups,
    ) {
        let location = append_auth_error(
            &return_to,
            "Only members of the allowed groups may access protected features.",
        );
        return Ok(Redirect::temporary(&location).into_response());
    }

    let email = claims
        .email()
        .map(|email| email.as_str().to_string())
        .ok_or_else(|| ApiError::unauthorized("Authentik did not provide an email address."))
        .map_err(|error| error.into_response())?;

    let user = workspace::upsert_authenticated_user(
        &state,
        &workspace::ExternalAuthUser {
            provider: "authentik",
            provider_subject: claims.subject().as_str().to_string(),
            email,
            username: None,
            display_name: None,
            avatar_url: None,
        },
    )
    .await
    .map_err(|error| error.into_response())?;
    let session_id = workspace::create_db_session(&state, &user.user_id, auth_config.session_ttl)
        .await
        .map_err(|error| error.into_response())?;

    let mut response = Redirect::temporary(&append_auth_success(&return_to)).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        session_cookie_header(&session_id, auth_config)
            .map_err(|error| ApiError::internal(error).into_response())?,
    );
    Ok(response)
}

async fn auth_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if let Some(session_id) = cookie_value(&headers, "menthtex_session") {
        workspace::delete_db_session(&state, &session_id)
            .await
            .map_err(|error| error.into_response())?;
    }

    let mut response = Json(serde_json::json!({ "status": "ok" })).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        expired_session_cookie_header(
            state
                .auth_config
                .as_ref()
                .map(|config| config.cookie_secure)
                .unwrap_or(false),
        )
        .map_err(|error| ApiError::internal(error).into_response())?,
    );
    Ok(response)
}

async fn format_entry(
    State(state): State<AppState>,
    Json(request): Json<FormatRequest>,
) -> Result<Json<FormatResponse>, (StatusCode, Json<ErrorResponse>)> {
    let response = format_bibtex_input(
        &state,
        &request.input,
        request.include_string_definitions,
        request.use_conferences_bib,
        request.rewrite_reference_key,
        request.keep_doi,
    )
    .map_err(|error| error.into_response())?;
    Ok(Json(response))
}

async fn lookup_doi(
    State(state): State<AppState>,
    Json(request): Json<DoiLookupRequest>,
) -> Result<Json<FormatResponse>, (StatusCode, Json<ErrorResponse>)> {
    let normalized_doi = normalize_doi_input(&request.doi)
        .ok_or_else(|| ApiError::bad_request("Please enter a valid DOI or DOI URL."))
        .map_err(|error| error.into_response())?;
    let retrieved_input = fetch_bibtex_for_doi(&state.http_client, &normalized_doi)
        .await
        .map_err(|error| error.into_response())?;
    let mut response = format_bibtex_input(
        &state,
        &retrieved_input,
        request.include_string_definitions,
        request.use_conferences_bib,
        request.rewrite_reference_key,
        request.keep_doi,
    )
    .map_err(|error| error.into_response())?;
    response.retrieved_input = Some(retrieved_input);
    response.normalized_doi = Some(normalized_doi);
    Ok(Json(response))
}

async fn lookup_rfc(
    State(state): State<AppState>,
    Json(request): Json<RfcLookupRequest>,
) -> Result<Json<FormatResponse>, (StatusCode, Json<ErrorResponse>)> {
    let normalized_rfc = normalize_rfc_input(&request.rfc)
        .ok_or_else(|| {
            ApiError::bad_request("Please enter a valid RFC number like RFC1034 or 1034.")
        })
        .map_err(|error| error.into_response())?;
    let retrieved_input = fetch_prepared_bibtex_for_rfc(&state.http_client, &normalized_rfc)
        .await
        .map_err(|error| error.into_response())?;
    let mut response = format_bibtex_input(
        &state,
        &retrieved_input,
        request.include_string_definitions,
        request.use_conferences_bib,
        request.rewrite_reference_key,
        request.keep_doi,
    )
    .map_err(|error| error.into_response())?;
    response.retrieved_input = Some(retrieved_input);
    response.normalized_rfc = Some(normalized_rfc);
    Ok(Json(response))
}

async fn lookup_pdf(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<FormatResponse>, (StatusCode, Json<ErrorResponse>)> {
    let upload = read_pdf_upload(&mut multipart)
        .await
        .map_err(|error| error.into_response())?;
    let temp_path = env::temp_dir().join(format!("menthtex-{}.pdf", random_token(16)));
    tokio::fs::write(&temp_path, upload.bytes)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to write temporary PDF upload: {error}"))
                .into_response()
        })?;

    let metadata_result = extract_pdf_metadata(&temp_path, upload.filename.as_deref()).await;
    let _ = tokio::fs::remove_file(&temp_path).await;
    let metadata = metadata_result.map_err(|error| error.into_response())?;

    let response = if let Some(doi) = metadata.doi.clone() {
        let retrieved_input = fetch_bibtex_for_doi(&state.http_client, &doi)
            .await
            .map_err(|error| error.into_response())?;
        let mut response = format_bibtex_input(
            &state,
            &retrieved_input,
            upload.include_string_definitions,
            upload.use_conferences_bib,
            upload.rewrite_reference_key,
            upload.keep_doi,
        )
        .map_err(|error| error.into_response())?;
        response.retrieved_input = Some(retrieved_input);
        response.normalized_doi = Some(doi);
        response
    } else {
        let partial_input = build_partial_bibtex_from_pdf_metadata(&metadata)
            .ok_or_else(|| {
                ApiError::bad_request("No DOI or usable embedded metadata found in PDF.")
            })
            .map_err(|error| error.into_response())?;
        let mut response = format_bibtex_input(
            &state,
            &partial_input,
            upload.include_string_definitions,
            upload.use_conferences_bib,
            upload.rewrite_reference_key,
            upload.keep_doi,
        )
        .map_err(|error| error.into_response())?;
        response.retrieved_input = Some(partial_input);
        response
    };

    Ok(Json(response))
}

async fn add_definition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AddDefinitionRequest>,
) -> Result<Json<AddDefinitionResponse>, (StatusCode, Json<ErrorResponse>)> {
    require_abbreviation_write_access(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;

    let _guard = state.file_lock.lock().map_err(|_| {
        ApiError::internal("Failed to acquire conferences.bib lock.").into_response()
    })?;

    let conferences_source = fs::read_to_string(&state.conferences_path).map_err(|error| {
        ApiError::internal(format!("Failed to read conferences.bib: {error}")).into_response()
    })?;
    let mut definitions =
        DefinitionCollection::from_source(&conferences_source).map_err(|error| {
            ApiError::bad_request(format!("Failed to parse conferences.bib: {error}"))
                .into_response()
        })?;

    let definition = definition_from_parts(&request.token, &request.raw_value);
    if definition.token.is_empty() || definition.raw_value.is_empty() {
        return Err(
            ApiError::bad_request("Definition token and value must not be empty.").into_response(),
        );
    }
    let definition_text = format_string_definition(&definition);
    DefinitionCollection::from_source(&definition_text).map_err(|error| {
        ApiError::bad_request(format!("Invalid @STRING definition: {error}")).into_response()
    })?;

    if definitions.contains_token(&definition.token) {
        return Ok(Json(AddDefinitionResponse {
            added_abbreviation: format!("{} = {}", definition.token, definition.expanded_value),
            definition_text,
        }));
    }

    definitions.insert(definition.clone());

    let mut updated_source = conferences_source.trim_end().to_string();
    if !updated_source.ends_with('\n') {
        updated_source.push('\n');
    }
    updated_source.push('\n');
    updated_source.push_str(&definition_text);
    updated_source.push('\n');

    fs::write(&state.conferences_path, updated_source).map_err(|error| {
        ApiError::internal(format!("Failed to update conferences.bib: {error}")).into_response()
    })?;

    Ok(Json(AddDefinitionResponse {
        added_abbreviation: format!("{} = {}", definition.token, definition.expanded_value),
        definition_text,
    }))
}

async fn list_definitions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DefinitionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    require_abbreviation_write_access(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let definitions = load_definitions(&state).map_err(|error| error.into_response())?;

    Ok(Json(DefinitionListResponse {
        definitions: definitions
            .definitions()
            .iter()
            .map(definition_response)
            .collect(),
    }))
}

async fn list_definition_options(
    State(state): State<AppState>,
) -> Result<Json<DefinitionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let definitions = load_definitions(&state).map_err(|error| error.into_response())?;

    Ok(Json(DefinitionListResponse {
        definitions: definitions
            .definitions()
            .iter()
            .map(definition_response)
            .collect(),
    }))
}

async fn create_definition_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DefinitionMutationRequest>,
) -> Result<Json<DefinitionMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    require_abbreviation_write_access(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let _guard = state.file_lock.lock().map_err(|_| {
        ApiError::internal("Failed to acquire conferences.bib lock.").into_response()
    })?;

    let (source, definitions) =
        load_definitions_source_and_collection(&state).map_err(|error| error.into_response())?;
    let new_definition = definition_from_expanded_value(&request.token, &request.expanded_value);
    validate_definition(&new_definition).map_err(|error| error.into_response())?;

    if definitions.contains_token(&new_definition.token) {
        return Err(ApiError::conflict(format!(
            "Definition token '{}' already exists.",
            new_definition.token
        ))
        .into_response());
    }

    let mut all_definitions = definitions.definitions().to_vec();
    all_definitions.push(new_definition.clone());
    write_definitions(&state, &source, &all_definitions).map_err(|error| error.into_response())?;

    Ok(Json(DefinitionMutationResponse {
        definition: definition_response(&new_definition),
    }))
}

async fn update_definition_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
    Json(request): Json<DefinitionMutationRequest>,
) -> Result<Json<DefinitionMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    require_abbreviation_write_access(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let _guard = state.file_lock.lock().map_err(|_| {
        ApiError::internal("Failed to acquire conferences.bib lock.").into_response()
    })?;

    let (source, definitions) =
        load_definitions_source_and_collection(&state).map_err(|error| error.into_response())?;
    let updated_definition =
        definition_from_expanded_value(&request.token, &request.expanded_value);
    validate_definition(&updated_definition).map_err(|error| error.into_response())?;

    let mut all_definitions = definitions.definitions().to_vec();
    let current_index = all_definitions
        .iter()
        .position(|definition| definition.token.eq_ignore_ascii_case(&token))
        .ok_or_else(|| ApiError::not_found(format!("Definition token '{}' was not found.", token)))
        .map_err(|error| error.into_response())?;

    if all_definitions
        .iter()
        .enumerate()
        .any(|(index, definition)| {
            index != current_index
                && definition
                    .token
                    .eq_ignore_ascii_case(&updated_definition.token)
        })
    {
        return Err(ApiError::conflict(format!(
            "Definition token '{}' already exists.",
            updated_definition.token
        ))
        .into_response());
    }

    all_definitions[current_index] = updated_definition.clone();
    write_definitions(&state, &source, &all_definitions).map_err(|error| error.into_response())?;

    Ok(Json(DefinitionMutationResponse {
        definition: definition_response(&updated_definition),
    }))
}

async fn delete_definition_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    require_abbreviation_write_access(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let _guard = state.file_lock.lock().map_err(|_| {
        ApiError::internal("Failed to acquire conferences.bib lock.").into_response()
    })?;

    let (source, definitions) =
        load_definitions_source_and_collection(&state).map_err(|error| error.into_response())?;
    let mut all_definitions = definitions.definitions().to_vec();
    let before = all_definitions.len();
    all_definitions.retain(|definition| !definition.token.eq_ignore_ascii_case(&token));

    if all_definitions.len() == before {
        return Err(
            ApiError::not_found(format!("Definition token '{}' was not found.", token))
                .into_response(),
        );
    }

    write_definitions(&state, &source, &all_definitions).map_err(|error| error.into_response())?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn format_bibtex_input(
    state: &AppState,
    input: &str,
    include_string_definitions: bool,
    use_conferences_bib: bool,
    rewrite_reference_key: bool,
    keep_doi: bool,
) -> Result<FormatResponse, ApiError> {
    let mut definitions = if use_conferences_bib {
        let _guard = state
            .file_lock
            .lock()
            .map_err(|_| ApiError::internal("Failed to acquire conferences.bib lock."))?;

        let conferences_source = fs::read_to_string(&state.conferences_path).map_err(|error| {
            ApiError::internal(format!("Failed to read conferences.bib: {error}"))
        })?;
        DefinitionCollection::from_source(&conferences_source).map_err(|error| {
            ApiError::bad_request(format!("Failed to parse conferences.bib: {error}"))
        })?
    } else {
        DefinitionCollection::from_source("").map_err(|error| {
            ApiError::internal(format!(
                "Failed to initialize empty definition set: {error}"
            ))
        })?
    };
    let entries = parse_bibtex_document(input)
        .map_err(|error| ApiError::bad_request(format!("Failed to parse input BibTeX: {error}")))?;

    let result = format_entries(
        &entries,
        &mut definitions,
        &FormatOptions {
            include_string_definitions,
            use_conferences_bib,
            rewrite_reference_key,
            keep_doi,
        },
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(FormatResponse {
        output: result.output,
        warnings: result.warnings,
        added_abbreviations: Vec::new(),
        proposed_definitions: result
            .added_definitions
            .iter()
            .map(proposed_definition_response)
            .collect(),
        output_definitions: result
            .output_definitions
            .iter()
            .map(definition_response_as_proposed)
            .collect(),
        entry_count: result.entry_count,
        formatted_count: result.formatted_count,
        untouched_count: result.untouched_count,
        entries: result.entries,
        retrieved_input: None,
        normalized_doi: None,
        normalized_rfc: None,
    })
}

fn proposed_definition_response(definition: &ProposedDefinition) -> ProposedDefinitionResponse {
    ProposedDefinitionResponse {
        token: definition.definition.token.clone(),
        raw_value: definition.definition.raw_value.clone(),
        expanded_value: definition.definition.expanded_value.clone(),
        definition_text: format_string_definition(&definition.definition),
        suggestions: definition
            .suggestions
            .iter()
            .map(venue_suggestion_response)
            .collect(),
    }
}

fn definition_response_as_proposed(
    definition: &bibtex::AbbreviationDefinition,
) -> ProposedDefinitionResponse {
    ProposedDefinitionResponse {
        token: definition.token.clone(),
        raw_value: definition.raw_value.clone(),
        expanded_value: definition.expanded_value.clone(),
        definition_text: format_string_definition(definition),
        suggestions: Vec::new(),
    }
}

fn venue_suggestion_response(suggestion: &VenueSuggestion) -> VenueSuggestionResponse {
    VenueSuggestionResponse {
        token: suggestion.token.clone(),
        raw_value: suggestion.raw_value.clone(),
        expanded_value: suggestion.expanded_value.clone(),
        definition_text: format_string_definition(&definition_from_parts(
            &suggestion.token,
            &suggestion.raw_value,
        )),
        score: suggestion.score,
    }
}

pub(crate) fn load_definitions(state: &AppState) -> Result<DefinitionCollection, ApiError> {
    let source = fs::read_to_string(&state.conferences_path)
        .map_err(|error| ApiError::internal(format!("Failed to read conferences.bib: {error}")))?;
    DefinitionCollection::from_source(&source)
        .map_err(|error| ApiError::bad_request(format!("Failed to parse conferences.bib: {error}")))
}

fn load_definitions_source_and_collection(
    state: &AppState,
) -> Result<(String, DefinitionCollection), ApiError> {
    let source = fs::read_to_string(&state.conferences_path)
        .map_err(|error| ApiError::internal(format!("Failed to read conferences.bib: {error}")))?;
    let definitions = DefinitionCollection::from_source(&source).map_err(|error| {
        ApiError::bad_request(format!("Failed to parse conferences.bib: {error}"))
    })?;
    Ok((source, definitions))
}

fn validate_definition(definition: &bibtex::AbbreviationDefinition) -> Result<(), ApiError> {
    if definition.token.is_empty() || definition.raw_value.is_empty() {
        return Err(ApiError::bad_request(
            "Definition token and value must not be empty.",
        ));
    }

    let definition_text = format_string_definition(definition);
    DefinitionCollection::from_source(&definition_text)
        .map_err(|error| ApiError::bad_request(format!("Invalid @STRING definition: {error}")))?;
    Ok(())
}

fn write_definitions(
    state: &AppState,
    _existing_source: &str,
    definitions: &[bibtex::AbbreviationDefinition],
) -> Result<(), ApiError> {
    let updated_source = format_definition_source(definitions);
    fs::write(&state.conferences_path, updated_source)
        .map_err(|error| ApiError::internal(format!("Failed to update conferences.bib: {error}")))
}

fn definition_response(definition: &bibtex::AbbreviationDefinition) -> DefinitionRecordResponse {
    DefinitionRecordResponse {
        token: definition.token.clone(),
        raw_value: definition.raw_value.clone(),
        expanded_value: definition.expanded_value.clone(),
        definition_text: format_string_definition(definition),
    }
}

pub(crate) fn auth_provider_options(state: &AppState) -> Vec<AuthProviderOption> {
    state
        .auth_config
        .as_ref()
        .map(auth_config_provider_options)
        .unwrap_or_default()
}

fn auth_config_provider_options(config: &AuthConfig) -> Vec<AuthProviderOption> {
    let mut providers = Vec::new();
    if config.gitlab.is_some() {
        providers.push(AuthProviderOption {
            id: "gitlab",
            label: "GitLab",
        });
    }
    if config.authentik.is_some() {
        providers.push(AuthProviderOption {
            id: "authentik",
            label: "Authentik",
        });
    }
    providers
}

fn oidc_http_client() -> Result<openidconnect::reqwest::Client, ApiError> {
    openidconnect::reqwest::ClientBuilder::new()
        .redirect(openidconnect::reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| {
            ApiError::internal(format!("Failed to construct OIDC HTTP client: {error}"))
        })
}

fn decode_jwt_claims(token: &str) -> Result<Value, ApiError> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| ApiError::bad_gateway("ID token is not a valid JWT."))?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).map_err(|error| {
        ApiError::bad_gateway(format!("Failed to decode ID token claims: {error}"))
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|error| ApiError::bad_gateway(format!("Failed to parse ID token claims: {error}")))
}

fn oidc_claim_has_allowed_group(
    claims: &Value,
    groups_claim: &str,
    allowed_groups: &[String],
) -> bool {
    let allowed = allowed_groups
        .iter()
        .map(|group| group.as_str())
        .collect::<std::collections::HashSet<_>>();
    oidc_claim_group_values(claims, groups_claim)
        .iter()
        .any(|group| allowed.contains(group.as_str()))
}

fn oidc_claim_group_values(claims: &Value, groups_claim: &str) -> Vec<String> {
    let Some(value) = claims.get(groups_claim) else {
        return Vec::new();
    };

    match value {
        Value::Array(values) => values
            .iter()
            .filter_map(|value| value.as_str().map(|value| value.to_string()))
            .collect(),
        Value::String(value) => vec![value.clone()],
        _ => Vec::new(),
    }
}

async fn fetch_gitlab_user(
    state: &AppState,
    access_token: &str,
) -> Result<GitLabUserResponse, ApiError> {
    let config = state
        .auth_config
        .as_ref()
        .and_then(|config| config.gitlab.as_ref())
        .ok_or_else(|| ApiError::service_unavailable("GitLab sign-in is not configured."))?;
    let user_url = format!(
        "{}/api/v4/user",
        config.gitlab_base_url.trim_end_matches('/')
    );
    let response = state
        .http_client
        .get(user_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!("Failed to fetch GitLab user profile: {error}"))
        })?;

    if !response.status().is_success() {
        return Err(ApiError::bad_gateway(format!(
            "GitLab user lookup failed with status {}.",
            response.status()
        )));
    }

    response.json().await.map_err(|error| {
        ApiError::bad_gateway(format!("Failed to parse GitLab user profile: {error}"))
    })
}

async fn user_is_allowed_group_member(
    state: &AppState,
    access_token: &str,
    user_id: u64,
) -> Result<bool, ApiError> {
    let config = state
        .auth_config
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Authentication is not configured."))?;
    let gitlab_config = config
        .gitlab
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("GitLab sign-in is not configured."))?;
    for allowed_group in &config.allowed_groups {
        let mut member_url =
            reqwest::Url::parse(&gitlab_config.gitlab_base_url).map_err(|error| {
                ApiError::internal(format!("Failed to construct GitLab base URL: {error}"))
            })?;
        {
            let mut segments = member_url
                .path_segments_mut()
                .map_err(|_| ApiError::internal("GitLab base URL cannot be a base path."))?;
            segments.push("api");
            segments.push("v4");
            segments.push("groups");
            for segment in allowed_group
                .split('/')
                .filter(|segment| !segment.is_empty())
            {
                segments.push(segment);
            }
            segments.push("members");
            segments.push("all");
            segments.push(&user_id.to_string());
        }

        let response = state
            .http_client
            .get(member_url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|error| {
                ApiError::bad_gateway(format!("Failed to verify GitLab group membership: {error}"))
            })?;

        match response.status() {
            StatusCode::OK => return Ok(true),
            StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => continue,
            status => {
                return Err(ApiError::bad_gateway(format!(
                    "GitLab group membership check failed with status {status}."
                )));
            }
        }
    }

    Ok(false)
}

async fn require_abbreviation_write_access(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<workspace::AuthenticatedUser, ApiError> {
    if auth_provider_options(state).is_empty() {
        return Err(ApiError::service_unavailable(
            "Authentication is not configured for this deployment.",
        ));
    }

    session_from_headers(state, headers)
        .await?
        .ok_or_else(|| ApiError::unauthorized("Sign in to add definitions to conferences.bib."))
}

pub(crate) async fn require_authenticated_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<workspace::AuthenticatedUser, ApiError> {
    if auth_provider_options(state).is_empty() {
        return Err(ApiError::service_unavailable(
            "Authentication is not configured for this deployment.",
        ));
    }

    session_from_headers(state, headers)
        .await?
        .ok_or_else(|| ApiError::unauthorized("Sign in to access the literature workspace."))
}

pub(crate) async fn session_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<workspace::AuthenticatedUser>, ApiError> {
    let Some(session_id) = cookie_value(headers, "menthtex_session") else {
        return Ok(None);
    };
    workspace::load_session_user(state, &session_id).await
}

fn cookie_value(headers: &HeaderMap, key: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|segment| {
        let trimmed = segment.trim();
        let (cookie_key, value) = trimmed.split_once('=')?;
        if cookie_key == key {
            Some(value.to_string())
        } else {
            None
        }
    })
}

fn session_cookie_header(session_id: &str, config: &AuthConfig) -> Result<HeaderValue, String> {
    HeaderValue::from_str(&format!(
        "menthtex_session={session_id}; Path=/; HttpOnly; SameSite=Lax; Max-Age={};{}",
        config.session_ttl.as_secs(),
        if config.cookie_secure { " Secure;" } else { "" }
    ))
    .map_err(|error| format!("Failed to build session cookie: {error}"))
}

fn expired_session_cookie_header(cookie_secure: bool) -> Result<HeaderValue, String> {
    HeaderValue::from_str(&format!(
        "menthtex_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT;{}",
        if cookie_secure { " Secure;" } else { "" }
    ))
    .map_err(|error| format!("Failed to clear session cookie: {error}"))
}

fn sanitize_return_to(return_to: Option<&str>) -> String {
    let candidate = return_to.unwrap_or("/").trim();
    if candidate.is_empty() || !candidate.starts_with('/') {
        "/".to_string()
    } else {
        candidate.to_string()
    }
}

fn append_auth_success(return_to: &str) -> String {
    append_query_parameter(return_to, "auth", "success")
}

fn append_auth_error(return_to: &str, error: &str) -> String {
    append_query_parameter(return_to, "auth_error", error)
}

fn append_query_parameter(base: &str, key: &str, value: &str) -> String {
    if let Ok(mut url) = reqwest::Url::parse(base) {
        url.query_pairs_mut().append_pair(key, value);
        return url.to_string();
    }

    if let Ok(base_url) = reqwest::Url::parse("http://localhost") {
        if let Ok(mut url) = base_url.join(base) {
            url.query_pairs_mut().append_pair(key, value);
            let path = url.path().to_string();
            let query = url
                .query()
                .map(|query| format!("?{query}"))
                .unwrap_or_default();
            let fragment = url
                .fragment()
                .map(|fragment| format!("#{fragment}"))
                .unwrap_or_default();
            return format!("{path}{query}{fragment}");
        }
    }

    base.to_string()
}

fn random_token(length: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn build_auth_config() -> Option<AuthConfig> {
    let gitlab = match (
        non_empty_env("GITLAB_CLIENT_ID"),
        non_empty_env("GITLAB_CLIENT_SECRET"),
        non_empty_env("GITLAB_REDIRECT_URI"),
    ) {
        (Some(client_id), Some(client_secret), Some(redirect_uri)) => Some(GitLabAuthConfig {
            gitlab_base_url: non_empty_env("GITLAB_BASE_URL")
                .unwrap_or_else(|| "https://kn-gitlab.cs.uni-tuebingen.de".to_string()),
            client_id,
            client_secret,
            redirect_uri,
        }),
        _ => None,
    };

    let authentik = match (
        non_empty_env("AUTHENTIK_ISSUER_URL"),
        non_empty_env("AUTHENTIK_CLIENT_ID"),
        non_empty_env("AUTHENTIK_CLIENT_SECRET"),
        non_empty_env("AUTHENTIK_REDIRECT_URI"),
    ) {
        (Some(issuer_url), Some(client_id), Some(client_secret), Some(redirect_uri)) => {
            Some(AuthentikAuthConfig {
                issuer_url,
                client_id,
                client_secret,
                redirect_uri,
                groups_claim: non_empty_env("AUTHENTIK_GROUPS_CLAIM")
                    .unwrap_or_else(|| "groups".to_string()),
                scopes: parse_auth_scopes(non_empty_env("AUTHENTIK_SCOPES").as_deref()),
            })
        }
        _ => None,
    };

    if gitlab.is_none() && authentik.is_none() {
        return None;
    }

    let allowed_groups = parse_allowed_groups(
        non_empty_env("AUTH_ALLOWED_GROUPS").as_deref(),
        non_empty_env("GITLAB_ALLOWED_GROUPS").as_deref(),
        non_empty_env("GITLAB_ALLOWED_GROUP").as_deref(),
    );
    let session_ttl_hours = env::var("AUTH_SESSION_TTL_HOURS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(8);
    let cookie_secure = non_empty_env("AUTH_COOKIE_SECURE")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(false);

    Some(AuthConfig {
        gitlab,
        authentik,
        allowed_groups,
        session_ttl: Duration::from_secs(session_ttl_hours * 60 * 60),
        cookie_secure,
    })
}

fn parse_allowed_groups(
    auth_groups_env: Option<&str>,
    gitlab_groups_env: Option<&str>,
    gitlab_group_env: Option<&str>,
) -> Vec<String> {
    let configured = [auth_groups_env, gitlab_groups_env, gitlab_group_env]
        .into_iter()
        .flatten()
        .find(|value| !value.trim().is_empty())
        .unwrap_or("kn-staff,kn-esslingen");
    let parsed = configured
        .split(',')
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect::<Vec<_>>();

    if parsed.is_empty() {
        vec!["kn-staff".to_string(), "kn-esslingen".to_string()]
    } else {
        parsed
    }
}

fn parse_auth_scopes(scopes_env: Option<&str>) -> Vec<String> {
    let parsed = scopes_env
        .unwrap_or("openid profile email")
        .split_whitespace()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect::<Vec<_>>();

    if parsed.is_empty() {
        vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
        ]
    } else {
        parsed
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn fetch_bibtex_for_doi(client: &Client, doi: &str) -> Result<String, ApiError> {
    let url = format!("https://doi.org/{doi}");
    let response = client
        .get(url)
        .header(header::ACCEPT, "application/x-bibtex")
        .send()
        .await
        .map_err(|error| ApiError::bad_gateway(format!("Failed to query DOI resolver: {error}")))?;

    let status = response.status();
    if !status.is_success() {
        let message = match status.as_u16() {
            404 => format!("No bibliographic entry was found for DOI '{doi}'."),
            406 => format!("The DOI resolver could not provide BibTeX for DOI '{doi}'."),
            _ => format!("DOI lookup failed with status {}.", status.as_u16()),
        };
        return Err(ApiError::bad_gateway(message));
    }

    response.text().await.map_err(|error| {
        ApiError::bad_gateway(format!("Failed to read DOI lookup response: {error}"))
    })
}

async fn fetch_bibtex_for_rfc(client: &Client, rfc_number: &str) -> Result<String, ApiError> {
    let url = format!(
        "https://datatracker.ietf.org/doc/rfc{}/bibtex/",
        rfc_number.trim()
    );
    let response = client.get(url).send().await.map_err(|error| {
        ApiError::bad_gateway(format!("Failed to query IETF Datatracker: {error}"))
    })?;

    let status = response.status();
    if !status.is_success() {
        let message = match status.as_u16() {
            404 => format!("No bibliographic entry was found for RFC {}.", rfc_number),
            _ => format!("RFC lookup failed with status {}.", status.as_u16()),
        };
        return Err(ApiError::bad_gateway(message));
    }

    response.text().await.map_err(|error| {
        ApiError::bad_gateway(format!("Failed to read RFC BibTeX response: {error}"))
    })
}

async fn fetch_rfc_editor_info_page(client: &Client, rfc_number: &str) -> Result<String, ApiError> {
    let url = format!("https://www.rfc-editor.org/info/rfc{}", rfc_number.trim());
    let response =
        client.get(url).send().await.map_err(|error| {
            ApiError::bad_gateway(format!("Failed to query RFC Editor: {error}"))
        })?;

    let status = response.status();
    if !status.is_success() {
        let message = match status.as_u16() {
            404 => format!("RFC Editor did not have metadata for RFC {}.", rfc_number),
            _ => format!(
                "RFC Editor metadata lookup failed with status {}.",
                status.as_u16()
            ),
        };
        return Err(ApiError::bad_gateway(message));
    }

    response.text().await.map_err(|error| {
        ApiError::bad_gateway(format!("Failed to read RFC Editor metadata: {error}"))
    })
}

async fn fetch_prepared_bibtex_for_rfc(
    client: &Client,
    rfc_number: &str,
) -> Result<String, ApiError> {
    let datatracker_bibtex = fetch_bibtex_for_rfc(client, rfc_number).await?;
    let base_entry = extract_primary_rfc_entry(&datatracker_bibtex)?;
    if !rfc_entry_needs_repair(&base_entry) {
        return Ok(base_entry.raw_text.trim().to_string());
    }

    let info_page = fetch_rfc_editor_info_page(client, rfc_number).await?;
    let metadata = parse_rfc_editor_metadata(&info_page);
    build_repaired_rfc_bibtex(&base_entry, &metadata, rfc_number)
}

pub(crate) fn normalize_doi_input(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut value = trimmed
        .trim_start_matches("doi:")
        .trim_start_matches("DOI:")
        .trim()
        .to_string();

    for prefix in [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi.org/",
        "dx.doi.org/",
    ] {
        if value.to_ascii_lowercase().starts_with(prefix) {
            value = value[prefix.len()..].to_string();
            break;
        }
    }

    if let Some((before_query, _)) = value.split_once('?') {
        value = before_query.to_string();
    }
    if let Some((before_fragment, _)) = value.split_once('#') {
        value = before_fragment.to_string();
    }

    let normalized = value
        .trim()
        .trim_matches('/')
        .trim_matches(|character: char| {
            !character.is_ascii_alphanumeric()
                && !matches!(character, '.' | '/' | '-' | '_' | ';' | '(' | ')')
        })
        .to_string();
    if normalized.to_ascii_lowercase().starts_with("10.") && normalized.contains('/') {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_rfc_input(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed
        .to_ascii_lowercase()
        .replace("request for comments", "rfc")
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let digits = normalized.strip_prefix("rfc").unwrap_or(&normalized);
    if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }

    let stripped = digits.trim_start_matches('0');
    Some(if stripped.is_empty() {
        "0".to_string()
    } else {
        stripped.to_string()
    })
}

fn extract_primary_rfc_entry(source: &str) -> Result<RawEntry, ApiError> {
    parse_bibtex_document(source)
        .map_err(|error| {
            ApiError::bad_gateway(format!("Failed to parse Datatracker BibTeX: {error}"))
        })?
        .into_iter()
        .find(|entry| entry.entry_type.eq_ignore_ascii_case("misc"))
        .ok_or_else(|| {
            ApiError::bad_gateway("Datatracker did not return a usable RFC BibTeX entry.")
        })
}

fn rfc_entry_needs_repair(entry: &RawEntry) -> bool {
    rfc_field_value(entry, "author").is_none()
        || rfc_field_value(entry, "title").is_none()
        || rfc_field_value(entry, "year").is_none()
        || rfc_field_value(entry, "number").is_none()
        || rfc_field_value(entry, "pagetotal").is_none()
}

fn rfc_field_value(entry: &RawEntry, field: &str) -> Option<String> {
    entry
        .fields
        .get(field)
        .map(|value| strip_wrapping(value))
        .and_then(|value| {
            let normalized = value.trim().to_string();
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        })
}

fn build_repaired_rfc_bibtex(
    entry: &RawEntry,
    metadata: &RfcEditorMetadata,
    rfc_number: &str,
) -> Result<String, ApiError> {
    let key = if entry.key.trim().is_empty() {
        format!("rfc{}", rfc_number)
    } else {
        entry.key.trim().to_string()
    };
    let author = rfc_field_value(entry, "author")
        .or_else(|| metadata.author.clone())
        .ok_or_else(|| {
            ApiError::bad_gateway(format!(
                "RFC {} metadata did not contain usable author information.",
                rfc_number
            ))
        })?;
    let title = rfc_field_value(entry, "title")
        .or_else(|| metadata.title.clone())
        .ok_or_else(|| {
            ApiError::bad_gateway(format!(
                "RFC {} metadata did not contain a usable title.",
                rfc_number
            ))
        })?;
    let year = rfc_field_value(entry, "year")
        .or_else(|| metadata.year.clone())
        .ok_or_else(|| {
            ApiError::bad_gateway(format!(
                "RFC {} metadata did not contain a usable year.",
                rfc_number
            ))
        })?;
    let number = rfc_field_value(entry, "number").unwrap_or_else(|| rfc_number.to_string());
    let pagetotal = rfc_field_value(entry, "pagetotal")
        .or_else(|| metadata.pagetotal.clone())
        .ok_or_else(|| {
            ApiError::bad_gateway(format!(
                "RFC {} metadata did not contain a usable page count.",
                rfc_number
            ))
        })?;
    let month = rfc_field_value(entry, "month").or_else(|| metadata.month.clone());

    let mut rendered_fields = vec![
        ("series".to_string(), "{Request for Comments}".to_string()),
        ("number".to_string(), number),
    ];

    for field in ["howpublished", "publisher", "doi", "url"] {
        if let Some(value) = entry
            .fields
            .get(field)
            .cloned()
            .filter(|value| !strip_wrapping(value).is_empty())
        {
            rendered_fields.push((field.to_string(), value));
        }
    }

    rendered_fields.push(("author".to_string(), format!("{{{author}}}")));
    rendered_fields.push(("title".to_string(), format!("{{{title}}}")));
    rendered_fields.push(("pagetotal".to_string(), pagetotal));
    rendered_fields.push(("year".to_string(), year));

    if let Some(month) = month {
        rendered_fields.push(("month".to_string(), month));
    }

    if let Some(value) = entry
        .fields
        .get("abstract")
        .cloned()
        .filter(|value| !strip_wrapping(value).is_empty())
    {
        rendered_fields.push(("abstract".to_string(), value));
    }

    let preferred = rendered_fields
        .into_iter()
        .map(|(field, value)| format!("    {field} = {value}"))
        .collect::<Vec<_>>();

    Ok(format!("@misc{{{key},\n{}\n}}", preferred.join(",\n")))
}

fn parse_rfc_editor_metadata(html: &str) -> RfcEditorMetadata {
    let text = html_to_text(html);
    parse_rfc_editor_metadata_from_text(&text)
}

fn parse_rfc_editor_metadata_from_text(text: &str) -> RfcEditorMetadata {
    let lines = text
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let mut metadata = RfcEditorMetadata::default();
    let mut index = 0usize;
    while index < lines.len() {
        let line = &lines[index];

        if metadata.title.is_none() {
            let candidate = line.strip_prefix("### ").unwrap_or(line);
            if let Some((title, month_year)) = candidate.rsplit_once(',') {
                if let Some((month, year)) = parse_rfc_month_year(month_year.trim()) {
                    metadata.title = Some(title.trim().to_string());
                    metadata.month = Some(month);
                    metadata.year = Some(year);
                }
            }
        }

        if line.eq_ignore_ascii_case("Author:") || line.eq_ignore_ascii_case("Authors:") {
            let mut authors = Vec::new();
            index += 1;
            while index < lines.len() {
                let candidate = &lines[index];
                if is_rfc_editor_metadata_boundary(candidate) {
                    index = index.saturating_sub(1);
                    break;
                }
                let cleaned = candidate
                    .trim()
                    .trim_end_matches(", Ed.")
                    .trim_end_matches(", ed.")
                    .trim_end_matches(", Ed")
                    .trim()
                    .to_string();
                if !cleaned.is_empty() {
                    authors.push(cleaned);
                }
                index += 1;
            }
            if !authors.is_empty() {
                metadata.author = Some(authors.join(" and "));
            }
        }

        index += 1;
    }

    metadata
}

fn is_rfc_editor_metadata_boundary(line: &str) -> bool {
    matches!(
        line,
        "Stream:" | "Source:" | "Cite this RFC:" | "DOI:" | "Discuss this RFC:" | "Other actions:"
    ) || line.starts_with("## ")
        || line == "* * *"
}

fn parse_rfc_month_year(input: &str) -> Option<(String, String)> {
    let (month, year) = input.split_once(' ')?;
    let normalized_month = match month.trim().to_ascii_lowercase().as_str() {
        "january" => "jan",
        "february" => "feb",
        "march" => "mar",
        "april" => "apr",
        "may" => "may",
        "june" => "jun",
        "july" => "jul",
        "august" => "aug",
        "september" => "sep",
        "october" => "oct",
        "november" => "nov",
        "december" => "dec",
        _ => return None,
    };
    let normalized_year = year.trim();
    if normalized_year.len() == 4
        && normalized_year
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        Some((normalized_month.to_string(), normalized_year.to_string()))
    } else {
        None
    }
}

fn html_to_text(input: &str) -> String {
    let mut output = String::new();
    let mut tag = String::new();
    let mut entity = String::new();
    let mut in_tag = false;
    let mut in_entity = false;

    for character in input.chars() {
        if in_tag {
            if character == '>' {
                if should_break_after_tag(&tag) && !output.ends_with('\n') {
                    output.push('\n');
                }
                tag.clear();
                in_tag = false;
            } else {
                tag.push(character);
            }
            continue;
        }

        if in_entity {
            if character == ';' {
                output.push_str(&decode_html_entity(&entity));
                entity.clear();
                in_entity = false;
            } else if entity.len() > 16 {
                output.push('&');
                output.push_str(&entity);
                output.push(character);
                entity.clear();
                in_entity = false;
            } else {
                entity.push(character);
            }
            continue;
        }

        match character {
            '<' => in_tag = true,
            '&' => in_entity = true,
            _ => output.push(character),
        }
    }

    output
}

fn should_break_after_tag(tag: &str) -> bool {
    let normalized = tag
        .trim()
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "br" | "p"
            | "div"
            | "section"
            | "article"
            | "li"
            | "ul"
            | "ol"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "dt"
            | "dd"
            | "tr"
            | "td"
            | "th"
    )
}

fn decode_html_entity(entity: &str) -> String {
    match entity {
        "nbsp" => " ".to_string(),
        "amp" => "&".to_string(),
        "quot" => "\"".to_string(),
        "apos" | "#39" => "'".to_string(),
        "lt" => "<".to_string(),
        "gt" => ">".to_string(),
        _ if entity.starts_with("#x") => u32::from_str_radix(&entity[2..], 16)
            .ok()
            .and_then(char::from_u32)
            .map(|character| character.to_string())
            .unwrap_or_default(),
        _ if entity.starts_with('#') => entity[1..]
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map(|character| character.to_string())
            .unwrap_or_default(),
        _ => format!("&{entity};"),
    }
}

pub(crate) fn strip_wrapping(raw: &str) -> String {
    let mut current = raw.trim();
    while current.len() >= 2 {
        let wrapped_in_braces = current.starts_with('{') && current.ends_with('}');
        let wrapped_in_quotes = current.starts_with('"') && current.ends_with('"');
        if !(wrapped_in_braces || wrapped_in_quotes) {
            break;
        }
        current = current[1..current.len() - 1].trim();
    }
    current.to_string()
}

async fn read_pdf_upload(multipart: &mut Multipart) -> Result<PdfUploadRequest, ApiError> {
    let mut file_bytes = None;
    let mut filename = None;
    let mut include_string_definitions = true;
    let mut use_conferences_bib = true;
    let mut rewrite_reference_key = true;
    let mut keep_doi = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(format!("Failed to read PDF upload: {error}")))?
    {
        let field_name = field.name().unwrap_or_default().to_string();
        if field_name == "file" {
            if file_bytes.is_some() {
                return Err(ApiError::bad_request("Please upload exactly one PDF file."));
            }
            let provided_filename = field.file_name().map(|value| value.to_string());
            let provided_mime = field.content_type().map(|value| value.to_string());
            let bytes = field.bytes().await.map_err(|error| {
                ApiError::bad_request(format!("Failed to read uploaded PDF: {error}"))
            })?;
            if bytes.is_empty() {
                return Err(ApiError::bad_request("Uploaded PDF file is empty."));
            }
            let filename_is_pdf = provided_filename
                .as_deref()
                .map(|value| value.to_ascii_lowercase().ends_with(".pdf"))
                .unwrap_or(false);
            let mime_is_pdf = provided_mime
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("application/pdf"))
                .unwrap_or(false);
            if !filename_is_pdf && !mime_is_pdf {
                return Err(ApiError::bad_request("Only PDF uploads are supported."));
            }
            filename = provided_filename;
            file_bytes = Some(bytes.to_vec());
        } else if field_name == "include_string_definitions" {
            let value = field.text().await.map_err(|error| {
                ApiError::bad_request(format!("Failed to read upload field: {error}"))
            })?;
            include_string_definitions = parse_boolean_field(&value);
        } else if field_name == "use_conferences_bib" {
            let value = field.text().await.map_err(|error| {
                ApiError::bad_request(format!("Failed to read upload field: {error}"))
            })?;
            use_conferences_bib = parse_boolean_field(&value);
        } else if field_name == "rewrite_reference_key" {
            let value = field.text().await.map_err(|error| {
                ApiError::bad_request(format!("Failed to read upload field: {error}"))
            })?;
            rewrite_reference_key = parse_boolean_field(&value);
        } else if field_name == "keep_doi" {
            let value = field.text().await.map_err(|error| {
                ApiError::bad_request(format!("Failed to read upload field: {error}"))
            })?;
            keep_doi = parse_boolean_field(&value);
        }
    }

    Ok(PdfUploadRequest {
        bytes: file_bytes.ok_or_else(|| ApiError::bad_request("Please upload one PDF file."))?,
        filename,
        include_string_definitions,
        use_conferences_bib,
        rewrite_reference_key,
        keep_doi,
    })
}

fn parse_boolean_field(value: &str) -> bool {
    matches!(value.trim(), "1" | "true" | "TRUE" | "True" | "on")
}

fn default_true() -> bool {
    true
}

fn parse_boolean_env(value: &str) -> bool {
    matches!(
        value.trim(),
        "1" | "true" | "TRUE" | "True" | "on" | "yes" | "YES" | "Yes"
    )
}

pub(crate) async fn extract_pdf_metadata(
    path: &PathBuf,
    filename: Option<&str>,
) -> Result<PdfMetadataCandidate, ApiError> {
    let path_string = path.to_string_lossy().to_string();
    let pdfinfo_output = run_command("pdfinfo", &[path_string.as_str()]).await?;
    let info_fields = parse_pdfinfo_output(&pdfinfo_output);
    let extracted_text = run_command("pdftotext", &[path_string.as_str(), "-"]).await?;
    let front_matter = extract_front_matter_metadata(&extracted_text);

    let title = info_fields
        .get("title")
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| front_matter.title.clone())
        .or_else(|| filename.and_then(filename_to_title_hint));
    let author = info_fields
        .get("author")
        .and_then(|value| normalize_pdf_author_metadata(value))
        .or_else(|| front_matter.author.clone());
    let year = info_fields
        .get("creationdate")
        .and_then(|value| extract_year_from_metadata(value))
        .or_else(|| {
            info_fields
                .get("moddate")
                .and_then(|value| extract_year_from_metadata(value))
        })
        .or_else(|| front_matter.year.clone());
    let doi = info_fields
        .values()
        .find_map(|value| search_doi_in_text(value))
        .or_else(|| front_matter.doi.clone())
        .filter(|value| !is_repository_style_doi(value));

    Ok(PdfMetadataCandidate {
        title,
        author,
        year,
        doi,
        venue: front_matter.venue,
        venue_kind: front_matter.venue_kind,
        abstract_text: front_matter.abstract_text,
        extracted_text,
    })
}

async fn run_command(command: &str, args: &[&str]) -> Result<String, ApiError> {
    let output = Command::new(command)
        .args(args)
        .output()
        .await
        .map_err(|error| ApiError::bad_gateway(format!("Failed to run {command}: {error}")))?;

    if !output.status.success() {
        return Err(ApiError::bad_gateway(format!(
            "{command} failed while processing the uploaded PDF."
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_pdfinfo_output(output: &str) -> HashMap<String, String> {
    output
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect()
}

fn extract_year_from_metadata(value: &str) -> Option<String> {
    let digits = value.chars().collect::<Vec<_>>();
    for window in digits.windows(4) {
        let candidate = window.iter().collect::<String>();
        if candidate
            .chars()
            .all(|character| character.is_ascii_digit())
        {
            let year = candidate.parse::<u16>().ok()?;
            if (1900..=2100).contains(&year) {
                return Some(candidate);
            }
        }
    }
    None
}

fn normalize_pdf_author_metadata(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed
        .replace(';', " and ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn filename_to_title_hint(filename: &str) -> Option<String> {
    let stem = filename
        .rsplit_once('.')
        .map(|(base, _)| base)
        .unwrap_or(filename);
    let normalized = stem
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn extract_front_matter_metadata(extracted_text: &str) -> FrontMatterMetadata {
    let snippet = extracted_text.chars().take(16_000).collect::<String>();
    if snippet.trim().is_empty() {
        return FrontMatterMetadata::default();
    }

    let lines = snippet
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .take(48)
        .collect::<Vec<_>>();

    let publication_notice = detect_publication_notice(&lines);
    let title = detect_front_matter_title(&lines);
    let author = detect_front_matter_authors(&lines, title.as_deref());
    let year = detect_front_matter_year(&lines);
    let doi = publication_notice
        .as_ref()
        .and_then(|notice| notice.repository_doi.clone())
        .or_else(|| detect_front_matter_doi(&lines, title.as_deref()));
    let (venue, venue_kind) = publication_notice
        .as_ref()
        .map(|notice| (notice.venue.clone(), Some("booktitle".to_string())))
        .unwrap_or_else(|| detect_front_matter_venue(&lines, title.as_deref(), author.as_deref()));
    let abstract_text = extract_front_matter_abstract(&lines);

    FrontMatterMetadata {
        title,
        author,
        year,
        doi,
        venue,
        venue_kind,
        abstract_text,
    }
}

fn detect_front_matter_title(lines: &[String]) -> Option<String> {
    for index in 0..lines.len().min(18) {
        let line = lines[index].trim();
        if !looks_like_title_line(line) {
            continue;
        }

        let mut candidate = line.to_string();
        let mut offset = 1usize;
        while index + offset < lines.len() && offset < 3 {
            let next = lines[index + offset].trim();
            if looks_like_title_continuation(next) {
                candidate.push(' ');
                candidate.push_str(next);
                offset += 1;
            } else {
                break;
            }
        }
        return Some(candidate);
    }

    None
}

fn looks_like_title_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 18 || trimmed.len() > 220 {
        return false;
    }
    if looks_like_author_line(trimmed) {
        return false;
    }
    let lower = trimmed.to_lowercase();
    if lower.starts_with("this work has been published")
        || lower.starts_with("digital object identifier")
        || lower.starts_with("doi")
        || lower.starts_with("abstract")
        || lower.starts_with("keywords")
        || lower.starts_with("index terms")
        || lower.starts_with("arxiv")
        || lower.starts_with("accepted")
        || lower.contains("creative commons")
        || lower.contains("attribution")
        || lower.contains("license")
        || lower.contains("digital object identifier")
        || lower.contains("www.")
        || lower.contains('@')
    {
        return false;
    }
    if trimmed.split_whitespace().count() < 4 {
        return false;
    }
    let alpha_count = trimmed
        .chars()
        .filter(|character| character.is_alphabetic())
        .count();
    if alpha_count < 12 {
        return false;
    }
    let uppercase_count = trimmed
        .chars()
        .filter(|character| character.is_ascii_uppercase())
        .count();
    if uppercase_count > 0 && uppercase_count * 100 / alpha_count.max(1) > 75 {
        return false;
    }
    true
}

fn looks_like_title_continuation(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 140
        && trimmed.split_whitespace().count() >= 2
        && !looks_like_author_line(trimmed)
        && !looks_like_front_matter_section_heading(trimmed)
        && !trimmed.to_lowercase().starts_with("abstract")
}

fn detect_front_matter_authors(lines: &[String], title: Option<&str>) -> Option<String> {
    let title_lower = title.map(|value| value.to_lowercase());
    for line in lines.iter().take(18) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if title_lower
            .as_ref()
            .map(|value| trimmed.to_lowercase() == *value)
            .unwrap_or(false)
        {
            continue;
        }
        if looks_like_author_line(trimmed) {
            return normalize_front_matter_author_line(trimmed);
        }
    }
    None
}

fn normalize_front_matter_author_line(line: &str) -> Option<String> {
    let replaced = line
        .replace(" ,", ",")
        .replace(", and", ",")
        .replace(" and ", ",");
    let normalized = replaced
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.join(" and "))
    }
}

fn looks_like_author_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 8 || trimmed.len() > 180 {
        return false;
    }
    let lower = trimmed.to_lowercase();
    if lower.starts_with("abstract")
        || lower.starts_with("keywords")
        || lower.starts_with("index terms")
        || lower.starts_with("this work has been published")
        || lower.contains("university")
        || lower.contains("department")
        || lower.contains("institute")
        || lower.contains("laboratory")
        || lower.contains("conference")
        || lower.contains("journal")
        || lower.contains("creative commons")
        || lower.contains("license")
    {
        return false;
    }

    let separators = trimmed.matches(',').count()
        + trimmed.matches(';').count()
        + lower.matches(" and ").count();
    let titlecase_words = trimmed
        .split_whitespace()
        .filter(|word| {
            let token = word.trim_matches(|character: char| !character.is_alphabetic());
            token.len() >= 2
                && token
                    .chars()
                    .next()
                    .map(|character| character.is_uppercase())
                    .unwrap_or(false)
        })
        .count();

    separators > 0 && titlecase_words >= 3
}

fn detect_front_matter_year(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .take(20)
        .find_map(|line| extract_year_from_metadata(line))
}

fn detect_front_matter_doi(lines: &[String], title: Option<&str>) -> Option<String> {
    let title_lower = title.map(|value| value.to_lowercase());
    lines
        .iter()
        .take(28)
        .filter(|line| {
            let lower = line.to_lowercase();
            !lower.starts_with("this work has been published")
                && !lower.contains("depositonce")
                && !title_lower
                    .as_ref()
                    .map(|value| *value == lower)
                    .unwrap_or(false)
        })
        .find_map(|line| search_doi_in_text(line))
        .filter(|value| !is_repository_style_doi(value))
}

fn detect_front_matter_venue(
    lines: &[String],
    title: Option<&str>,
    authors: Option<&str>,
) -> (Option<String>, Option<String>) {
    let title_lower = title.map(|value| value.to_lowercase());
    let authors_lower = authors.map(|value| value.to_lowercase());

    for line in lines.iter().take(24) {
        let trimmed = line.trim();
        if trimmed.len() < 8 || trimmed.len() > 220 {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if title_lower
            .as_ref()
            .map(|value| *value == lower)
            .unwrap_or(false)
            || authors_lower
                .as_ref()
                .map(|value| *value == lower)
                .unwrap_or(false)
            || lower.starts_with("abstract")
            || lower.starts_with("keywords")
            || lower.starts_with("index terms")
            || lower.contains("doi")
        {
            continue;
        }

        if lower.contains("journal")
            || lower.contains("transactions on")
            || lower.contains("letters")
            || lower.contains("magazine")
            || lower.contains("communications")
        {
            return (Some(trimmed.to_string()), Some("journal".to_string()));
        }

        if lower.contains("conference")
            || lower.contains("symposium")
            || lower.contains("workshop")
            || lower.contains("proceedings")
        {
            return (Some(trimmed.to_string()), Some("booktitle".to_string()));
        }
    }

    (None, None)
}

#[derive(Debug, Clone)]
struct PublicationNotice {
    venue: Option<String>,
    repository_doi: Option<String>,
}

fn detect_publication_notice(lines: &[String]) -> Option<PublicationNotice> {
    let line = lines.iter().take(3).find(|line| {
        line.to_lowercase()
            .starts_with("this work has been published")
    })?;
    let lower = line.to_lowercase();
    let venue = lower
        .find("published at the ")
        .map(|index| &line[index + "published at the ".len()..])
        .and_then(|value| {
            value
                .split_once(" under the")
                .map(|(before, _)| before)
                .or(Some(value))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let repository_doi = search_doi_in_text(line);

    Some(PublicationNotice {
        venue,
        repository_doi,
    })
}

fn extract_front_matter_abstract(lines: &[String]) -> Option<String> {
    let mut abstract_index = None;
    for (index, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();
        if lower.starts_with("abstract") {
            abstract_index = Some(index);
            break;
        }
    }
    let start = abstract_index?;
    let mut section = lines[start]
        .trim()
        .trim_start_matches("Abstract")
        .trim_start_matches("Abstract—")
        .trim_start_matches("Abstract-")
        .trim_start_matches(':')
        .trim_start_matches(['-', '—', '–', ':'])
        .trim()
        .to_string();

    for next in lines.iter().skip(start + 1).take(12) {
        if looks_like_front_matter_section_heading(next)
            || next.to_lowercase().starts_with("index terms")
            || next.to_lowercase().starts_with("keywords")
        {
            break;
        }
        if !section.is_empty() {
            section.push(' ');
        }
        section.push_str(next.trim());
    }

    let normalized = section
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    (!normalized.is_empty()).then_some(normalized)
}

fn looks_like_front_matter_section_heading(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }

    let lower = trimmed.to_lowercase();
    if lower.starts_with("introduction")
        || lower.starts_with("references")
        || lower.starts_with("conclusion")
        || lower.starts_with("related work")
        || lower.starts_with("discussion")
        || lower.starts_with("abstract")
    {
        return true;
    }

    let compact = trimmed.trim_matches(|character: char| character == '.' || character == ':');
    compact.chars().all(|character| {
        character.is_ascii_uppercase()
            || character.is_ascii_digit()
            || character.is_whitespace()
            || matches!(character, 'I' | 'V' | 'X')
    })
}

fn search_doi_in_text(input: &str) -> Option<String> {
    let characters = input.chars().collect::<Vec<_>>();
    let mut index = 0usize;

    while index + 3 < characters.len() {
        if characters[index] == '1' && characters[index + 1] == '0' && characters[index + 2] == '.'
        {
            let mut candidate = String::new();
            let mut cursor = index;
            let mut newline_streak = 0usize;

            while cursor < characters.len() && candidate.len() < 180 {
                let character = characters[cursor];
                if character == '\n' || character == '\r' {
                    newline_streak += 1;
                    if newline_streak > 1 {
                        break;
                    }
                    candidate.push(' ');
                    cursor += 1;
                    continue;
                }
                newline_streak = 0;

                if character.is_whitespace() {
                    candidate.push(' ');
                    cursor += 1;
                    continue;
                }
                if is_doi_character(character)
                    || matches!(
                        character,
                        '.' | ':' | ';' | ',' | '/' | '-' | '_' | '(' | ')'
                    )
                {
                    candidate.push(character);
                    cursor += 1;
                    continue;
                }
                break;
            }

            if let Some(normalized) = normalize_doi_candidate(&candidate) {
                return Some(normalized);
            }
            index = cursor.max(index + 1);
        } else {
            index += 1;
        }
    }

    None
}

fn is_doi_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '/' | '-' | '_' | '.' | ';' | '(' | ')')
}

fn normalize_doi_candidate(raw: &str) -> Option<String> {
    let mut compact = raw
        .replace(['\n', '\r', '\t'], " ")
        .split_whitespace()
        .collect::<String>()
        .trim_matches(|character: char| {
            !character.is_ascii_alphanumeric()
                && !matches!(character, '.' | '/' | '-' | '_' | ';' | '(' | ')')
        })
        .trim_end_matches('�')
        .trim_end_matches(|character: char| ",.;:".contains(character))
        .to_string();
    for marker in [
        "Abstract",
        "ABSTRACT",
        "Keywords",
        "KEYWORDS",
        "IndexTerms",
        "Indexterms",
    ] {
        if let Some((before, _)) = compact.split_once(marker) {
            compact = before.to_string();
        }
    }
    normalize_doi_input(&compact)
}

pub(crate) fn is_repository_style_doi(doi: &str) -> bool {
    let lowered = doi.to_ascii_lowercase();
    lowered.contains("depositonce")
}

pub(crate) fn build_partial_bibtex_from_pdf_metadata(
    metadata: &PdfMetadataCandidate,
) -> Option<String> {
    let title = metadata.title.as_deref()?.trim();
    let author = metadata.author.as_deref()?.trim();
    let year = metadata.year.as_deref()?.trim();
    if title.is_empty() || author.is_empty() || year.is_empty() {
        return None;
    }

    let entry_type = if metadata.venue_kind.as_deref() == Some("booktitle") {
        "inproceedings"
    } else {
        "article"
    };
    let mut fields = vec![
        format!("  author = {{{author}}}"),
        format!("  title = {{{title}}}"),
        format!("  year = {{{year}}}"),
    ];
    if let Some(doi) = metadata
        .doi
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        fields.push(format!("  doi = {{{doi}}}"));
    }
    if let Some(venue) = metadata
        .venue
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let field_name = if metadata.venue_kind.as_deref() == Some("booktitle") {
            "booktitle"
        } else {
            "journal"
        };
        fields.push(format!("  {field_name} = {{{venue}}}"));
    }

    Some(format!(
        "@{entry_type}{{pdf_lookup,\n{}\n}}",
        fields.join(",\n")
    ))
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    fn bad_gateway(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }

    fn into_response(self) -> (StatusCode, Json<ErrorResponse>) {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PdfMetadataCandidate, build_partial_bibtex_from_pdf_metadata, build_repaired_rfc_bibtex,
        extract_front_matter_metadata, extract_primary_rfc_entry, filename_to_title_hint,
        normalize_doi_input, normalize_rfc_input, oidc_claim_group_values,
        oidc_claim_has_allowed_group, parse_allowed_groups, parse_rfc_editor_metadata_from_text,
        search_doi_in_text,
    };
    use serde_json::json;

    #[test]
    fn normalizes_plain_and_url_dois() {
        assert_eq!(
            normalize_doi_input("10.1145/1234567.7654321").as_deref(),
            Some("10.1145/1234567.7654321")
        );
        assert_eq!(
            normalize_doi_input("https://doi.org/10.1145/1234567.7654321").as_deref(),
            Some("10.1145/1234567.7654321")
        );
        assert_eq!(
            normalize_doi_input("doi:10.17487/RFC1034").as_deref(),
            Some("10.17487/RFC1034")
        );
        assert_eq!(
            normalize_doi_input("https://dx.doi.org/10.17487/RFC1034?foo=bar").as_deref(),
            Some("10.17487/RFC1034")
        );
        assert_eq!(normalize_doi_input("not-a-doi"), None);
    }

    #[test]
    fn normalizes_flexible_rfc_inputs() {
        assert_eq!(normalize_rfc_input("RFC1034").as_deref(), Some("1034"));
        assert_eq!(normalize_rfc_input("rfc8655").as_deref(), Some("8655"));
        assert_eq!(normalize_rfc_input("RFC 1034").as_deref(), Some("1034"));
        assert_eq!(normalize_rfc_input("1034").as_deref(), Some("1034"));
        assert_eq!(normalize_rfc_input("RFC-1034"), None);
        assert_eq!(normalize_rfc_input("RFCABC"), None);
        assert_eq!(normalize_rfc_input(""), None);
    }

    #[test]
    fn parses_allowed_groups_with_auth_then_gitlab_then_legacy_then_default() {
        assert_eq!(
            parse_allowed_groups(
                Some("kn-staff, kn-esslingen"),
                Some("ignored"),
                Some("ignored-legacy")
            ),
            vec!["kn-staff".to_string(), "kn-esslingen".to_string()]
        );
        assert_eq!(
            parse_allowed_groups(None, Some("kn-staff"), Some("ignored")),
            vec!["kn-staff".to_string()]
        );
        assert_eq!(
            parse_allowed_groups(None, None, Some("kn-legacy")),
            vec!["kn-legacy".to_string()]
        );
        assert_eq!(
            parse_allowed_groups(Some("   "), Some("kn-staff"), None),
            vec!["kn-staff".to_string()]
        );
        assert_eq!(
            parse_allowed_groups(Some("   "), None, Some("kn-legacy")),
            vec!["kn-legacy".to_string()]
        );
        assert_eq!(
            parse_allowed_groups(Some("   "), None, None),
            vec!["kn-staff".to_string(), "kn-esslingen".to_string()]
        );
    }

    #[test]
    fn extracts_oidc_group_claim_arrays_and_strings() {
        assert_eq!(
            oidc_claim_group_values(&json!({ "groups": ["kn-staff", "other", 5] }), "groups"),
            vec!["kn-staff".to_string(), "other".to_string()]
        );
        assert_eq!(
            oidc_claim_group_values(&json!({ "groups": "kn-staff" }), "groups"),
            vec!["kn-staff".to_string()]
        );
        assert!(oidc_claim_group_values(&json!({ "roles": ["kn-staff"] }), "groups").is_empty());
    }

    #[test]
    fn checks_oidc_group_claim_against_allowed_groups() {
        let claims = json!({ "groups": ["kn-staff", "other"] });
        let allowed = vec!["kn-staff".to_string(), "kn-esslingen".to_string()];
        assert!(oidc_claim_has_allowed_group(&claims, "groups", &allowed));

        let denied_claims = json!({ "groups": ["external"] });
        assert!(!oidc_claim_has_allowed_group(
            &denied_claims,
            "groups",
            &allowed
        ));

        let missing_claims = json!({ "email": "person@example.org" });
        assert!(!oidc_claim_has_allowed_group(
            &missing_claims,
            "groups",
            &allowed
        ));
    }

    #[test]
    fn finds_doi_inside_pdf_text_snippets() {
        assert_eq!(
            search_doi_in_text("Read more at https://doi.org/10.1109/OJCOMS.2025.3557082.")
                .as_deref(),
            Some("10.1109/OJCOMS.2025.3557082")
        );
    }

    #[test]
    fn uses_filename_as_title_hint() {
        assert_eq!(
            filename_to_title_hint("mpls-network-actions.pdf").as_deref(),
            Some("mpls network actions")
        );
    }

    #[test]
    fn builds_partial_bibtex_from_pdf_metadata_when_enough_fields_exist() {
        let metadata = PdfMetadataCandidate {
            title: Some("UniCorn-P4".to_string()),
            author: Some("Ihle, Fabian and Menth, Michael".to_string()),
            year: Some("2025".to_string()),
            doi: None,
            venue: Some("IEEE Network".to_string()),
            venue_kind: Some("journal".to_string()),
            abstract_text: None,
            extracted_text: String::new(),
        };

        let partial = build_partial_bibtex_from_pdf_metadata(&metadata).unwrap();
        assert!(partial.contains("@article{pdf_lookup,"));
        assert!(partial.contains("author = {Ihle, Fabian and Menth, Michael}"));
        assert!(partial.contains("title = {UniCorn-P4}"));
        assert!(partial.contains("year = {2025}"));
        assert!(partial.contains("journal = {IEEE Network}"));
    }

    #[test]
    fn finds_wrapped_doi_inside_pdf_text() {
        assert_eq!(
            search_doi_in_text("doi 10.1145/\n1234567.7654321").as_deref(),
            Some("10.1145/1234567.7654321")
        );
    }

    #[test]
    fn extracts_front_matter_title_and_doi() {
        let metadata = extract_front_matter_metadata(
            "Packet Level Resilience for the User Plane in 5G Networks\nFabian Ihle, Tobias Meuser and Michael Menth\nIEEE Network, 2025\nDOI: 10.15496/publikation-105098\nAbstract ...",
        );
        assert_eq!(
            metadata.title.as_deref(),
            Some("Packet Level Resilience for the User Plane in 5G Networks")
        );
        assert_eq!(metadata.year.as_deref(), Some("2025"));
        assert_eq!(metadata.doi.as_deref(), Some("10.15496/publikation-105098"));
    }

    #[test]
    fn ignores_repository_notice_and_extracts_actual_front_matter() {
        let metadata = extract_front_matter_metadata(
            "This work has been published at the 1st Workshop on Resilient Networks and Systems (ReNeSys), Sept. 2025, Ilmenau/Germany under the\nCreative Commons Attribution 4.0 International License (CC BY 4.0). Digital Object Identifier 10.14279/depositonce-24399\n\nEnhancements to P4TG: Histogram-Based RTT\nMonitoring in the Data Plane\nFabian Ihle , Etienne Zink , and Michael Menth\nUniversity of Tuebingen, Chair of Communication Networks\n\nAbstract-Modern traffic generators are essential tools.\nIndex Terms-Data Plane Programming, Network Testing, P4, Traffic Generator",
        );
        assert_eq!(
            metadata.title.as_deref(),
            Some("Enhancements to P4TG: Histogram-Based RTT Monitoring in the Data Plane")
        );
        assert_eq!(
            metadata.author.as_deref(),
            Some("Fabian Ihle and Etienne Zink and Michael Menth")
        );
        assert_eq!(
            metadata.venue.as_deref(),
            Some(
                "1st Workshop on Resilient Networks and Systems (ReNeSys), Sept. 2025, Ilmenau/Germany"
            )
        );
        assert_eq!(metadata.venue_kind.as_deref(), Some("booktitle"));
        assert_eq!(metadata.doi, None);
        assert_eq!(
            metadata.abstract_text.as_deref(),
            Some("Modern traffic generators are essential tools.")
        );
    }

    #[test]
    fn parses_rfc_editor_metadata_from_text() {
        let text = r#"
RFC 1034
### Domain names - concepts and facilities, November 1987

Author:
P. Mockapetris

Stream:
IETF
"#;

        let metadata = parse_rfc_editor_metadata_from_text(text);
        assert_eq!(
            metadata.title.as_deref(),
            Some("Domain names - concepts and facilities")
        );
        assert_eq!(metadata.author.as_deref(), Some("P. Mockapetris"));
        assert_eq!(metadata.month.as_deref(), Some("nov"));
        assert_eq!(metadata.year.as_deref(), Some("1987"));
    }

    #[test]
    fn repairs_incomplete_rfc_entries_from_editor_metadata() {
        let source = r#"
@misc{rfc1034,
  series = {Request for Comments},
  number = {1034},
  howpublished = {RFC 1034},
  publisher = {RFC Editor},
  doi = {10.17487/RFC1034},
  url = {https://www.rfc-editor.org/info/rfc1034},
  author = {},
  title = {Domain names - concepts and facilities},
  pagetotal = {55},
  year = {1987}
}
"#;

        let entry = extract_primary_rfc_entry(source).unwrap();
        let metadata = parse_rfc_editor_metadata_from_text(
            r#"
### Domain names - concepts and facilities, November 1987
Author:
P. Mockapetris
Stream:
IETF
"#,
        );

        let repaired = build_repaired_rfc_bibtex(&entry, &metadata, "1034").unwrap();
        assert!(repaired.contains("@misc{rfc1034,"));
        assert!(repaired.contains("author = {P. Mockapetris}"));
        assert!(repaired.contains("title = {Domain names - concepts and facilities}"));
        assert!(repaired.contains("pagetotal = 55"));
        assert!(repaired.contains("month = nov"));
    }
}
