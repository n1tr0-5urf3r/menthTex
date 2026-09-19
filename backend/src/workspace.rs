use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::extract::{Multipart, Path as AxumPath, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::{FromRow, PgPool};
use tokio::fs;

use crate::bibtex::{
    DefinitionCollection, FormatOptions, create_definition, format_entries,
    format_string_definition, parse_bibtex_document,
};
use crate::{
    ApiError, AppState, AuthProviderOption, auth_provider_options,
    build_partial_bibtex_from_pdf_metadata, extract_pdf_metadata, fetch_bibtex_for_doi,
    format_bibtex_input, random_token, strip_wrapping,
};

#[derive(Clone, Debug)]
pub(crate) struct AuthenticatedUser {
    pub(crate) user_id: String,
    pub(crate) email: String,
    pub(crate) username: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) avatar_url: Option<String>,
}

pub(crate) struct ExternalAuthUser {
    pub(crate) provider: &'static str,
    pub(crate) provider_subject: String,
    pub(crate) email: String,
    pub(crate) username: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct WorkspaceMeResponse {
    enabled: bool,
    authenticated: bool,
    providers: Vec<AuthProviderOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<UserSummaryResponse>,
}

#[derive(Debug, Serialize)]
struct UserSummaryResponse {
    id: String,
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
struct ProjectSummaryResponse {
    id: String,
    name: String,
    description: String,
    visibility: String,
    role: String,
    owner_user_id: String,
    owner_email: String,
    document_count: i64,
    member_count: i64,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct FolderResponse {
    id: String,
    project_id: String,
    parent_folder_id: Option<String>,
    name: String,
    is_hidden: bool,
}

#[derive(Debug, Serialize, Clone)]
struct TagResponse {
    id: String,
    name: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f32>,
}

#[derive(Debug, Serialize, FromRow)]
struct ProjectTagResponse {
    id: String,
    name: String,
    usage_count: i64,
}

#[derive(Debug, Serialize, Clone)]
struct DocumentListItemResponse {
    id: String,
    project_id: String,
    folder_id: String,
    original_filename: String,
    title: Option<String>,
    authors_text: Option<String>,
    year: Option<String>,
    doi: Option<String>,
    citation_key: Option<String>,
    processing_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    processing_error: Option<String>,
    tags: Vec<TagResponse>,
}

#[derive(Debug, Serialize)]
struct DocumentDetailResponse {
    id: String,
    project_id: String,
    folder_id: String,
    original_filename: String,
    title: Option<String>,
    authors_text: Option<String>,
    year: Option<String>,
    doi: Option<String>,
    abstract_text: Option<String>,
    journal: Option<String>,
    booktitle: Option<String>,
    citation_key: Option<String>,
    processing_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    processing_error: Option<String>,
    bibtex_source: Option<String>,
    bibtex_formatted: Option<String>,
    source_metadata_json: Value,
    tags: Vec<TagResponse>,
    venue_reviews: Vec<VenueReviewResponse>,
    notes: Vec<NoteResponse>,
    duplicate_warnings: Vec<String>,
    enrichment_warnings: Vec<String>,
    file_url: String,
}

#[derive(Debug, Serialize, FromRow)]
struct NoteResponse {
    id: String,
    document_id: String,
    author_user_id: String,
    author_email: String,
    author_display_name: Option<String>,
    content: String,
}

#[derive(Debug, Serialize, FromRow)]
struct ProjectMemberResponse {
    user_id: String,
    email: String,
    username: Option<String>,
    display_name: Option<String>,
    avatar_url: Option<String>,
    role: String,
}

#[derive(Debug, Serialize)]
struct ProjectTreeResponse {
    project: ProjectSummaryResponse,
    folders: Vec<FolderResponse>,
    documents: Vec<DocumentListItemResponse>,
}

#[derive(Debug, Serialize)]
struct ProjectListResponse {
    projects: Vec<ProjectSummaryResponse>,
}

#[derive(Debug, Serialize)]
struct MembersResponse {
    members: Vec<ProjectMemberResponse>,
}

#[derive(Debug, Serialize)]
struct NotesResponse {
    notes: Vec<NoteResponse>,
}

#[derive(Debug, Serialize)]
struct ProjectTagsResponse {
    tags: Vec<ProjectTagResponse>,
}

#[derive(Debug, Serialize)]
struct UserSearchResponse {
    users: Vec<UserSummaryResponse>,
}

#[derive(Debug, Deserialize)]
struct CreateProjectRequest {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_project_visibility")]
    visibility: String,
}

#[derive(Debug, Deserialize)]
struct UpdateProjectRequest {
    name: Option<String>,
    description: Option<String>,
    visibility: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateFolderRequest {
    parent_folder_id: Option<String>,
    name: String,
}

#[derive(Debug, Deserialize)]
struct UpdateFolderRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AddMemberRequest {
    user_id: String,
    role: String,
}

#[derive(Debug, Deserialize)]
struct UpdateMemberRequest {
    role: String,
}

#[derive(Debug, Deserialize)]
struct UpdateDocumentRequest {
    folder_id: Option<String>,
    title: Option<String>,
    authors_text: Option<String>,
    year: Option<String>,
    doi: Option<String>,
    abstract_text: Option<String>,
    journal: Option<String>,
    booktitle: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct VenueSuggestionResponse {
    token: String,
    raw_value: String,
    expanded_value: String,
    definition_text: String,
    score: u16,
}

#[derive(Debug, Serialize, Clone)]
struct VenueDefinitionResponse {
    token: String,
    raw_value: String,
    expanded_value: String,
    definition_text: String,
}

#[derive(Debug, Serialize, Clone)]
struct VenueReviewResponse {
    field_name: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    matched_definition: Option<VenueDefinitionResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proposed_definition: Option<VenueDefinitionResponse>,
    suggestions: Vec<VenueSuggestionResponse>,
}

#[derive(Debug, Deserialize)]
struct CreateNoteRequest {
    content: String,
}

#[derive(Debug, Deserialize)]
struct UpdateNoteRequest {
    content: String,
}

#[derive(Debug, Deserialize)]
struct AssignTagsRequest {
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UserSearchQuery {
    query: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DocumentListQuery {
    folder_id: Option<String>,
    search: Option<String>,
    tag: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ProjectRole {
    Viewer,
    Editor,
    Owner,
}

#[derive(Debug, FromRow)]
struct SessionUserRow {
    user_id: String,
    email: String,
    username: Option<String>,
    display_name: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Debug, FromRow)]
struct ProjectAccessRow {
    role: Option<String>,
    visibility: String,
}

#[allow(dead_code)]
#[derive(Debug, FromRow, Clone)]
struct DocumentRow {
    id: String,
    project_id: String,
    folder_id: String,
    original_filename: String,
    storage_path: String,
    mime_type: String,
    size_bytes: i64,
    sha256: String,
    title: Option<String>,
    authors_text: Option<String>,
    year: Option<String>,
    doi: Option<String>,
    abstract_text: Option<String>,
    source_metadata_json: Value,
    bibtex_source: Option<String>,
    bibtex_formatted: Option<String>,
    citation_key: Option<String>,
    processing_status: String,
    processing_error: Option<String>,
}

#[derive(Debug, FromRow, Clone)]
struct DocumentTagRow {
    document_id: String,
    tag_id: String,
    tag_name: String,
    source: String,
    confidence: Option<f32>,
}

#[derive(Debug)]
struct UploadedDocument {
    filename: String,
    folder_id: Option<String>,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct ProcessedDocument {
    title: Option<String>,
    authors_text: Option<String>,
    year: Option<String>,
    doi: Option<String>,
    abstract_text: Option<String>,
    citation_key: Option<String>,
    bibtex_source: Option<String>,
    bibtex_formatted: Option<String>,
    source_metadata_json: Value,
    processing_status: String,
    processing_error: Option<String>,
    auto_tags: Vec<(String, Option<f32>)>,
}

#[derive(Debug)]
struct RebuiltDocumentBibtex {
    source: String,
    formatted: String,
    citation_key: Option<String>,
}

#[derive(Debug)]
struct BibtexSummary {
    citation_key: Option<String>,
    title: Option<String>,
    authors_text: Option<String>,
    year: Option<String>,
    abstract_text: Option<String>,
    keywords: Vec<String>,
}

#[derive(Debug, Clone)]
struct ExternalMetadataCandidate {
    source: &'static str,
    title: String,
    authors_text: Option<String>,
    year: Option<String>,
    doi: Option<String>,
    abstract_text: Option<String>,
    venue: Option<String>,
    venue_kind: Option<String>,
    url: Option<String>,
    confidence: f32,
}

#[derive(Debug, Default)]
struct TitleLookupResult {
    applied: Option<ExternalMetadataCandidate>,
    suggestion: Option<ExternalMetadataCandidate>,
    warnings: Vec<String>,
    attempts: Vec<Value>,
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/me", get(me))
        .route("/api/users", get(search_users))
        .route("/api/projects", get(list_projects).post(create_project))
        .route(
            "/api/projects/{project_id}",
            get(get_project)
                .patch(update_project)
                .delete(delete_project),
        )
        .route("/api/projects/{project_id}/tree", get(get_project_tree))
        .route("/api/projects/{project_id}/folders", post(create_folder))
        .route(
            "/api/folders/{folder_id}",
            patch(update_folder).delete(delete_folder),
        )
        .route(
            "/api/projects/{project_id}/members",
            get(list_members).post(add_member),
        )
        .route(
            "/api/projects/{project_id}/members/{user_id}",
            patch(update_member).delete(remove_member),
        )
        .route(
            "/api/projects/{project_id}/documents",
            get(list_documents).post(upload_document),
        )
        .route(
            "/api/documents/{document_id}",
            get(get_document)
                .patch(update_document)
                .delete(delete_document),
        )
        .route(
            "/api/documents/{document_id}/file",
            get(download_document_file),
        )
        .route(
            "/api/documents/{document_id}/reprocess",
            post(reprocess_document),
        )
        .route(
            "/api/documents/{document_id}/notes",
            get(list_notes).post(create_note),
        )
        .route(
            "/api/notes/{note_id}",
            patch(update_note).delete(delete_note),
        )
        .route("/api/projects/{project_id}/tags", get(list_project_tags))
        .route("/api/documents/{document_id}/tags", post(assign_tags))
        .route(
            "/api/documents/{document_id}/tags/{tag_id}",
            delete(remove_tag),
        )
        .route(
            "/api/projects/{project_id}/export/bibtex",
            get(export_project_bibtex),
        )
}

pub(crate) async fn connect_database(database_url: &str) -> Result<PgPool, String> {
    let mut last_error = None;
    for _ in 0..30 {
        match PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
        {
            Ok(pool) => return Ok(pool),
            Err(error) => {
                last_error = Some(error.to_string());
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    Err(format!(
        "Failed to connect to PostgreSQL after multiple attempts: {}",
        last_error.unwrap_or_else(|| "unknown connection error".to_string())
    ))
}

pub(crate) async fn migrate_database(pool: &PgPool) -> Result<(), String> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|error| format!("Failed to apply database migrations: {error}"))
}

pub(crate) async fn ensure_storage_root(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).await.map_err(|error| {
        format!(
            "Failed to create storage root '{}': {error}",
            path.display()
        )
    })
}

pub(crate) async fn upsert_authenticated_user(
    state: &AppState,
    auth_user: &ExternalAuthUser,
) -> Result<AuthenticatedUser, ApiError> {
    if let Some(row) = sqlx::query_as::<_, SessionUserRow>(
        r#"
        SELECT
            users.id AS user_id,
            users.email,
            users.username,
            users.display_name,
            users.avatar_url
        FROM user_identities
        JOIN users ON users.id = user_identities.user_id
        WHERE user_identities.provider = $1
          AND user_identities.provider_subject = $2
        "#,
    )
    .bind(auth_user.provider)
    .bind(&auth_user.provider_subject)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load user identity: {error}")))?
    {
        let row = sqlx::query_as::<_, SessionUserRow>(
            r#"
            UPDATE users
            SET
                email = $2,
                username = $3,
                display_name = $4,
                avatar_url = $5,
                updated_at = NOW(),
                last_login_at = NOW()
            WHERE id = $1
            RETURNING
                id AS user_id,
                email,
                username,
                display_name,
                avatar_url
            "#,
        )
        .bind(&row.user_id)
        .bind(&auth_user.email)
        .bind(auth_user.username.as_deref())
        .bind(auth_user.display_name.as_deref())
        .bind(auth_user.avatar_url.as_deref())
        .fetch_one(&state.db_pool)
        .await
        .map_err(|error| ApiError::internal(format!("Failed to update user record: {error}")))?;

        return Ok(AuthenticatedUser {
            user_id: row.user_id,
            email: row.email,
            username: row.username,
            display_name: row.display_name,
            avatar_url: row.avatar_url,
        });
    }

    let generated_id = random_token(24);
    let gitlab_user_id = if auth_user.provider == "gitlab" {
        auth_user.provider_subject.parse::<i64>().ok()
    } else {
        None
    };
    let row = sqlx::query_as::<_, SessionUserRow>(
        r#"
        INSERT INTO users (id, gitlab_user_id, email, username, display_name, avatar_url, updated_at, last_login_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
        ON CONFLICT (email)
        DO UPDATE SET
            gitlab_user_id = COALESCE(users.gitlab_user_id, EXCLUDED.gitlab_user_id),
            username = EXCLUDED.username,
            display_name = EXCLUDED.display_name,
            avatar_url = EXCLUDED.avatar_url,
            updated_at = NOW(),
            last_login_at = NOW()
        RETURNING
            id AS user_id,
            email,
            username,
            display_name,
            avatar_url
        "#,
    )
    .bind(generated_id)
    .bind(gitlab_user_id)
    .bind(&auth_user.email)
    .bind(auth_user.username.as_deref())
    .bind(auth_user.display_name.as_deref())
    .bind(auth_user.avatar_url.as_deref())
    .fetch_one(&state.db_pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to upsert user record: {error}")))?;

    sqlx::query(
        r#"
        INSERT INTO user_identities (provider, provider_subject, user_id, updated_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (provider, provider_subject)
        DO UPDATE SET
            user_id = EXCLUDED.user_id,
            updated_at = NOW()
        "#,
    )
    .bind(auth_user.provider)
    .bind(&auth_user.provider_subject)
    .bind(&row.user_id)
    .execute(&state.db_pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to persist user identity: {error}")))?;

    Ok(AuthenticatedUser {
        user_id: row.user_id,
        email: row.email,
        username: row.username,
        display_name: row.display_name,
        avatar_url: row.avatar_url,
    })
}

pub(crate) async fn create_db_session(
    state: &AppState,
    user_id: &str,
    expires_in: Duration,
) -> Result<String, ApiError> {
    let session_id = random_token(48);
    sqlx::query(
        r#"
        INSERT INTO sessions (id, user_id, expires_at)
        VALUES ($1, $2, NOW() + ($3 || ' seconds')::interval)
        "#,
    )
    .bind(&session_id)
    .bind(user_id)
    .bind(expires_in.as_secs() as i64)
    .execute(&state.db_pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to persist auth session: {error}")))?;

    sqlx::query("DELETE FROM sessions WHERE expires_at <= NOW()")
        .execute(&state.db_pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to clean expired sessions: {error}"))
        })?;

    Ok(session_id)
}

pub(crate) async fn delete_db_session(state: &AppState, session_id: &str) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(session_id)
        .execute(&state.db_pool)
        .await
        .map_err(|error| ApiError::internal(format!("Failed to delete auth session: {error}")))?;
    Ok(())
}

pub(crate) async fn load_session_user(
    state: &AppState,
    session_id: &str,
) -> Result<Option<AuthenticatedUser>, ApiError> {
    let row = sqlx::query_as::<_, SessionUserRow>(
        r#"
        SELECT
            users.id AS user_id,
            users.email,
            users.username,
            users.display_name,
            users.avatar_url
        FROM sessions
        JOIN users ON users.id = sessions.user_id
        WHERE sessions.id = $1
          AND sessions.expires_at > NOW()
        "#,
    )
    .bind(session_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load auth session: {error}")))?;

    Ok(row.map(|row| AuthenticatedUser {
        user_id: row.user_id,
        email: row.email,
        username: row.username,
        display_name: row.display_name,
        avatar_url: row.avatar_url,
    }))
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WorkspaceMeResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let providers = auth_provider_options(&state);
    if providers.is_empty() {
        return Ok(Json(WorkspaceMeResponse {
            enabled: false,
            authenticated: false,
            providers,
            user: None,
        }));
    }

    let user = crate::session_from_headers(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(WorkspaceMeResponse {
        enabled: true,
        authenticated: user.is_some(),
        providers,
        user: user.map(user_summary),
    }))
}

async fn search_users(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<UserSearchQuery>,
) -> Result<Json<UserSearchResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let _user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;

    let normalized = query.query.unwrap_or_default().trim().to_string();
    let pattern = if normalized.is_empty() {
        "%".to_string()
    } else {
        format!("%{}%", normalized.to_lowercase())
    };

    let rows = sqlx::query_as::<_, SessionUserRow>(
        r#"
        SELECT
            id AS user_id,
            email,
            username,
            display_name,
            avatar_url
        FROM users
        WHERE LOWER(email) LIKE $1
           OR LOWER(COALESCE(username, '')) LIKE $1
           OR LOWER(COALESCE(display_name, '')) LIKE $1
        ORDER BY LOWER(COALESCE(display_name, email))
        LIMIT 15
        "#,
    )
    .bind(pattern)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|error| {
        ApiError::internal(format!("Failed to search users: {error}")).into_response()
    })?;

    Ok(Json(UserSearchResponse {
        users: rows
            .into_iter()
            .map(|row| UserSummaryResponse {
                id: row.user_id,
                email: row.email,
                username: row.username,
                display_name: row.display_name,
                avatar_url: row.avatar_url,
            })
            .collect(),
    }))
}

async fn list_projects(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProjectListResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;

    let projects = load_accessible_projects(&state.db_pool, &user.user_id)
        .await
        .map_err(|error| error.into_response())?;

    Ok(Json(ProjectListResponse { projects }))
}

async fn create_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Json<ProjectSummaryResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let name = request.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("Project name must not be empty.").into_response());
    }

    let project_id = random_token(20);
    let root_folder_id = random_token(20);
    let inbox_folder_id = random_token(20);
    let description = request.description.trim().to_string();
    let visibility =
        validate_project_visibility(&request.visibility).map_err(|error| error.into_response())?;
    let mut tx = state.db_pool.begin().await.map_err(|error| {
        ApiError::internal(format!("Failed to begin project creation: {error}")).into_response()
    })?;

    sqlx::query(
        r#"
        INSERT INTO projects (id, name, description, visibility, owner_user_id)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(&project_id)
    .bind(name)
    .bind(&description)
    .bind(visibility)
    .bind(&user.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        ApiError::internal(format!("Failed to create project: {error}")).into_response()
    })?;

    sqlx::query(
        r#"
        INSERT INTO project_memberships (project_id, user_id, role)
        VALUES ($1, $2, 'owner')
        "#,
    )
    .bind(&project_id)
    .bind(&user.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        ApiError::internal(format!("Failed to create owner membership: {error}")).into_response()
    })?;

    sqlx::query(
        r#"
        INSERT INTO folders (id, project_id, parent_folder_id, name, is_hidden, created_by_user_id)
        VALUES ($1, $2, NULL, 'Root', TRUE, $3),
               ($4, $2, $1, 'Root', FALSE, $3)
        "#,
    )
    .bind(&root_folder_id)
    .bind(&project_id)
    .bind(&user.user_id)
    .bind(&inbox_folder_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        ApiError::internal(format!("Failed to create default folders: {error}")).into_response()
    })?;

    tx.commit().await.map_err(|error| {
        ApiError::internal(format!("Failed to commit project creation: {error}")).into_response()
    })?;

    let project = load_project_summary(&state.db_pool, &project_id, &user.user_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(project))
}

async fn get_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
) -> Result<Json<ProjectSummaryResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Viewer,
    )
    .await
    .map_err(|error| error.into_response())?;
    let project = load_project_summary(&state.db_pool, &project_id, &user.user_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(project))
}

async fn update_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectSummaryResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Owner,
    )
    .await
    .map_err(|error| error.into_response())?;

    let current = load_project_summary(&state.db_pool, &project_id, &user.user_id)
        .await
        .map_err(|error| error.into_response())?;
    let next_name = request
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.name);
    let next_description = request
        .description
        .as_deref()
        .map(str::trim)
        .unwrap_or(&current.description);
    let next_visibility = request
        .visibility
        .as_deref()
        .map(validate_project_visibility)
        .transpose()
        .map_err(|error| error.into_response())?
        .unwrap_or(&current.visibility);

    sqlx::query(
        r#"
        UPDATE projects
        SET name = $2, description = $3, visibility = $4, updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(&project_id)
    .bind(next_name)
    .bind(next_description)
    .bind(next_visibility)
    .execute(&state.db_pool)
    .await
    .map_err(|error| {
        ApiError::internal(format!("Failed to update project: {error}")).into_response()
    })?;

    let updated = load_project_summary(&state.db_pool, &project_id, &user.user_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(updated))
}

async fn delete_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Owner,
    )
    .await
    .map_err(|error| error.into_response())?;

    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(&project_id)
        .execute(&state.db_pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to delete project: {error}")).into_response()
        })?;

    let project_storage = state.storage_root.join("projects").join(&project_id);
    let _ = fs::remove_dir_all(project_storage).await;

    Ok(StatusCode::NO_CONTENT)
}

async fn get_project_tree(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
) -> Result<Json<ProjectTreeResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Viewer,
    )
    .await
    .map_err(|error| error.into_response())?;

    let project = load_project_summary(&state.db_pool, &project_id, &user.user_id)
        .await
        .map_err(|error| error.into_response())?;
    let folders = load_project_folders(&state.db_pool, &project_id)
        .await
        .map_err(|error| error.into_response())?;
    let documents = load_project_documents(&state.db_pool, &project_id, None, None, None)
        .await
        .map_err(|error| error.into_response())?;

    Ok(Json(ProjectTreeResponse {
        project,
        folders,
        documents,
    }))
}

async fn create_folder(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<CreateFolderRequest>,
) -> Result<Json<FolderResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    let name = request.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("Folder name must not be empty.").into_response());
    }
    let parent_folder_id = match request.parent_folder_id {
        Some(parent) => {
            validate_folder_in_project(&state.db_pool, &project_id, &parent)
                .await
                .map_err(|error| error.into_response())?;
            parent
        }
        None => find_root_folder_id(&state.db_pool, &project_id)
            .await
            .map_err(|error| error.into_response())?,
    };
    let folder_id = random_token(20);

    sqlx::query(
        r#"
        INSERT INTO folders (id, project_id, parent_folder_id, name, is_hidden, created_by_user_id)
        VALUES ($1, $2, $3, $4, FALSE, $5)
        "#,
    )
    .bind(&folder_id)
    .bind(&project_id)
    .bind(&parent_folder_id)
    .bind(name)
    .bind(&user.user_id)
    .execute(&state.db_pool)
    .await
    .map_err(|error| {
        ApiError::conflict(format!("Failed to create folder: {error}")).into_response()
    })?;

    let folder = sqlx::query_as::<_, FolderResponse>(
        "SELECT id, project_id, parent_folder_id, name, is_hidden FROM folders WHERE id = $1",
    )
    .bind(&folder_id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|error| {
        ApiError::internal(format!("Failed to load created folder: {error}")).into_response()
    })?;
    Ok(Json(folder))
}

async fn update_folder(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(folder_id): AxumPath<String>,
    Json(request): Json<UpdateFolderRequest>,
) -> Result<Json<FolderResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let folder = load_folder(&state.db_pool, &folder_id)
        .await
        .map_err(|error| error.into_response())?;
    if folder.is_hidden {
        return Err(
            ApiError::bad_request("Hidden system folders cannot be renamed.").into_response(),
        );
    }
    require_project_access(
        &state.db_pool,
        &folder.project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    let name = request.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("Folder name must not be empty.").into_response());
    }

    sqlx::query("UPDATE folders SET name = $2, updated_at = NOW() WHERE id = $1")
        .bind(&folder_id)
        .bind(name)
        .execute(&state.db_pool)
        .await
        .map_err(|error| {
            ApiError::conflict(format!("Failed to rename folder: {error}")).into_response()
        })?;

    let updated = load_folder(&state.db_pool, &folder_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(updated))
}

async fn delete_folder(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(folder_id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let folder = load_folder(&state.db_pool, &folder_id)
        .await
        .map_err(|error| error.into_response())?;
    if folder.is_hidden {
        return Err(
            ApiError::bad_request("Hidden system folders cannot be deleted.").into_response(),
        );
    }
    require_project_access(
        &state.db_pool,
        &folder.project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    let child_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM folders WHERE parent_folder_id = $1")
            .bind(&folder_id)
            .fetch_one(&state.db_pool)
            .await
            .map_err(|error| {
                ApiError::internal(format!("Failed to inspect child folders: {error}"))
                    .into_response()
            })?;
    let document_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM documents WHERE folder_id = $1")
            .bind(&folder_id)
            .fetch_one(&state.db_pool)
            .await
            .map_err(|error| {
                ApiError::internal(format!("Failed to inspect folder documents: {error}"))
                    .into_response()
            })?;
    if child_count > 0 || document_count > 0 {
        return Err(ApiError::conflict("Folder must be empty before deletion.").into_response());
    }

    sqlx::query("DELETE FROM folders WHERE id = $1")
        .bind(&folder_id)
        .execute(&state.db_pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to delete folder: {error}")).into_response()
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
) -> Result<Json<MembersResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Viewer,
    )
    .await
    .map_err(|error| error.into_response())?;

    let members = load_project_members(&state.db_pool, &project_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(MembersResponse { members }))
}

async fn add_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<AddMemberRequest>,
) -> Result<Json<ProjectMemberResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Owner,
    )
    .await
    .map_err(|error| error.into_response())?;
    let role = validate_share_role(&request.role).map_err(|error| error.into_response())?;

    let target_user_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
            .bind(&request.user_id)
            .fetch_one(&state.db_pool)
            .await
            .map_err(|error| {
                ApiError::internal(format!("Failed to validate target user: {error}"))
                    .into_response()
            })?;
    if !target_user_exists {
        return Err(
            ApiError::not_found("The selected user has not signed in yet.").into_response(),
        );
    }

    sqlx::query(
        r#"
        INSERT INTO project_memberships (project_id, user_id, role)
        VALUES ($1, $2, $3)
        ON CONFLICT (project_id, user_id)
        DO UPDATE SET role = EXCLUDED.role, updated_at = NOW()
        "#,
    )
    .bind(&project_id)
    .bind(&request.user_id)
    .bind(role)
    .execute(&state.db_pool)
    .await
    .map_err(|error| {
        ApiError::internal(format!("Failed to add project member: {error}")).into_response()
    })?;

    let member = load_project_member(&state.db_pool, &project_id, &request.user_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(member))
}

async fn update_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((project_id, user_id)): AxumPath<(String, String)>,
    Json(request): Json<UpdateMemberRequest>,
) -> Result<Json<ProjectMemberResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Owner,
    )
    .await
    .map_err(|error| error.into_response())?;
    let role = validate_share_role(&request.role).map_err(|error| error.into_response())?;

    let member = load_project_member(&state.db_pool, &project_id, &user_id)
        .await
        .map_err(|error| error.into_response())?;
    if member.role == "owner" {
        return Err(ApiError::bad_request(
            "The owner role cannot be reassigned through this endpoint.",
        )
        .into_response());
    }

    sqlx::query("UPDATE project_memberships SET role = $3, updated_at = NOW() WHERE project_id = $1 AND user_id = $2")
        .bind(&project_id)
        .bind(&user_id)
        .bind(role)
        .execute(&state.db_pool)
        .await
        .map_err(|error| ApiError::internal(format!("Failed to update project membership: {error}")).into_response())?;

    let updated = load_project_member(&state.db_pool, &project_id, &user_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(updated))
}

async fn remove_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((project_id, user_id)): AxumPath<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Owner,
    )
    .await
    .map_err(|error| error.into_response())?;

    let member = load_project_member(&state.db_pool, &project_id, &user_id)
        .await
        .map_err(|error| error.into_response())?;
    if member.role == "owner" {
        return Err(ApiError::bad_request("The project owner cannot be removed.").into_response());
    }

    sqlx::query("DELETE FROM project_memberships WHERE project_id = $1 AND user_id = $2")
        .bind(&project_id)
        .bind(&user_id)
        .execute(&state.db_pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to remove member: {error}")).into_response()
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_documents(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
    Query(query): Query<DocumentListQuery>,
) -> Result<Json<Vec<DocumentListItemResponse>>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Viewer,
    )
    .await
    .map_err(|error| error.into_response())?;

    let documents = load_project_documents(
        &state.db_pool,
        &project_id,
        query.folder_id.as_deref(),
        query.search.as_deref(),
        query.tag.as_deref(),
    )
    .await
    .map_err(|error| error.into_response())?;
    Ok(Json(documents))
}

async fn upload_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
    mut multipart: Multipart,
) -> Result<Json<DocumentDetailResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    let upload = read_document_upload(&mut multipart)
        .await
        .map_err(|error| error.into_response())?;
    let folder_id = match upload.folder_id.clone() {
        Some(folder_id) => {
            validate_folder_in_project(&state.db_pool, &project_id, &folder_id)
                .await
                .map_err(|error| error.into_response())?;
            folder_id
        }
        None => find_inbox_folder_id(&state.db_pool, &project_id)
            .await
            .map_err(|error| error.into_response())?,
    };

    let document_id = random_token(24);
    let sanitized_filename = sanitize_filename(&upload.filename);
    let project_storage = state
        .storage_root
        .join("projects")
        .join(&project_id)
        .join(&document_id);
    fs::create_dir_all(&project_storage)
        .await
        .map_err(|error| {
            ApiError::internal(format!(
                "Failed to create document storage directory: {error}"
            ))
            .into_response()
        })?;
    let stored_path = project_storage.join(&sanitized_filename);
    fs::write(&stored_path, &upload.bytes)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to persist uploaded document: {error}"))
                .into_response()
        })?;

    let sha256 = sha256_hex(&upload.bytes);
    let processed = process_stored_document(&state, &stored_path, &upload.filename)
        .await
        .map_err(|error| error.into_response())?;

    sqlx::query(
        r#"
        INSERT INTO documents (
            id, project_id, folder_id, uploaded_by_user_id, original_filename, storage_path, mime_type,
            size_bytes, sha256, title, authors_text, year, doi, abstract_text,
            source_metadata_json, bibtex_source, bibtex_formatted, citation_key,
            processing_status, processing_error
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, 'application/pdf',
            $7, $8, $9, $10, $11, $12, $13,
            $14, $15, $16, $17,
            $18, $19
        )
        "#,
    )
    .bind(&document_id)
    .bind(&project_id)
    .bind(&folder_id)
    .bind(&user.user_id)
    .bind(&upload.filename)
    .bind(stored_path.to_string_lossy().to_string())
    .bind(upload.bytes.len() as i64)
    .bind(&sha256)
    .bind(processed.title.as_deref())
    .bind(processed.authors_text.as_deref())
    .bind(processed.year.as_deref())
    .bind(processed.doi.as_deref())
    .bind(processed.abstract_text.as_deref())
    .bind(&processed.source_metadata_json)
    .bind(processed.bibtex_source.as_deref())
    .bind(processed.bibtex_formatted.as_deref())
    .bind(processed.citation_key.as_deref())
    .bind(&processed.processing_status)
    .bind(processed.processing_error.as_deref())
    .execute(&state.db_pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to store document metadata: {error}")).into_response())?;

    sync_document_tags(
        &state.db_pool,
        &project_id,
        &document_id,
        &user.user_id,
        "auto",
        &processed.auto_tags,
        true,
    )
    .await
    .map_err(|error| error.into_response())?;

    let document = build_document_detail_response(&state, &project_id, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(document))
}

async fn get_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(document_id): AxumPath<String>,
) -> Result<Json<DocumentDetailResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let document = load_document(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &document.project_id,
        &user.user_id,
        ProjectRole::Viewer,
    )
    .await
    .map_err(|error| error.into_response())?;
    let detail = build_document_detail_response(&state, &document.project_id, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(detail))
}

async fn update_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(document_id): AxumPath<String>,
    Json(request): Json<UpdateDocumentRequest>,
) -> Result<Json<DocumentDetailResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let current = load_document(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &current.project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    if let Some(folder_id) = request.folder_id.as_deref() {
        validate_folder_in_project(&state.db_pool, &current.project_id, folder_id)
            .await
            .map_err(|error| error.into_response())?;
    }

    let next_title = request.title.as_deref().map(str::trim);
    let next_authors = request.authors_text.as_deref().map(str::trim);
    let next_year = request.year.as_deref().map(str::trim);
    let next_doi = request.doi.as_deref().map(str::trim);
    let next_abstract = request.abstract_text.as_deref().map(str::trim);
    let next_journal = request.journal.as_deref().map(str::trim);
    let next_booktitle = request.booktitle.as_deref().map(str::trim);

    let updated_bibtex = rebuild_document_bibtex(
        &state,
        &current,
        next_title,
        next_authors,
        next_year,
        next_doi,
        next_abstract,
        next_journal,
        next_booktitle,
    )
    .map_err(|error| error.into_response())?;

    sqlx::query(
        r#"
        UPDATE documents
        SET
            folder_id = COALESCE($2, folder_id),
            title = $3,
            authors_text = $4,
            year = $5,
            doi = NULLIF($6, ''),
            abstract_text = $7,
            bibtex_source = $8,
            bibtex_formatted = $9,
            citation_key = $10,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(&document_id)
    .bind(request.folder_id.as_deref())
    .bind(next_title)
    .bind(next_authors)
    .bind(next_year)
    .bind(next_doi)
    .bind(next_abstract)
    .bind(updated_bibtex.source.as_str())
    .bind(updated_bibtex.formatted.as_str())
    .bind(updated_bibtex.citation_key.as_deref())
    .execute(&state.db_pool)
    .await
    .map_err(|error| {
        ApiError::internal(format!("Failed to update document metadata: {error}")).into_response()
    })?;

    let updated = build_document_detail_response(&state, &current.project_id, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(updated))
}

async fn delete_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(document_id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let current = load_document(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &current.project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    sqlx::query("DELETE FROM documents WHERE id = $1")
        .bind(&document_id)
        .execute(&state.db_pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to delete document: {error}")).into_response()
        })?;

    cleanup_orphan_project_tags(&state.db_pool, &current.project_id)
        .await
        .map_err(|error| error.into_response())?;

    if let Some(parent_dir) = PathBuf::from(&current.storage_path).parent() {
        let _ = fs::remove_dir_all(parent_dir).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn download_document_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(document_id): AxumPath<String>,
) -> Result<Response, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let document = load_document(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &document.project_id,
        &user.user_id,
        ProjectRole::Viewer,
    )
    .await
    .map_err(|error| error.into_response())?;

    let bytes = fs::read(&document.storage_path).await.map_err(|error| {
        ApiError::internal(format!("Failed to read document file: {error}")).into_response()
    })?;
    let mut response = bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "inline; filename=\"{}\"",
            sanitize_header_filename(&document.original_filename)
        ))
        .map_err(|error| {
            ApiError::internal(format!("Failed to set file download header: {error}"))
                .into_response()
        })?,
    );
    Ok(response)
}

async fn reprocess_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(document_id): AxumPath<String>,
) -> Result<Json<DocumentDetailResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let current = load_document(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &current.project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    let processed = reprocess_existing_document(&state, &current)
        .await
        .map_err(|error| error.into_response())?;

    sqlx::query(
        r#"
        UPDATE documents
        SET
            title = $2,
            authors_text = $3,
            year = $4,
            doi = $5,
            abstract_text = $6,
            source_metadata_json = $7,
            bibtex_source = $8,
            bibtex_formatted = $9,
            citation_key = $10,
            processing_status = $11,
            processing_error = $12,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(&document_id)
    .bind(processed.title.as_deref())
    .bind(processed.authors_text.as_deref())
    .bind(processed.year.as_deref())
    .bind(processed.doi.as_deref())
    .bind(processed.abstract_text.as_deref())
    .bind(&processed.source_metadata_json)
    .bind(processed.bibtex_source.as_deref())
    .bind(processed.bibtex_formatted.as_deref())
    .bind(processed.citation_key.as_deref())
    .bind(&processed.processing_status)
    .bind(processed.processing_error.as_deref())
    .execute(&state.db_pool)
    .await
    .map_err(|error| {
        ApiError::internal(format!(
            "Failed to update document after reprocessing: {error}"
        ))
        .into_response()
    })?;

    sync_document_tags(
        &state.db_pool,
        &current.project_id,
        &document_id,
        &user.user_id,
        "auto",
        &processed.auto_tags,
        true,
    )
    .await
    .map_err(|error| error.into_response())?;

    let detail = build_document_detail_response(&state, &current.project_id, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(detail))
}

async fn list_notes(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(document_id): AxumPath<String>,
) -> Result<Json<NotesResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let document = load_document(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &document.project_id,
        &user.user_id,
        ProjectRole::Viewer,
    )
    .await
    .map_err(|error| error.into_response())?;

    let notes = load_notes(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(NotesResponse { notes }))
}

async fn create_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(document_id): AxumPath<String>,
    Json(request): Json<CreateNoteRequest>,
) -> Result<Json<NoteResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let document = load_document(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &document.project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    let content = request.content.trim();
    if content.is_empty() {
        return Err(ApiError::bad_request("Note content must not be empty.").into_response());
    }

    let note_id = random_token(20);
    sqlx::query(
        "INSERT INTO document_notes (id, document_id, author_user_id, content) VALUES ($1, $2, $3, $4)",
    )
    .bind(&note_id)
    .bind(&document_id)
    .bind(&user.user_id)
    .bind(content)
    .execute(&state.db_pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to create note: {error}")).into_response())?;

    let note = load_note(&state.db_pool, &note_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(note))
}

async fn update_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(note_id): AxumPath<String>,
    Json(request): Json<UpdateNoteRequest>,
) -> Result<Json<NoteResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let note = load_note(&state.db_pool, &note_id)
        .await
        .map_err(|error| error.into_response())?;
    let document = load_document(&state.db_pool, &note.document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &document.project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    let content = request.content.trim();
    if content.is_empty() {
        return Err(ApiError::bad_request("Note content must not be empty.").into_response());
    }

    sqlx::query("UPDATE document_notes SET content = $2, updated_at = NOW() WHERE id = $1")
        .bind(&note_id)
        .bind(content)
        .execute(&state.db_pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to update note: {error}")).into_response()
        })?;

    let updated = load_note(&state.db_pool, &note_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(Json(updated))
}

async fn delete_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(note_id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let note = load_note(&state.db_pool, &note_id)
        .await
        .map_err(|error| error.into_response())?;
    let document = load_document(&state.db_pool, &note.document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &document.project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    sqlx::query("DELETE FROM document_notes WHERE id = $1")
        .bind(&note_id)
        .execute(&state.db_pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to delete note: {error}")).into_response()
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_project_tags(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
) -> Result<Json<ProjectTagsResponse>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Viewer,
    )
    .await
    .map_err(|error| error.into_response())?;

    cleanup_orphan_project_tags(&state.db_pool, &project_id)
        .await
        .map_err(|error| error.into_response())?;

    let tags = sqlx::query_as::<_, ProjectTagResponse>(
        r#"
        SELECT
            tags.id,
            tags.name,
            COUNT(document_tags.document_id)::BIGINT AS usage_count
        FROM tags
        JOIN document_tags ON document_tags.tag_id = tags.id
        WHERE tags.project_id = $1
        GROUP BY tags.id, tags.name
        ORDER BY COUNT(document_tags.document_id) DESC, LOWER(tags.name)
        "#,
    )
    .bind(&project_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|error| {
        ApiError::internal(format!("Failed to load project tags: {error}")).into_response()
    })?;
    Ok(Json(ProjectTagsResponse { tags }))
}

async fn assign_tags(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(document_id): AxumPath<String>,
    Json(request): Json<AssignTagsRequest>,
) -> Result<Json<Vec<TagResponse>>, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let document = load_document(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &document.project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    let normalized = request
        .tags
        .iter()
        .filter_map(|tag| normalize_tag_name(tag))
        .take(16)
        .map(|name| (name, None))
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return Err(ApiError::bad_request("At least one valid tag is required.").into_response());
    }

    sync_document_tags(
        &state.db_pool,
        &document.project_id,
        &document_id,
        &user.user_id,
        "manual",
        &normalized,
        false,
    )
    .await
    .map_err(|error| error.into_response())?;

    let tags = load_document_tags(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?
        .iter()
        .map(tag_from_row)
        .collect();
    Ok(Json(tags))
}

async fn remove_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((document_id, tag_id)): AxumPath<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    let document = load_document(&state.db_pool, &document_id)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &document.project_id,
        &user.user_id,
        ProjectRole::Editor,
    )
    .await
    .map_err(|error| error.into_response())?;

    sqlx::query("DELETE FROM document_tags WHERE document_id = $1 AND tag_id = $2")
        .bind(&document_id)
        .bind(&tag_id)
        .execute(&state.db_pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to remove tag from document: {error}"))
                .into_response()
        })?;

    cleanup_orphan_project_tags(&state.db_pool, &document.project_id)
        .await
        .map_err(|error| error.into_response())?;
    Ok(StatusCode::NO_CONTENT)
}

async fn export_project_bibtex(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
) -> Result<Response, (StatusCode, Json<crate::ErrorResponse>)> {
    let user = crate::require_authenticated_user(&state, &headers)
        .await
        .map_err(|error| error.into_response())?;
    require_project_access(
        &state.db_pool,
        &project_id,
        &user.user_id,
        ProjectRole::Viewer,
    )
    .await
    .map_err(|error| error.into_response())?;

    let entries: Vec<Option<String>> = sqlx::query_scalar(
        r#"
        SELECT bibtex_formatted
        FROM documents
        WHERE project_id = $1
          AND processing_status = 'ready'
          AND bibtex_formatted IS NOT NULL
        ORDER BY LOWER(COALESCE(authors_text, original_filename)), LOWER(COALESCE(title, original_filename))
        "#,
    )
    .bind(&project_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to export project BibTeX: {error}")).into_response())?;

    let entry_blocks = entries
        .into_iter()
        .flatten()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();

    let mut blocks = Vec::new();
    if let Ok(definitions) = crate::load_definitions(&state) {
        let used_tokens = collect_used_definition_tokens(&definitions, &entry_blocks);
        let used_definitions = definitions
            .definitions()
            .iter()
            .filter(|definition| used_tokens.contains(&definition.token.to_ascii_uppercase()))
            .cloned()
            .collect::<Vec<_>>();
        let rendered = crate::bibtex::format_definition_source(&used_definitions);
        if !rendered.trim().is_empty() {
            blocks.push(rendered.trim().to_string());
        }
    }

    blocks.extend(entry_blocks);
    let body = blocks.join("\n\n");
    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"literature.bib\""),
    );
    Ok(response)
}

async fn load_accessible_projects(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<ProjectSummaryResponse>, ApiError> {
    sqlx::query_as::<_, ProjectSummaryResponse>(
        r#"
        SELECT
            projects.id,
            projects.name,
            projects.description,
            projects.visibility,
            COALESCE(current_membership.role, 'viewer') AS role,
            projects.owner_user_id,
            owner.email AS owner_email,
            COUNT(DISTINCT documents.id)::BIGINT AS document_count,
            COUNT(DISTINCT members.user_id)::BIGINT AS member_count
        FROM projects
        LEFT JOIN project_memberships current_membership
            ON current_membership.project_id = projects.id
           AND current_membership.user_id = $1
        JOIN users owner ON owner.id = projects.owner_user_id
        LEFT JOIN documents ON documents.project_id = projects.id
        LEFT JOIN project_memberships members ON members.project_id = projects.id
        WHERE projects.visibility = 'public'
           OR current_membership.user_id IS NOT NULL
        GROUP BY projects.id, projects.visibility, current_membership.role, owner.email
        ORDER BY LOWER(projects.name)
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load accessible projects: {error}")))
}

async fn load_project_summary(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
) -> Result<ProjectSummaryResponse, ApiError> {
    sqlx::query_as::<_, ProjectSummaryResponse>(
        r#"
        SELECT
            projects.id,
            projects.name,
            projects.description,
            projects.visibility,
            COALESCE(current_membership.role, 'viewer') AS role,
            projects.owner_user_id,
            owner.email AS owner_email,
            COUNT(DISTINCT documents.id)::BIGINT AS document_count,
            COUNT(DISTINCT members.user_id)::BIGINT AS member_count
        FROM projects
        LEFT JOIN project_memberships current_membership
            ON current_membership.project_id = projects.id
           AND current_membership.user_id = $2
        JOIN users owner ON owner.id = projects.owner_user_id
        LEFT JOIN documents ON documents.project_id = projects.id
        LEFT JOIN project_memberships members ON members.project_id = projects.id
        WHERE projects.id = $1
          AND (projects.visibility = 'public' OR current_membership.user_id IS NOT NULL)
        GROUP BY projects.id, projects.visibility, current_membership.role, owner.email
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load project summary: {error}")))?
    .ok_or_else(|| ApiError::not_found("Project not found."))
}

async fn require_project_access(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    minimum_role: ProjectRole,
) -> Result<ProjectRole, ApiError> {
    let row = sqlx::query_as::<_, ProjectAccessRow>(
        r#"
        SELECT
            project_memberships.role,
            projects.visibility
        FROM projects
        LEFT JOIN project_memberships
            ON project_memberships.project_id = projects.id
           AND project_memberships.user_id = $2
        WHERE projects.id = $1
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to check project access: {error}")))?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Project not found."));
    };

    let role = row
        .role
        .as_deref()
        .and_then(parse_project_role)
        .or_else(|| {
            if minimum_role <= ProjectRole::Viewer && row.visibility == "public" {
                Some(ProjectRole::Viewer)
            } else {
                None
            }
        })
        .ok_or_else(|| {
            if minimum_role <= ProjectRole::Viewer {
                ApiError::unauthorized("You do not have access to this project.")
            } else {
                ApiError::unauthorized("You do not have permission to perform this action.")
            }
        })?;
    if role < minimum_role {
        return Err(ApiError::unauthorized(
            "You do not have permission to perform this action.",
        ));
    }

    Ok(role)
}

async fn load_project_folders(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<FolderResponse>, ApiError> {
    sqlx::query_as::<_, FolderResponse>(
        r#"
        SELECT id, project_id, parent_folder_id, name, is_hidden
        FROM folders
        WHERE project_id = $1
        ORDER BY is_hidden ASC, LOWER(name)
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load project folders: {error}")))
}

async fn load_folder(pool: &PgPool, folder_id: &str) -> Result<FolderResponse, ApiError> {
    sqlx::query_as::<_, FolderResponse>(
        "SELECT id, project_id, parent_folder_id, name, is_hidden FROM folders WHERE id = $1",
    )
    .bind(folder_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load folder: {error}")))?
    .ok_or_else(|| ApiError::not_found("Folder not found."))
}

async fn validate_folder_in_project(
    pool: &PgPool,
    project_id: &str,
    folder_id: &str,
) -> Result<(), ApiError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM folders WHERE id = $1 AND project_id = $2)",
    )
    .bind(folder_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to validate folder ownership: {error}")))?;
    if exists {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "Folder does not belong to this project.",
        ))
    }
}

async fn find_root_folder_id(pool: &PgPool, project_id: &str) -> Result<String, ApiError> {
    sqlx::query_scalar(
        "SELECT id FROM folders WHERE project_id = $1 AND is_hidden = TRUE AND parent_folder_id IS NULL LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to locate project root folder: {error}")))?
    .ok_or_else(|| ApiError::internal("The project root folder is missing."))
}

async fn find_inbox_folder_id(pool: &PgPool, project_id: &str) -> Result<String, ApiError> {
    sqlx::query_scalar(
        "SELECT id FROM folders WHERE project_id = $1 AND is_hidden = FALSE ORDER BY created_at ASC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to locate default project folder: {error}")))?
    .ok_or_else(|| ApiError::internal("The project default folder is missing."))
}

async fn load_project_members(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<ProjectMemberResponse>, ApiError> {
    sqlx::query_as::<_, ProjectMemberResponse>(
        r#"
        SELECT
            users.id AS user_id,
            users.email,
            users.username,
            users.display_name,
            users.avatar_url,
            project_memberships.role
        FROM project_memberships
        JOIN users ON users.id = project_memberships.user_id
        WHERE project_memberships.project_id = $1
        ORDER BY
            CASE project_memberships.role
                WHEN 'owner' THEN 0
                WHEN 'editor' THEN 1
                ELSE 2
            END,
            LOWER(COALESCE(users.display_name, users.email))
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load project members: {error}")))
}

async fn load_project_member(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
) -> Result<ProjectMemberResponse, ApiError> {
    sqlx::query_as::<_, ProjectMemberResponse>(
        r#"
        SELECT
            users.id AS user_id,
            users.email,
            users.username,
            users.display_name,
            users.avatar_url,
            project_memberships.role
        FROM project_memberships
        JOIN users ON users.id = project_memberships.user_id
        WHERE project_memberships.project_id = $1
          AND project_memberships.user_id = $2
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load project member: {error}")))?
    .ok_or_else(|| ApiError::not_found("Project member not found."))
}

async fn load_project_documents(
    pool: &PgPool,
    project_id: &str,
    folder_id: Option<&str>,
    search: Option<&str>,
    tag: Option<&str>,
) -> Result<Vec<DocumentListItemResponse>, ApiError> {
    let rows = sqlx::query_as::<_, DocumentRow>(
        r#"
        SELECT
            id, project_id, folder_id, original_filename, storage_path, mime_type,
            size_bytes, sha256, title, authors_text, year, doi, abstract_text,
            source_metadata_json, bibtex_source, bibtex_formatted, citation_key,
            processing_status, processing_error
        FROM documents
        WHERE project_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load project documents: {error}")))?;

    let tags_by_document =
        load_document_tags_map(pool, rows.iter().map(|row| row.id.as_str()).collect()).await?;
    let tag_filter = tag
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    let search_filter = search
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());

    Ok(rows
        .into_iter()
        .filter(|row| {
            folder_id
                .map(|candidate| candidate == row.folder_id)
                .unwrap_or(true)
        })
        .filter(|row| {
            search_filter
                .as_ref()
                .map(|needle| {
                    [
                        row.title.as_deref(),
                        row.authors_text.as_deref(),
                        row.doi.as_deref(),
                        row.citation_key.as_deref(),
                        Some(row.original_filename.as_str()),
                    ]
                    .into_iter()
                    .flatten()
                    .any(|value| value.to_lowercase().contains(needle))
                })
                .unwrap_or(true)
        })
        .filter(|row| {
            tag_filter
                .as_ref()
                .map(|needle| {
                    tags_by_document
                        .get(&row.id)
                        .into_iter()
                        .flat_map(|tags| tags.iter())
                        .any(|candidate| candidate.tag_name.to_lowercase() == *needle)
                })
                .unwrap_or(true)
        })
        .map(|row| document_list_item_from_row(&row, tags_by_document.get(&row.id)))
        .collect())
}

async fn load_document(pool: &PgPool, document_id: &str) -> Result<DocumentRow, ApiError> {
    sqlx::query_as::<_, DocumentRow>(
        r#"
        SELECT
            id, project_id, folder_id, original_filename, storage_path, mime_type,
            size_bytes, sha256, title, authors_text, year, doi, abstract_text,
            source_metadata_json, bibtex_source, bibtex_formatted, citation_key,
            processing_status, processing_error
        FROM documents
        WHERE id = $1
        "#,
    )
    .bind(document_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load document: {error}")))?
    .ok_or_else(|| ApiError::not_found("Document not found."))
}

async fn load_document_tags(
    pool: &PgPool,
    document_id: &str,
) -> Result<Vec<DocumentTagRow>, ApiError> {
    sqlx::query_as::<_, DocumentTagRow>(
        r#"
        SELECT
            document_tags.document_id,
            tags.id AS tag_id,
            tags.name AS tag_name,
            document_tags.source,
            document_tags.confidence
        FROM document_tags
        JOIN tags ON tags.id = document_tags.tag_id
        WHERE document_tags.document_id = $1
        ORDER BY
            CASE document_tags.source WHEN 'manual' THEN 0 ELSE 1 END,
            LOWER(tags.name)
        "#,
    )
    .bind(document_id)
    .fetch_all(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load document tags: {error}")))
}

async fn load_document_tags_map(
    pool: &PgPool,
    document_ids: Vec<&str>,
) -> Result<HashMap<String, Vec<DocumentTagRow>>, ApiError> {
    if document_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, DocumentTagRow>(
        r#"
        SELECT
            document_tags.document_id,
            tags.id AS tag_id,
            tags.name AS tag_name,
            document_tags.source,
            document_tags.confidence
        FROM document_tags
        JOIN tags ON tags.id = document_tags.tag_id
        WHERE document_tags.document_id = ANY($1)
        ORDER BY
            CASE document_tags.source WHEN 'manual' THEN 0 ELSE 1 END,
            LOWER(tags.name)
        "#,
    )
    .bind(&document_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load document tag map: {error}")))?;

    let mut grouped = HashMap::new();
    for row in rows {
        grouped
            .entry(row.document_id.clone())
            .or_insert_with(Vec::new)
            .push(row);
    }
    Ok(grouped)
}

async fn load_notes(pool: &PgPool, document_id: &str) -> Result<Vec<NoteResponse>, ApiError> {
    sqlx::query_as::<_, NoteResponse>(
        r#"
        SELECT
            document_notes.id,
            document_notes.document_id,
            document_notes.author_user_id,
            users.email AS author_email,
            users.display_name AS author_display_name,
            document_notes.content
        FROM document_notes
        JOIN users ON users.id = document_notes.author_user_id
        WHERE document_notes.document_id = $1
        ORDER BY document_notes.updated_at DESC
        "#,
    )
    .bind(document_id)
    .fetch_all(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load document notes: {error}")))
}

async fn load_note(pool: &PgPool, note_id: &str) -> Result<NoteResponse, ApiError> {
    sqlx::query_as::<_, NoteResponse>(
        r#"
        SELECT
            document_notes.id,
            document_notes.document_id,
            document_notes.author_user_id,
            users.email AS author_email,
            users.display_name AS author_display_name,
            document_notes.content
        FROM document_notes
        JOIN users ON users.id = document_notes.author_user_id
        WHERE document_notes.id = $1
        "#,
    )
    .bind(note_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to load note: {error}")))?
    .ok_or_else(|| ApiError::not_found("Note not found."))
}

async fn read_document_upload(multipart: &mut Multipart) -> Result<UploadedDocument, ApiError> {
    let mut file_bytes = None;
    let mut filename = None;
    let mut folder_id = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiError::bad_request(format!("Failed to read document upload: {error}"))
    })? {
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
        } else if field_name == "folder_id" {
            let value = field.text().await.map_err(|error| {
                ApiError::bad_request(format!("Failed to read upload field: {error}"))
            })?;
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                folder_id = Some(trimmed.to_string());
            }
        }
    }

    Ok(UploadedDocument {
        filename: filename.ok_or_else(|| ApiError::bad_request("Please upload one PDF file."))?,
        folder_id,
        bytes: file_bytes.ok_or_else(|| ApiError::bad_request("Please upload one PDF file."))?,
    })
}

async fn process_stored_document(
    state: &AppState,
    stored_path: &Path,
    original_filename: &str,
) -> Result<ProcessedDocument, ApiError> {
    let metadata =
        extract_pdf_metadata(&stored_path.to_path_buf(), Some(original_filename)).await?;
    process_metadata_candidate(state, metadata).await
}

async fn process_metadata_candidate(
    state: &AppState,
    metadata: crate::PdfMetadataCandidate,
) -> Result<ProcessedDocument, ApiError> {
    let trusted_metadata = trusted_local_metadata(&metadata);
    let title_lookup = enrich_metadata_from_title(state, &trusted_metadata).await;

    if let Some(doi) = trusted_metadata.doi.clone() {
        match fetch_bibtex_for_doi(&state.http_client, &doi).await {
            Ok(retrieved_input) => {
                let summary = summarize_bibtex(&retrieved_input);
                if !doi_lookup_matches_local_metadata(&trusted_metadata, &summary) {
                    if let Some(candidate) = title_lookup.applied.as_ref() {
                        return build_processed_document_from_external_candidate(
                            state,
                            &trusted_metadata,
                            candidate,
                            &title_lookup,
                            Some(format!(
                                "Ignored DOI metadata for {} because it did not match the extracted paper title.",
                                doi
                            )),
                        )
                        .await;
                    }

                    if let Some(partial_input) =
                        build_partial_bibtex_from_pdf_metadata(&trusted_metadata)
                    {
                        return build_processed_document_from_partial(
                            state,
                            &trusted_metadata,
                            &partial_input,
                            build_processing_metadata_json(
                                "pdf_metadata_fallback",
                                &trusted_metadata,
                                &title_lookup,
                                None,
                                Some(json!({
                                    "doi_lookup_warning": format!(
                                        "Ignored DOI metadata for {} because it did not match the extracted paper title.",
                                        doi
                                    ),
                                    "doi": doi,
                                })),
                            ),
                        );
                    }
                }

                if bibtex_summary_is_complete(&summary) || title_lookup.applied.is_none() {
                    return build_processed_document_from_bibtex(
                        state,
                        &trusted_metadata,
                        &retrieved_input,
                        Some(doi),
                        build_processing_metadata_json(
                            "doi",
                            &trusted_metadata,
                            &title_lookup,
                            None,
                            None,
                        ),
                        None,
                    );
                }
            }
            Err(error) => {
                if let Some(candidate) = title_lookup.applied.as_ref() {
                    return build_processed_document_from_external_candidate(
                        state,
                        &trusted_metadata,
                        candidate,
                        &title_lookup,
                        Some(format!(
                            "Local DOI lookup failed for {}: {}",
                            doi, error.message
                        )),
                    )
                    .await;
                }

                if let Some(partial_input) =
                    build_partial_bibtex_from_pdf_metadata(&trusted_metadata)
                {
                    return build_processed_document_from_partial(
                        state,
                        &trusted_metadata,
                        &partial_input,
                        build_processing_metadata_json(
                            "pdf_metadata_fallback",
                            &trusted_metadata,
                            &title_lookup,
                            None,
                            Some(json!({
                                "doi_lookup_error": error.message,
                                "doi": trusted_metadata.doi.clone(),
                            })),
                        ),
                    );
                }

                return Ok(build_failed_document(
                    &trusted_metadata,
                    Some(error.message),
                    build_processing_metadata_json(
                        "metadata_failed",
                        &trusted_metadata,
                        &title_lookup,
                        None,
                        None,
                    ),
                ));
            }
        }
    }

    if let Some(candidate) = title_lookup.applied.as_ref() {
        return build_processed_document_from_external_candidate(
            state,
            &trusted_metadata,
            candidate,
            &title_lookup,
            None,
        )
        .await;
    }

    if let Some(partial_input) = build_partial_bibtex_from_pdf_metadata(&trusted_metadata) {
        return build_processed_document_from_partial(
            state,
            &trusted_metadata,
            &partial_input,
            build_processing_metadata_json(
                "pdf_metadata",
                &trusted_metadata,
                &title_lookup,
                None,
                None,
            ),
        );
    }

    Ok(build_failed_document(
        &trusted_metadata,
        Some(
            "No DOI or sufficient PDF metadata was available to build a BibTeX entry.".to_string(),
        ),
        build_processing_metadata_json(
            "metadata_failed",
            &trusted_metadata,
            &title_lookup,
            None,
            None,
        ),
    ))
}

async fn reprocess_existing_document(
    state: &AppState,
    document: &DocumentRow,
) -> Result<ProcessedDocument, ApiError> {
    let current_metadata = extract_pdf_metadata(
        &PathBuf::from(&document.storage_path),
        Some(&document.original_filename),
    )
    .await?;
    let mut candidate = current_metadata.clone();
    if let Some(title) = document
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        candidate.title = Some(title);
    }
    if let Some(authors) = document
        .authors_text
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        candidate.author = Some(authors);
    }
    if let Some(year) = document
        .year
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        candidate.year = Some(year);
    }
    if let Some(doi) = document
        .doi
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        candidate.doi = Some(doi);
    }
    process_metadata_candidate(state, candidate.clone())
        .await
        .or_else(|_| process_manual_document(state, &candidate))
}

fn process_manual_document(
    state: &AppState,
    candidate: &crate::PdfMetadataCandidate,
) -> Result<ProcessedDocument, ApiError> {
    let trusted_candidate = trusted_local_metadata(candidate);
    let partial_input =
        build_partial_bibtex_from_pdf_metadata(&trusted_candidate).ok_or_else(|| {
            ApiError::bad_request(
                "The document does not contain enough metadata to reprocess without a DOI.",
            )
        })?;
    let formatted = format_bibtex_input(state, &partial_input, false, true, true, false)?;
    let summary = summarize_bibtex(&partial_input);
    Ok(ProcessedDocument {
        title: summary
            .as_ref()
            .and_then(|candidate| candidate.title.clone())
            .or(trusted_candidate.title.clone()),
        authors_text: summary
            .as_ref()
            .and_then(|candidate| candidate.authors_text.clone())
            .or(trusted_candidate.author.clone()),
        year: summary
            .as_ref()
            .and_then(|candidate| candidate.year.clone())
            .or(trusted_candidate.year.clone()),
        doi: trusted_candidate.doi.clone(),
        abstract_text: trusted_candidate.abstract_text.clone(),
        citation_key: summary
            .as_ref()
            .and_then(|candidate| candidate.citation_key.clone())
            .or_else(|| formatted.entries.first().map(|entry| entry.key.clone())),
        bibtex_source: Some(partial_input.clone()),
        bibtex_formatted: Some(formatted.output.clone()),
        source_metadata_json: json!({
            "resolution": "manual_reprocess",
            "pdf_metadata": local_metadata_json(&trusted_candidate),
            "enrichment": { "warnings": [] }
        }),
        processing_status: "ready".to_string(),
        processing_error: None,
        auto_tags: build_auto_tags(&[], &trusted_candidate.extracted_text),
    })
}

fn build_processed_document_from_bibtex(
    state: &AppState,
    metadata: &crate::PdfMetadataCandidate,
    retrieved_input: &str,
    resolved_doi: Option<String>,
    source_metadata_json: Value,
    error_override: Option<String>,
) -> Result<ProcessedDocument, ApiError> {
    let summary = summarize_bibtex(retrieved_input);
    let formatted = format_bibtex_input(state, retrieved_input, false, true, true, false)?;
    let keyword_candidates = summary
        .as_ref()
        .map(|candidate| candidate.keywords.clone())
        .unwrap_or_default();

    Ok(ProcessedDocument {
        title: summary
            .as_ref()
            .and_then(|candidate| candidate.title.clone())
            .or(metadata.title.clone()),
        authors_text: summary
            .as_ref()
            .and_then(|candidate| candidate.authors_text.clone())
            .or(metadata.author.clone()),
        year: summary
            .as_ref()
            .and_then(|candidate| candidate.year.clone())
            .or(metadata.year.clone()),
        doi: resolved_doi.or_else(|| metadata.doi.clone()),
        abstract_text: summary
            .as_ref()
            .and_then(|candidate| candidate.abstract_text.clone())
            .or(metadata.abstract_text.clone()),
        citation_key: summary
            .as_ref()
            .and_then(|candidate| candidate.citation_key.clone())
            .or_else(|| formatted.entries.first().map(|entry| entry.key.clone())),
        bibtex_source: Some(retrieved_input.to_string()),
        bibtex_formatted: Some(formatted.output.clone()),
        source_metadata_json,
        processing_status: "ready".to_string(),
        processing_error: error_override,
        auto_tags: build_auto_tags(&keyword_candidates, &metadata.extracted_text),
    })
}

fn build_processed_document_from_partial(
    state: &AppState,
    metadata: &crate::PdfMetadataCandidate,
    partial_input: &str,
    source_metadata_json: Value,
) -> Result<ProcessedDocument, ApiError> {
    let summary = summarize_bibtex(partial_input);
    let formatted = format_bibtex_input(state, partial_input, false, true, true, false)?;
    Ok(ProcessedDocument {
        title: summary
            .as_ref()
            .and_then(|candidate| candidate.title.clone())
            .or(metadata.title.clone()),
        authors_text: summary
            .as_ref()
            .and_then(|candidate| candidate.authors_text.clone())
            .or(metadata.author.clone()),
        year: summary
            .as_ref()
            .and_then(|candidate| candidate.year.clone())
            .or(metadata.year.clone()),
        doi: metadata.doi.clone(),
        abstract_text: summary
            .as_ref()
            .and_then(|candidate| candidate.abstract_text.clone())
            .or(metadata.abstract_text.clone()),
        citation_key: summary
            .as_ref()
            .and_then(|candidate| candidate.citation_key.clone())
            .or_else(|| formatted.entries.first().map(|entry| entry.key.clone())),
        bibtex_source: Some(partial_input.to_string()),
        bibtex_formatted: Some(formatted.output.clone()),
        source_metadata_json,
        processing_status: "ready".to_string(),
        processing_error: None,
        auto_tags: build_auto_tags(&[], &metadata.extracted_text),
    })
}

async fn build_processed_document_from_external_candidate(
    state: &AppState,
    metadata: &crate::PdfMetadataCandidate,
    candidate: &ExternalMetadataCandidate,
    title_lookup: &TitleLookupResult,
    extra_warning: Option<String>,
) -> Result<ProcessedDocument, ApiError> {
    let merged = merge_metadata_candidate(metadata, candidate);
    if let Some(doi) = candidate.doi.clone() {
        if let Ok(retrieved_input) = fetch_bibtex_for_doi(&state.http_client, &doi).await {
            return build_processed_document_from_bibtex(
                state,
                &merged,
                &retrieved_input,
                Some(doi),
                build_processing_metadata_json(
                    "title_lookup_doi",
                    &merged,
                    title_lookup,
                    Some(candidate),
                    extra_warning.map(|warning| json!({ "lookup_warning": warning })),
                ),
                None,
            );
        }
    }

    let partial_input = build_partial_bibtex_from_pdf_metadata(&merged).ok_or_else(|| {
        ApiError::bad_request("The title lookup result did not contain enough metadata.")
    })?;
    build_processed_document_from_partial(
        state,
        &merged,
        &partial_input,
        build_processing_metadata_json(
            "title_lookup_partial",
            &merged,
            title_lookup,
            Some(candidate),
            extra_warning.map(|warning| json!({ "lookup_warning": warning })),
        ),
    )
}

fn build_failed_document(
    metadata: &crate::PdfMetadataCandidate,
    processing_error: Option<String>,
    source_metadata_json: Value,
) -> ProcessedDocument {
    ProcessedDocument {
        title: metadata.title.clone(),
        authors_text: metadata.author.clone(),
        year: metadata.year.clone(),
        doi: metadata.doi.clone(),
        abstract_text: metadata.abstract_text.clone(),
        citation_key: None,
        bibtex_source: None,
        bibtex_formatted: None,
        source_metadata_json,
        processing_status: "error".to_string(),
        processing_error,
        auto_tags: build_auto_tags(&[], &metadata.extracted_text),
    }
}

fn bibtex_summary_is_complete(summary: &Option<BibtexSummary>) -> bool {
    summary
        .as_ref()
        .map(|summary| {
            summary
                .title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
                && summary
                    .authors_text
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some()
                && summary
                    .year
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some()
        })
        .unwrap_or(false)
}

fn trusted_local_metadata(metadata: &crate::PdfMetadataCandidate) -> crate::PdfMetadataCandidate {
    let mut trusted = metadata.clone();
    if trusted
        .doi
        .as_deref()
        .map(crate::is_repository_style_doi)
        .unwrap_or(false)
    {
        trusted.doi = None;
    }
    trusted
}

fn doi_lookup_matches_local_metadata(
    metadata: &crate::PdfMetadataCandidate,
    summary: &Option<BibtexSummary>,
) -> bool {
    let Some(local_title) = metadata.title.as_deref() else {
        return true;
    };
    let Some(lookup_title) = summary
        .as_ref()
        .and_then(|candidate| candidate.title.as_deref())
    else {
        return true;
    };

    score_title_similarity(local_title, lookup_title) >= 0.7
}

fn local_metadata_json(metadata: &crate::PdfMetadataCandidate) -> Value {
    json!({
        "title": metadata.title,
        "author": metadata.author,
        "year": metadata.year,
        "doi": metadata.doi,
        "venue": metadata.venue,
        "venue_kind": metadata.venue_kind,
        "abstract_text": metadata.abstract_text
    })
}

fn candidate_metadata_json(candidate: &ExternalMetadataCandidate) -> Value {
    json!({
        "source": candidate.source,
        "title": candidate.title,
        "authors_text": candidate.authors_text,
        "year": candidate.year,
        "doi": candidate.doi,
        "abstract_text": candidate.abstract_text,
        "venue": candidate.venue,
        "venue_kind": candidate.venue_kind,
        "url": candidate.url,
        "confidence": candidate.confidence
    })
}

fn build_processing_metadata_json(
    resolution: &str,
    metadata: &crate::PdfMetadataCandidate,
    title_lookup: &TitleLookupResult,
    applied_candidate: Option<&ExternalMetadataCandidate>,
    extra: Option<Value>,
) -> Value {
    let mut enrichment = json!({
        "warnings": title_lookup.warnings,
        "attempts": title_lookup.attempts,
        "suggestion": title_lookup.suggestion.as_ref().map(candidate_metadata_json),
    });
    if let Some(applied) = applied_candidate {
        enrichment["applied"] = candidate_metadata_json(applied);
    }
    if let Some(extra) = extra {
        enrichment["extra"] = extra;
    }

    json!({
        "resolution": resolution,
        "pdf_metadata": local_metadata_json(metadata),
        "enrichment": enrichment
    })
}

fn merge_metadata_candidate(
    metadata: &crate::PdfMetadataCandidate,
    candidate: &ExternalMetadataCandidate,
) -> crate::PdfMetadataCandidate {
    crate::PdfMetadataCandidate {
        title: Some(candidate.title.clone()).or_else(|| metadata.title.clone()),
        author: candidate
            .authors_text
            .clone()
            .or_else(|| metadata.author.clone()),
        year: candidate.year.clone().or_else(|| metadata.year.clone()),
        doi: candidate.doi.clone().or_else(|| metadata.doi.clone()),
        venue: candidate.venue.clone().or_else(|| metadata.venue.clone()),
        venue_kind: candidate
            .venue_kind
            .clone()
            .or_else(|| metadata.venue_kind.clone()),
        abstract_text: candidate
            .abstract_text
            .clone()
            .or_else(|| metadata.abstract_text.clone()),
        extracted_text: metadata.extracted_text.clone(),
    }
}

async fn enrich_metadata_from_title(
    state: &AppState,
    metadata: &crate::PdfMetadataCandidate,
) -> TitleLookupResult {
    let mut result = TitleLookupResult::default();
    let Some(title) = metadata
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| value.len() >= 12)
    else {
        return result;
    };
    if !state.external_metadata_lookup_enabled {
        return result;
    }

    let timeout = state.external_metadata_lookup_timeout;
    let mut candidates = Vec::new();

    match fetch_crossref_title_candidates(&state.http_client, timeout, title, metadata).await {
        Ok(found) => {
            result
                .attempts
                .push(json!({ "source": "crossref", "candidate_count": found.len() }));
            candidates.extend(found);
        }
        Err(error) => result
            .attempts
            .push(json!({ "source": "crossref", "error": error })),
    }
    match fetch_openalex_title_candidates(&state.http_client, timeout, title, metadata).await {
        Ok(found) => {
            result
                .attempts
                .push(json!({ "source": "openalex", "candidate_count": found.len() }));
            candidates.extend(found);
        }
        Err(error) => result
            .attempts
            .push(json!({ "source": "openalex", "error": error })),
    }
    match fetch_semantic_scholar_title_candidates(&state.http_client, timeout, title, metadata)
        .await
    {
        Ok(found) => {
            result
                .attempts
                .push(json!({ "source": "semantic_scholar", "candidate_count": found.len() }));
            candidates.extend(found);
        }
        Err(error) => result
            .attempts
            .push(json!({ "source": "semantic_scholar", "error": error })),
    }

    candidates.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    dedup_candidates(&mut candidates);

    if let Some(best) = candidates.into_iter().next() {
        if best.confidence >= 0.82 {
            if metadata
                .doi
                .as_deref()
                .zip(best.doi.as_deref())
                .map(|(local, external)| local != external)
                .unwrap_or(false)
            {
                result.warnings.push(format!(
                    "Local DOI metadata was replaced using a higher-confidence {} title match.",
                    best.source
                ));
            }
            result.applied = Some(best);
        } else if best.confidence >= 0.64 {
            result.warnings.push(format!(
                "Found a possible {} title match, but confidence was too low to apply automatically.",
                best.source
            ));
            result.suggestion = Some(best);
        }
    } else if metadata
        .doi
        .as_deref()
        .map(looks_like_incomplete_doi)
        .unwrap_or(false)
    {
        result.warnings.push(
            "The extracted DOI looks incomplete, and no strong title-based metadata match was found."
                .to_string(),
        );
    }

    result
}

async fn fetch_crossref_title_candidates(
    client: &reqwest::Client,
    timeout: Duration,
    title: &str,
    metadata: &crate::PdfMetadataCandidate,
) -> Result<Vec<ExternalMetadataCandidate>, String> {
    let mut url = reqwest::Url::parse("https://api.crossref.org/works")
        .map_err(|error| format!("invalid Crossref URL: {error}"))?;
    url.query_pairs_mut()
        .append_pair("query.title", title)
        .append_pair("rows", "5");

    let response = client
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("status {}", response.status()));
    }
    let payload = response
        .json::<Value>()
        .await
        .map_err(|error| format!("invalid response: {error}"))?;

    Ok(payload["message"]["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let title = item["title"]
                .as_array()
                .and_then(|titles| titles.first())
                .and_then(Value::as_str)?
                .trim()
                .to_string();
            let authors_text = item["author"]
                .as_array()
                .map(|authors| format_crossref_authors(authors));
            let year = extract_json_year(&item["issued"]["date-parts"]);
            let doi = item["DOI"].as_str().and_then(crate::normalize_doi_input);
            let venue = first_array_string(&item["container-title"])
                .or_else(|| first_array_string(&item["short-container-title"]));
            let venue_kind = item["type"]
                .as_str()
                .and_then(crossref_type_to_venue_kind)
                .map(str::to_string);
            let abstract_text = item["abstract"].as_str().map(strip_html_tags);
            let url = item["URL"].as_str().map(str::to_string);
            let confidence = score_external_candidate(
                metadata,
                &title,
                authors_text.as_deref(),
                year.as_deref(),
                venue.as_deref(),
                doi.as_deref(),
            );
            (confidence >= 0.45).then_some(ExternalMetadataCandidate {
                source: "Crossref",
                title,
                authors_text,
                year,
                doi,
                abstract_text,
                venue,
                venue_kind,
                url,
                confidence,
            })
        })
        .collect())
}

async fn fetch_openalex_title_candidates(
    client: &reqwest::Client,
    timeout: Duration,
    title: &str,
    metadata: &crate::PdfMetadataCandidate,
) -> Result<Vec<ExternalMetadataCandidate>, String> {
    let mut url = reqwest::Url::parse("https://api.openalex.org/works")
        .map_err(|error| format!("invalid OpenAlex URL: {error}"))?;
    url.query_pairs_mut()
        .append_pair("search", title)
        .append_pair("per-page", "5");

    let response = client
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("status {}", response.status()));
    }
    let payload = response
        .json::<Value>()
        .await
        .map_err(|error| format!("invalid response: {error}"))?;

    Ok(payload["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let title = item["display_name"].as_str()?.trim().to_string();
            let authors_text = item["authorships"]
                .as_array()
                .map(|authorships| format_openalex_authors(authorships));
            let year = item["publication_year"]
                .as_i64()
                .map(|value| value.to_string());
            let doi = item["doi"].as_str().and_then(crate::normalize_doi_input);
            let venue = item["primary_location"]["source"]["display_name"]
                .as_str()
                .map(str::to_string);
            let venue_kind = item["type_crossref"]
                .as_str()
                .and_then(crossref_type_to_venue_kind)
                .map(str::to_string);
            let abstract_text = item["abstract_inverted_index"]
                .as_object()
                .map(reconstruct_openalex_abstract);
            let url = item["primary_location"]["landing_page_url"]
                .as_str()
                .or_else(|| item["id"].as_str())
                .map(str::to_string);
            let confidence = score_external_candidate(
                metadata,
                &title,
                authors_text.as_deref(),
                year.as_deref(),
                venue.as_deref(),
                doi.as_deref(),
            );
            (confidence >= 0.45).then_some(ExternalMetadataCandidate {
                source: "OpenAlex",
                title,
                authors_text,
                year,
                doi,
                abstract_text,
                venue,
                venue_kind,
                url,
                confidence,
            })
        })
        .collect())
}

async fn fetch_semantic_scholar_title_candidates(
    client: &reqwest::Client,
    timeout: Duration,
    title: &str,
    metadata: &crate::PdfMetadataCandidate,
) -> Result<Vec<ExternalMetadataCandidate>, String> {
    let mut url = reqwest::Url::parse("https://api.semanticscholar.org/graph/v1/paper/search")
        .map_err(|error| format!("invalid Semantic Scholar URL: {error}"))?;
    url.query_pairs_mut()
        .append_pair("query", title)
        .append_pair("limit", "5")
        .append_pair(
            "fields",
            "title,authors,year,venue,externalIds,url,abstract,publicationTypes",
        );

    let response = client
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("status {}", response.status()));
    }
    let payload = response
        .json::<Value>()
        .await
        .map_err(|error| format!("invalid response: {error}"))?;

    Ok(payload["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let title = item["title"].as_str()?.trim().to_string();
            let authors_text = item["authors"]
                .as_array()
                .map(|authors| format_semantic_scholar_authors(authors));
            let year = item["year"].as_i64().map(|value| value.to_string());
            let doi = item["externalIds"]["DOI"]
                .as_str()
                .and_then(crate::normalize_doi_input);
            let venue = item["venue"].as_str().map(str::to_string);
            let venue_kind = item["publicationTypes"]
                .as_array()
                .and_then(|values| values.first())
                .and_then(Value::as_str)
                .and_then(crossref_type_to_venue_kind)
                .map(str::to_string);
            let abstract_text = item["abstract"].as_str().map(str::to_string);
            let url = item["url"].as_str().map(str::to_string);
            let confidence = score_external_candidate(
                metadata,
                &title,
                authors_text.as_deref(),
                year.as_deref(),
                venue.as_deref(),
                doi.as_deref(),
            );
            (confidence >= 0.45).then_some(ExternalMetadataCandidate {
                source: "Semantic Scholar",
                title,
                authors_text,
                year,
                doi,
                abstract_text,
                venue,
                venue_kind,
                url,
                confidence,
            })
        })
        .collect())
}

fn dedup_candidates(candidates: &mut Vec<ExternalMetadataCandidate>) {
    let mut seen = HashSet::new();
    candidates.retain(|candidate| {
        let key = candidate
            .doi
            .clone()
            .unwrap_or_else(|| normalize_lookup_text(&candidate.title));
        seen.insert(key)
    });
}

fn format_crossref_authors(authors: &[Value]) -> String {
    authors
        .iter()
        .filter_map(|author| {
            let given = author["given"].as_str().unwrap_or("").trim();
            let family = author["family"].as_str().unwrap_or("").trim();
            let name = [given, family]
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            (!name.is_empty()).then_some(name)
        })
        .collect::<Vec<_>>()
        .join(" and ")
}

fn format_openalex_authors(authorships: &[Value]) -> String {
    authorships
        .iter()
        .filter_map(|author| {
            author["author"]["display_name"]
                .as_str()
                .map(str::to_string)
        })
        .collect::<Vec<_>>()
        .join(" and ")
}

fn format_semantic_scholar_authors(authors: &[Value]) -> String {
    authors
        .iter()
        .filter_map(|author| author["name"].as_str().map(str::to_string))
        .collect::<Vec<_>>()
        .join(" and ")
}

fn reconstruct_openalex_abstract(index: &serde_json::Map<String, Value>) -> String {
    let mut positions = Vec::<(usize, &str)>::new();
    for (word, value) in index {
        if let Some(array) = value.as_array() {
            for position in array.iter().filter_map(Value::as_u64) {
                positions.push((position as usize, word.as_str()));
            }
        }
    }
    positions.sort_by_key(|(position, _)| *position);
    positions
        .into_iter()
        .map(|(_, word)| word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn first_array_string(value: &Value) -> Option<String> {
    value
        .as_array()
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_json_year(value: &Value) -> Option<String> {
    value
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_i64)
        .map(|year| year.to_string())
}

fn crossref_type_to_venue_kind(value: &str) -> Option<&'static str> {
    let lower = value.to_ascii_lowercase();
    if lower.contains("journal") {
        Some("journal")
    } else if lower.contains("proceedings")
        || lower.contains("conference")
        || lower.contains("workshop")
    {
        Some("booktitle")
    } else {
        None
    }
}

fn score_external_candidate(
    metadata: &crate::PdfMetadataCandidate,
    title: &str,
    authors_text: Option<&str>,
    year: Option<&str>,
    venue: Option<&str>,
    doi: Option<&str>,
) -> f32 {
    let Some(local_title) = metadata.title.as_deref() else {
        return 0.0;
    };
    let title_similarity = score_title_similarity(local_title, title);
    if title_similarity < 0.45 {
        return 0.0;
    }

    let mut score = title_similarity * 0.72;
    if metadata
        .year
        .as_deref()
        .zip(year)
        .map(|(left, right)| left == right)
        .unwrap_or(false)
    {
        score += 0.12;
    }

    let author_overlap = metadata
        .author
        .as_deref()
        .zip(authors_text)
        .map(|(left, right)| author_overlap_score(left, right))
        .unwrap_or(0.0);
    score += author_overlap * 0.12;

    let venue_score = metadata
        .venue
        .as_deref()
        .zip(venue)
        .map(|(left, right)| score_title_similarity(left, right))
        .unwrap_or(0.0);
    score += venue_score * 0.06;

    if metadata
        .doi
        .as_deref()
        .zip(doi)
        .map(|(left, right)| doi_relationship_score(left, right))
        .unwrap_or(false)
    {
        score += 0.12;
    }

    score.clamp(0.0, 0.99)
}

fn score_title_similarity(left: &str, right: &str) -> f32 {
    let normalized_left = normalize_lookup_text(left);
    let normalized_right = normalize_lookup_text(right);
    if normalized_left.is_empty() || normalized_right.is_empty() {
        return 0.0;
    }
    if normalized_left == normalized_right {
        return 1.0;
    }
    if normalized_left.contains(&normalized_right) || normalized_right.contains(&normalized_left) {
        return 0.9;
    }

    let left_tokens = normalized_left.split_whitespace().collect::<HashSet<_>>();
    let right_tokens = normalized_right.split_whitespace().collect::<HashSet<_>>();
    let intersection = left_tokens.intersection(&right_tokens).count() as f32;
    let union = left_tokens.union(&right_tokens).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn normalize_lookup_text(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn author_overlap_score(left: &str, right: &str) -> f32 {
    let left = author_surnames(left);
    let right = author_surnames(right);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(&right).count() as f32;
    let union = left.union(&right).count() as f32;
    intersection / union
}

fn author_surnames(value: &str) -> HashSet<String> {
    value
        .split(" and ")
        .flat_map(|part| part.split(';'))
        .flat_map(|part| part.split(','))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let token = part
                .split_whitespace()
                .last()
                .unwrap_or(part)
                .trim_matches(|character: char| !character.is_ascii_alphabetic())
                .to_lowercase();
            (token.len() >= 3).then_some(token)
        })
        .collect()
}

fn doi_relationship_score(left: &str, right: &str) -> bool {
    let left = left.to_ascii_lowercase();
    let right = right.to_ascii_lowercase();
    left == right || left.starts_with(&right) || right.starts_with(&left)
}

fn looks_like_incomplete_doi(doi: &str) -> bool {
    let trimmed = doi.trim();
    if !trimmed.starts_with("10.") || !trimmed.contains('/') {
        return true;
    }
    let suffix = trimmed
        .split_once('/')
        .map(|(_, suffix)| suffix)
        .unwrap_or("");
    suffix.len() < 6 || !suffix.chars().any(|character| character.is_ascii_digit())
}

fn strip_html_tags(input: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for character in input.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn sync_document_tags(
    pool: &PgPool,
    project_id: &str,
    document_id: &str,
    user_id: &str,
    source: &str,
    tags: &[(String, Option<f32>)],
    replace_existing_source: bool,
) -> Result<(), ApiError> {
    if replace_existing_source {
        sqlx::query("DELETE FROM document_tags WHERE document_id = $1 AND source = $2")
            .bind(document_id)
            .bind(source)
            .execute(pool)
            .await
            .map_err(|error| {
                ApiError::internal(format!("Failed to clear existing document tags: {error}"))
            })?;
    }

    let mut seen = HashSet::new();
    for (tag_name, confidence) in tags {
        let normalized = normalize_tag_name(tag_name)
            .ok_or_else(|| ApiError::bad_request("Encountered an invalid tag name."))?;
        if !seen.insert(normalized.to_lowercase()) {
            continue;
        }
        let tag_id = upsert_project_tag(pool, project_id, user_id, &normalized).await?;
        sqlx::query(
            r#"
            INSERT INTO document_tags (document_id, tag_id, source, confidence)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (document_id, tag_id)
            DO UPDATE SET source = EXCLUDED.source, confidence = EXCLUDED.confidence
            "#,
        )
        .bind(document_id)
        .bind(&tag_id)
        .bind(source)
        .bind(confidence)
        .execute(pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("Failed to assign tag to document: {error}"))
        })?;
    }

    cleanup_orphan_project_tags(pool, project_id).await?;

    Ok(())
}

async fn cleanup_orphan_project_tags(pool: &PgPool, project_id: &str) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        DELETE FROM tags
        WHERE project_id = $1
          AND NOT EXISTS (
              SELECT 1
              FROM document_tags
              WHERE document_tags.tag_id = tags.id
          )
        "#,
    )
    .bind(project_id)
    .execute(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to clean orphan project tags: {error}")))?;

    Ok(())
}

async fn upsert_project_tag(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    tag_name: &str,
) -> Result<String, ApiError> {
    let tag_id = random_token(18);
    let normalized_name = tag_name.to_lowercase();
    sqlx::query_scalar(
        r#"
        INSERT INTO tags (id, project_id, name, normalized_name, created_by_user_id)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (project_id, normalized_name)
        DO UPDATE SET name = EXCLUDED.name
        RETURNING id
        "#,
    )
    .bind(tag_id)
    .bind(project_id)
    .bind(tag_name)
    .bind(&normalized_name)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|error| ApiError::internal(format!("Failed to upsert project tag: {error}")))
}

async fn build_document_detail_response(
    state: &AppState,
    project_id: &str,
    document_id: &str,
) -> Result<DocumentDetailResponse, ApiError> {
    let document = load_document(&state.db_pool, document_id).await?;
    let (journal, booktitle) = extract_document_venue_fields(&document);
    let venue_reviews = build_venue_reviews(state, journal.as_deref(), booktitle.as_deref());
    let tags = load_document_tags(&state.db_pool, document_id)
        .await?
        .iter()
        .map(tag_from_row)
        .collect::<Vec<_>>();
    let notes = load_notes(&state.db_pool, document_id).await?;
    let duplicate_warnings = load_duplicate_warnings(
        &state.db_pool,
        project_id,
        &document.id,
        document.doi.as_deref(),
        &document.sha256,
    )
    .await?;
    let enrichment_warnings = extract_enrichment_warnings(&document.source_metadata_json);
    Ok(DocumentDetailResponse {
        id: document.id,
        project_id: document.project_id,
        folder_id: document.folder_id,
        original_filename: document.original_filename,
        title: document.title,
        authors_text: document.authors_text,
        year: document.year,
        doi: document.doi,
        abstract_text: document.abstract_text,
        journal,
        booktitle,
        citation_key: document.citation_key,
        processing_status: document.processing_status,
        processing_error: document.processing_error,
        bibtex_source: document.bibtex_source,
        bibtex_formatted: document.bibtex_formatted,
        source_metadata_json: document.source_metadata_json,
        tags,
        venue_reviews,
        notes,
        duplicate_warnings,
        enrichment_warnings,
        file_url: format!("/api/documents/{document_id}/file"),
    })
}

fn extract_enrichment_warnings(source_metadata_json: &Value) -> Vec<String> {
    source_metadata_json["enrichment"]["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

async fn load_duplicate_warnings(
    pool: &PgPool,
    project_id: &str,
    document_id: &str,
    doi: Option<&str>,
    sha256: &str,
) -> Result<Vec<String>, ApiError> {
    let rows: Vec<(String, Option<String>, Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT original_filename, doi, citation_key, sha256
        FROM documents
        WHERE project_id = $1
          AND id <> $2
          AND (($3 IS NOT NULL AND doi = $3) OR sha256 = $4)
        "#,
    )
    .bind(project_id)
    .bind(document_id)
    .bind(doi)
    .bind(sha256)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        ApiError::internal(format!("Failed to inspect duplicate documents: {error}"))
    })?;

    let mut warnings = Vec::new();
    for (original_filename, existing_doi, citation_key, existing_sha256) in rows {
        if doi.is_some() && existing_doi.as_deref() == doi {
            warnings.push(format!(
                "Potential duplicate: '{}' already uses DOI {}{}.",
                original_filename,
                existing_doi.unwrap_or_default(),
                citation_key
                    .as_deref()
                    .map(|value| format!(" (key {value})"))
                    .unwrap_or_default()
            ));
        }
        if existing_sha256 == sha256 {
            warnings.push(format!(
                "Potential duplicate: '{}' has the same PDF content hash.",
                original_filename
            ));
        }
    }
    warnings.sort();
    warnings.dedup();
    Ok(warnings)
}

fn user_summary(user: AuthenticatedUser) -> UserSummaryResponse {
    UserSummaryResponse {
        id: user.user_id,
        email: user.email,
        username: user.username,
        display_name: user.display_name,
        avatar_url: user.avatar_url,
    }
}

fn collect_used_definition_tokens(
    definitions: &DefinitionCollection,
    entries: &[String],
) -> HashSet<String> {
    let mut used = HashSet::new();
    for entry in entries {
        if let Ok(parsed_entries) = parse_bibtex_document(entry) {
            for parsed in parsed_entries {
                for field_name in ["journal", "booktitle"] {
                    if let Some(raw_value) = parsed.fields.get(field_name) {
                        let candidate = strip_wrapping(raw_value);
                        let token = candidate.trim();
                        if definitions.contains_token(token) {
                            used.insert(token.to_ascii_uppercase());
                        }
                    }
                }
            }
        }
    }
    used
}

fn extract_document_venue_fields(document: &DocumentRow) -> (Option<String>, Option<String>) {
    let source = document
        .bibtex_source
        .as_deref()
        .or(document.bibtex_formatted.as_deref());
    let Some(source) = source else {
        return (None, None);
    };

    let Ok(entries) = parse_bibtex_document(source) else {
        return (None, None);
    };
    let Some(entry) = entries.into_iter().find(|entry| {
        !entry.entry_type.eq_ignore_ascii_case("string")
            && !entry.entry_type.eq_ignore_ascii_case("comment")
    }) else {
        return (None, None);
    };

    (
        entry
            .fields
            .get("journal")
            .map(|value| strip_wrapping(value)),
        entry
            .fields
            .get("booktitle")
            .map(|value| strip_wrapping(value)),
    )
}

fn build_venue_reviews(
    state: &AppState,
    journal: Option<&str>,
    booktitle: Option<&str>,
) -> Vec<VenueReviewResponse> {
    let Ok(definitions) = crate::load_definitions(state) else {
        return Vec::new();
    };

    [("journal", journal), ("booktitle", booktitle)]
        .into_iter()
        .filter_map(|(field_name, value)| {
            let value = value?.trim();
            if value.is_empty() {
                return None;
            }

            let matched_definition = definitions
                .find_by_name(value)
                .map(|definition| venue_definition_response(&definition));

            let suggestions = definitions
                .similar_by_name(value, 3)
                .into_iter()
                .map(|suggestion| venue_suggestion_response(&suggestion))
                .collect::<Vec<_>>();

            let proposed_definition =
                if matched_definition.is_none() && (value.contains(' ') || value.contains('/')) {
                    Some(venue_definition_response(&create_definition(
                        value,
                        &definitions,
                    )))
                } else {
                    None
                };

            Some(VenueReviewResponse {
                field_name: field_name.to_string(),
                value: value.to_string(),
                matched_definition,
                proposed_definition,
                suggestions,
            })
        })
        .collect()
}

fn venue_suggestion_response(
    suggestion: &crate::bibtex::VenueSuggestion,
) -> VenueSuggestionResponse {
    VenueSuggestionResponse {
        token: suggestion.token.clone(),
        raw_value: suggestion.raw_value.clone(),
        expanded_value: suggestion.expanded_value.clone(),
        definition_text: format_string_definition(&crate::bibtex::definition_from_parts(
            &suggestion.token,
            &suggestion.raw_value,
        )),
        score: suggestion.score,
    }
}

fn venue_definition_response(
    definition: &crate::bibtex::AbbreviationDefinition,
) -> VenueDefinitionResponse {
    VenueDefinitionResponse {
        token: definition.token.clone(),
        raw_value: definition.raw_value.clone(),
        expanded_value: definition.expanded_value.clone(),
        definition_text: format_string_definition(definition),
    }
}

fn rebuild_document_bibtex(
    state: &AppState,
    current: &DocumentRow,
    title: Option<&str>,
    authors_text: Option<&str>,
    year: Option<&str>,
    doi: Option<&str>,
    abstract_text: Option<&str>,
    journal: Option<&str>,
    booktitle: Option<&str>,
) -> Result<RebuiltDocumentBibtex, ApiError> {
    let source = current
        .bibtex_source
        .as_deref()
        .or(current.bibtex_formatted.as_deref());

    let mut entry = source
        .and_then(|input| parse_bibtex_document(input).ok())
        .and_then(|entries| {
            entries.into_iter().find(|entry| {
                !entry.entry_type.eq_ignore_ascii_case("string")
                    && !entry.entry_type.eq_ignore_ascii_case("comment")
            })
        })
        .unwrap_or_else(|| crate::bibtex::RawEntry {
            entry_type: if booktitle.filter(|value| !value.is_empty()).is_some() {
                "inproceedings".to_string()
            } else {
                "article".to_string()
            },
            key: current
                .citation_key
                .clone()
                .unwrap_or_else(|| "workspace_entry".to_string()),
            fields: HashMap::new(),
            raw_text: String::new(),
        });

    let definitions = crate::load_definitions(state).or_else(|_| {
        DefinitionCollection::from_source("").map_err(|error| ApiError::internal(error.to_string()))
    })?;

    upsert_raw_field(&mut entry.fields, "title", title, false, &definitions);
    upsert_raw_field(
        &mut entry.fields,
        "author",
        authors_text,
        false,
        &definitions,
    );
    upsert_raw_field(&mut entry.fields, "year", year, false, &definitions);
    upsert_raw_field(&mut entry.fields, "doi", doi, false, &definitions);
    upsert_raw_field(
        &mut entry.fields,
        "abstract",
        abstract_text,
        false,
        &definitions,
    );
    upsert_raw_field(&mut entry.fields, "journal", journal, true, &definitions);
    upsert_raw_field(
        &mut entry.fields,
        "booktitle",
        booktitle,
        true,
        &definitions,
    );

    let rebuilt_source = render_raw_entry(&entry);
    let result = format_entries(
        &[entry],
        &mut definitions.clone(),
        &FormatOptions {
            include_string_definitions: false,
            use_conferences_bib: true,
            rewrite_reference_key: true,
            keep_doi: false,
        },
    )
    .map_err(|error| ApiError::bad_request(format!("Failed to update document BibTeX: {error}")))?;

    let formatted = result
        .entries
        .first()
        .map(|entry| entry.output_text.clone())
        .unwrap_or_else(|| result.output.clone());
    let citation_key = result.entries.first().map(|entry| entry.key.clone());

    Ok(RebuiltDocumentBibtex {
        source: rebuilt_source,
        formatted,
        citation_key,
    })
}

fn upsert_raw_field(
    fields: &mut HashMap<String, String>,
    field_name: &str,
    value: Option<&str>,
    allow_token_reference: bool,
    definitions: &DefinitionCollection,
) {
    let Some(value) = value.map(str::trim) else {
        return;
    };
    if value.is_empty() {
        fields.remove(field_name);
        return;
    }

    let raw_value = if allow_token_reference && definitions.contains_token(value) {
        value.to_string()
    } else {
        format!("{{{value}}}")
    };
    fields.insert(field_name.to_string(), raw_value);
}

fn render_raw_entry(entry: &crate::bibtex::RawEntry) -> String {
    let mut field_names = entry.fields.keys().cloned().collect::<Vec<_>>();
    field_names.sort();
    let rendered_fields = field_names
        .into_iter()
        .filter_map(|field_name| {
            entry
                .fields
                .get(&field_name)
                .map(|value| format!("  {field_name} = {value}"))
        })
        .collect::<Vec<_>>();

    if rendered_fields.is_empty() {
        format!("@{}{{{}}}", entry.entry_type, entry.key)
    } else {
        format!(
            "@{}{{{},\n{}\n}}",
            entry.entry_type,
            entry.key,
            rendered_fields.join(",\n")
        )
    }
}

fn document_list_item_from_row(
    row: &DocumentRow,
    tags: Option<&Vec<DocumentTagRow>>,
) -> DocumentListItemResponse {
    DocumentListItemResponse {
        id: row.id.clone(),
        project_id: row.project_id.clone(),
        folder_id: row.folder_id.clone(),
        original_filename: row.original_filename.clone(),
        title: row.title.clone(),
        authors_text: row.authors_text.clone(),
        year: row.year.clone(),
        doi: row.doi.clone(),
        citation_key: row.citation_key.clone(),
        processing_status: row.processing_status.clone(),
        processing_error: row.processing_error.clone(),
        tags: tags
            .into_iter()
            .flat_map(|rows| rows.iter())
            .map(tag_from_row)
            .collect(),
    }
}

fn tag_from_row(row: &DocumentTagRow) -> TagResponse {
    TagResponse {
        id: row.tag_id.clone(),
        name: row.tag_name.clone(),
        source: row.source.clone(),
        confidence: row.confidence,
    }
}

fn parse_project_role(value: &str) -> Option<ProjectRole> {
    match value {
        "viewer" => Some(ProjectRole::Viewer),
        "editor" => Some(ProjectRole::Editor),
        "owner" => Some(ProjectRole::Owner),
        _ => None,
    }
}

fn default_project_visibility() -> String {
    "private".to_string()
}

fn validate_project_visibility(value: &str) -> Result<&'static str, ApiError> {
    match value.trim() {
        "public" => Ok("public"),
        "private" => Ok("private"),
        _ => Err(ApiError::bad_request(
            "Project visibility must be either 'public' or 'private'.",
        )),
    }
}

fn validate_share_role(value: &str) -> Result<&'static str, ApiError> {
    match value.trim() {
        "viewer" => Ok("viewer"),
        "editor" => Ok("editor"),
        "owner" => Err(ApiError::bad_request(
            "Use the project owner record instead of assigning owner through sharing.",
        )),
        _ => Err(ApiError::bad_request(
            "Role must be either 'viewer' or 'editor'.",
        )),
    }
}

fn sanitize_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => character,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "document.pdf".to_string()
    } else if sanitized.to_ascii_lowercase().ends_with(".pdf") {
        sanitized
    } else {
        format!("{sanitized}.pdf")
    }
}

fn sanitize_header_filename(filename: &str) -> String {
    filename.replace('"', "")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

fn normalize_tag_name(value: &str) -> Option<String> {
    let cleaned = value
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.len() < 2 {
        None
    } else {
        Some(cleaned)
    }
}

fn summarize_bibtex(source: &str) -> Option<BibtexSummary> {
    let entry = parse_bibtex_document(source)
        .ok()?
        .into_iter()
        .find(|entry| {
            !entry.entry_type.eq_ignore_ascii_case("string")
                && !entry.entry_type.eq_ignore_ascii_case("comment")
        })?;
    let keywords = entry
        .fields
        .get("keywords")
        .map(|value| strip_wrapping(value))
        .map(|value| {
            value
                .split([';', ','])
                .map(str::trim)
                .filter(|candidate| candidate.len() >= 3)
                .map(|candidate| candidate.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(BibtexSummary {
        citation_key: Some(entry.key),
        title: entry.fields.get("title").map(|value| strip_wrapping(value)),
        authors_text: entry
            .fields
            .get("author")
            .map(|value| strip_wrapping(value)),
        year: entry.fields.get("year").map(|value| strip_wrapping(value)),
        abstract_text: entry
            .fields
            .get("abstract")
            .map(|value| strip_wrapping(value)),
        keywords,
    })
}

fn build_auto_tags(
    keyword_candidates: &[String],
    extracted_text: &str,
) -> Vec<(String, Option<f32>)> {
    let doi_tags = collect_keyword_tags(keyword_candidates, 0.95);
    if !doi_tags.is_empty() {
        return doi_tags;
    }

    let declared_keywords = extract_declared_keywords(extracted_text);
    let declared_tags = collect_keyword_tags(&declared_keywords, 0.9);
    if !declared_tags.is_empty() {
        return declared_tags;
    }

    extract_text_phrases(extracted_text)
        .into_iter()
        .map(|(phrase, confidence)| (phrase, Some(confidence)))
        .collect()
}

fn collect_keyword_tags(
    keyword_candidates: &[String],
    confidence: f32,
) -> Vec<(String, Option<f32>)> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    for keyword in keyword_candidates {
        if is_low_signal_keyword_candidate(keyword) {
            continue;
        }
        if let Some(normalized) = normalize_tag_name(keyword) {
            let lowered = normalized.to_lowercase();
            if seen.insert(lowered) {
                results.push((normalized, Some(confidence)));
            }
        }
        if results.len() >= 8 {
            return results;
        }
    }

    results
}

fn extract_declared_keywords(extracted_text: &str) -> Vec<String> {
    let snippet = extracted_text.chars().take(16_000).collect::<String>();
    if snippet.trim().is_empty() {
        return Vec::new();
    }

    let lines = snippet.lines().collect::<Vec<_>>();
    let abstract_index = lines
        .iter()
        .position(|line| line.to_lowercase().contains("abstract"))
        .unwrap_or(0);

    find_declared_keywords_in_lines(&lines[abstract_index..])
        .or_else(|| find_declared_keywords_in_lines(&lines))
        .unwrap_or_default()
}

fn find_declared_keywords_in_lines(lines: &[&str]) -> Option<Vec<String>> {
    for (index, line) in lines.iter().enumerate() {
        let lower_line = line.to_lowercase();
        let Some((marker_start, marker_length)) = find_keyword_marker(&lower_line) else {
            continue;
        };

        let mut section = line[marker_start + marker_length..].trim().to_string();
        section = section
            .trim_start_matches(|character: char| {
                character.is_whitespace()
                    || matches!(character, ':' | ';' | ',' | '.' | '-' | '–' | '—')
            })
            .trim()
            .to_string();

        let mut consumed = 1usize;
        while should_extend_declared_keywords(&section)
            && index + consumed < lines.len()
            && consumed < 3
        {
            let next_line = lines[index + consumed].trim();
            if next_line.is_empty() || looks_like_section_heading(next_line) {
                break;
            }
            if !section.is_empty() {
                section.push(' ');
            }
            section.push_str(next_line);
            consumed += 1;
        }

        let parsed = parse_declared_keywords(&section);
        if !parsed.is_empty() {
            return Some(parsed);
        }
    }

    None
}

fn find_keyword_marker(line: &str) -> Option<(usize, usize)> {
    ["keywords", "key words", "index terms"]
        .iter()
        .filter_map(|marker| line.find(marker).map(|start| (start, marker.len())))
        .min_by_key(|(start, _)| *start)
}

fn should_extend_declared_keywords(section: &str) -> bool {
    !section.is_empty()
        && !section.contains(';')
        && !section.contains(',')
        && !section.contains('•')
        && !section.contains('·')
        && section.split_whitespace().count() < 8
        && section.len() < 120
}

fn looks_like_section_heading(value: &str) -> bool {
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

fn parse_declared_keywords(section: &str) -> Vec<String> {
    let normalized = section.replace(['•', '·'], ";").replace(['–', '—'], "-");

    let candidates = normalized
        .split([';', ','])
        .map(|candidate| {
            candidate
                .trim()
                .trim_matches(|character: char| {
                    character.is_whitespace()
                        || matches!(
                            character,
                            ':' | ';' | ',' | '.' | '(' | ')' | '[' | ']' | '{' | '}' | '-'
                        )
                })
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|candidate| !candidate.is_empty())
        .filter(|candidate| candidate.split_whitespace().count() <= 6)
        .filter(|candidate| candidate.len() <= 64)
        .map(|candidate| candidate.to_string())
        .collect::<Vec<_>>();

    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for candidate in candidates {
        if is_low_signal_keyword_candidate(&candidate) {
            continue;
        }
        let lowered = candidate.to_lowercase();
        if seen.insert(lowered) {
            deduped.push(candidate);
        }
        if deduped.len() >= 8 {
            break;
        }
    }

    deduped
}

fn extract_text_phrases(extracted_text: &str) -> Vec<(String, f32)> {
    let snippet = extracted_text
        .chars()
        .take(20_000)
        .collect::<String>()
        .to_lowercase();
    let tokens = snippet
        .split_whitespace()
        .filter_map(normalize_phrase_token)
        .collect::<Vec<_>>();

    let mut frequencies = HashMap::new();
    let mut current_window = Vec::new();

    for token in tokens {
        if STOPWORDS.contains(&token.as_str()) {
            current_window.clear();
            continue;
        }

        current_window.push(token);
        if current_window.len() > 5 {
            current_window.remove(0);
        }

        let max_size = current_window.len().min(4);
        for size in (2..=max_size).rev() {
            let candidate_tokens = &current_window[current_window.len() - size..];
            if let Some(candidate) = normalize_phrase_candidate(candidate_tokens) {
                *frequencies.entry(candidate).or_insert(0usize) += 1;
            }
        }
    }

    let mut ranked = frequencies
        .into_iter()
        .map(|(phrase, count)| {
            let word_count = phrase.split_whitespace().count() as f32;
            let confidence = ((count as f32 * 0.18) + (word_count * 0.1)).clamp(0.25, 0.82);
            (phrase, count, confidence, word_count as usize)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.3.cmp(&left.3))
            .then_with(|| left.0.cmp(&right.0))
    });

    let mut selected = Vec::new();
    for (phrase, count, confidence, word_count) in ranked {
        if count < 2 && word_count < 3 {
            continue;
        }
        if is_generic_phrase(&phrase) || is_redundant_text_phrase(&selected, &phrase) {
            continue;
        }
        selected.push((phrase, confidence));
        if selected.len() >= 8 {
            break;
        }
    }

    selected
}

fn normalize_phrase_token(raw: &str) -> Option<String> {
    let cleaned = raw
        .trim_matches(|character: char| !character.is_alphanumeric() && character != '-')
        .trim_matches('-')
        .to_string();
    if cleaned.len() < 3 || is_numeric_like(&cleaned) {
        return None;
    }
    if cleaned
        .chars()
        .filter(|character| character.is_ascii_digit())
        .count()
        >= cleaned.len() / 2
    {
        return None;
    }

    Some(cleaned)
}

fn normalize_phrase_candidate(tokens: &[String]) -> Option<String> {
    if tokens.len() < 2 {
        return None;
    }

    let candidate = tokens.join(" ");
    let cleaned = candidate
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if cleaned.len() < 8 {
        return None;
    }
    if cleaned.split_whitespace().any(|token| token.len() <= 2) {
        return None;
    }
    if is_generic_phrase(&cleaned) {
        return None;
    }

    Some(cleaned)
}

fn is_generic_phrase(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty()
        || GENERIC_TAG_PHRASES.contains(&trimmed)
        || trimmed.starts_with("www ")
        || trimmed.starts_with("http ")
        || trimmed.contains("copyright")
        || trimmed.contains("all rights reserved")
        || trimmed.contains("permission to make")
}

fn is_low_signal_keyword_candidate(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }

    let lowered = trimmed.to_lowercase();
    if is_generic_phrase(&lowered) || is_numeric_like(trimmed) {
        return true;
    }

    let words = lowered.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return true;
    }

    if words.iter().any(|word| is_numeric_like(word)) {
        return true;
    }

    if words.len() == 1 {
        let token = words[0];
        let acronym_like = trimmed.len() <= 5
            && trimmed
                .chars()
                .any(|character| character.is_ascii_uppercase())
            && trimmed
                .chars()
                .any(|character| character.is_ascii_alphanumeric());
        if LOW_SIGNAL_SINGLE_WORD_TAGS.contains(&token) {
            return true;
        }
        if token.len() < 5 && !acronym_like {
            return true;
        }
    }

    false
}

fn is_numeric_like(value: &str) -> bool {
    let compact = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    !compact.is_empty() && compact.chars().all(|character| character.is_ascii_digit())
}

fn is_redundant_text_phrase(current: &[(String, f32)], candidate: &str) -> bool {
    current.iter().any(|(existing, _)| {
        existing == candidate
            || existing.contains(candidate)
            || candidate.contains(existing)
            || singular_phrase(existing) == singular_phrase(candidate)
    })
}

fn singular_phrase(value: &str) -> String {
    value
        .split_whitespace()
        .map(|token| token.strip_suffix('s').unwrap_or(token))
        .collect::<Vec<_>>()
        .join(" ")
}

const STOPWORDS: &[&str] = &[
    "a",
    "about",
    "after",
    "all",
    "also",
    "an",
    "and",
    "among",
    "are",
    "as",
    "at",
    "be",
    "been",
    "between",
    "both",
    "by",
    "can",
    "each",
    "for",
    "from",
    "have",
    "in",
    "into",
    "is",
    "it",
    "its",
    "more",
    "most",
    "of",
    "on",
    "onto",
    "or",
    "our",
    "ours",
    "over",
    "paper",
    "some",
    "such",
    "than",
    "that",
    "the",
    "their",
    "theirs",
    "them",
    "there",
    "these",
    "they",
    "this",
    "those",
    "through",
    "to",
    "under",
    "upon",
    "used",
    "using",
    "very",
    "via",
    "we",
    "were",
    "which",
    "while",
    "whose",
    "will",
    "with",
    "within",
    "without",
    "work",
    "works",
    "your",
    "across",
    "study",
    "research",
    "result",
    "results",
    "method",
    "methods",
    "based",
    "toward",
    "figure",
    "table",
    "tables",
    "figures",
    "section",
    "sections",
    "introduction",
    "conclusion",
    "references",
    "appendix",
    "appendices",
    "authors",
    "author",
    "abstract",
    "keywords",
    "index",
    "terms",
    "university",
    "department",
    "conference",
    "journal",
    "proceedings",
    "preprint",
    "accepted",
    "available",
    "rights",
    "reserved",
    "permission",
];

const GENERIC_TAG_PHRASES: &[&str] = &[
    "future work",
    "related work",
    "experimental results",
    "simulation results",
    "performance evaluation",
    "evaluation results",
    "numerical results",
    "paper proposes",
    "paper presents",
    "author copy",
    "accepted manuscript",
    "all rights reserved",
    "permission to make",
];

const LOW_SIGNAL_SINGLE_WORD_TAGS: &[&str] = &[
    "action",
    "actions",
    "data",
    "function",
    "functions",
    "label",
    "labels",
    "method",
    "methods",
    "network",
    "networks",
    "paper",
    "papers",
    "result",
    "results",
    "section",
    "sections",
    "study",
    "studies",
    "system",
    "systems",
    "user",
    "users",
    "work",
    "works",
];

#[cfg(test)]
mod tests {
    use super::{
        build_auto_tags, extract_declared_keywords, normalize_tag_name, sanitize_filename,
        summarize_bibtex, validate_project_visibility,
    };

    #[test]
    fn normalizes_tag_names() {
        assert_eq!(
            normalize_tag_name("  edge   computing ").as_deref(),
            Some("edge computing")
        );
        assert_eq!(normalize_tag_name("x"), None);
    }

    #[test]
    fn sanitizes_uploaded_filenames() {
        assert_eq!(sanitize_filename("paper draft.pdf"), "paper_draft.pdf");
        assert_eq!(
            sanitize_filename("unsafe/../../name"),
            "unsafe_.._.._name.pdf"
        );
    }

    #[test]
    fn summarizes_bibtex_keywords_when_present() {
        let source = "@article{demo, title={Sample}, author={Doe, Jane}, year={2026}, keywords={Networks; Telemetry}}";
        let summary = summarize_bibtex(source).unwrap();
        assert_eq!(summary.citation_key.as_deref(), Some("demo"));
        assert_eq!(
            summary.keywords,
            vec!["Networks".to_string(), "Telemetry".to_string()]
        );
    }

    #[test]
    fn builds_auto_tags_from_keywords_and_text() {
        let tags = build_auto_tags(
            &["Programmable data planes".to_string()],
            "Abstract This paper studies telemetry. Keywords: packet replication, user plane. telemetry telemetry networks programmable",
        );
        let names = tags.into_iter().map(|(name, _)| name).collect::<Vec<_>>();
        assert_eq!(names, vec!["Programmable data planes".to_string()]);
    }

    #[test]
    fn rejects_numeric_and_low_signal_auto_tags() {
        let tags = build_auto_tags(
            &["004".to_string(), "data".to_string(), "5G".to_string()],
            "the packet packet replication user plane packet replication",
        );
        let names = tags.into_iter().map(|(name, _)| name).collect::<Vec<_>>();
        assert!(!names.iter().any(|name| name == "004"));
        assert!(!names.iter().any(|name| name == "data"));
        assert_eq!(names, vec!["5G".to_string()]);
    }

    #[test]
    fn extracts_declared_keywords_after_abstract() {
        let keywords = extract_declared_keywords(
            "Abstract This paper studies packet replication in user planes.\nKeywords: packet replication, user plane, 004\n1 INTRODUCTION",
        );
        assert_eq!(
            keywords,
            vec!["packet replication".to_string(), "user plane".to_string()]
        );
    }

    #[test]
    fn prefers_declared_keywords_over_full_text_fallback() {
        let tags = build_auto_tags(
            &[],
            "Abstract This paper studies packet replication in user planes.\nKeywords: packet replication, user plane\nThe data plane data plane telemetry telemetry stack stack framework framework appears many times.",
        );
        let names = tags.into_iter().map(|(name, _)| name).collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["packet replication".to_string(), "user plane".to_string()]
        );
    }

    #[test]
    fn validates_project_visibility_values() {
        assert_eq!(validate_project_visibility("public").unwrap(), "public");
        assert_eq!(validate_project_visibility("private").unwrap(), "private");
        assert!(validate_project_visibility("internal").is_err());
    }
}
