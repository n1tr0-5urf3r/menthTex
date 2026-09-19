CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    gitlab_user_id BIGINT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    username TEXT,
    display_name TEXT,
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS sessions_user_id_idx ON sessions (user_id);
CREATE INDEX IF NOT EXISTS sessions_expires_at_idx ON sessions (expires_at);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    owner_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS projects_owner_user_id_idx ON projects (owner_user_id);

CREATE TABLE IF NOT EXISTS project_memberships (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('owner', 'editor', 'viewer')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, user_id)
);

CREATE INDEX IF NOT EXISTS project_memberships_user_id_idx ON project_memberships (user_id);

CREATE TABLE IF NOT EXISTS folders (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    parent_folder_id TEXT REFERENCES folders(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    is_hidden BOOLEAN NOT NULL DEFAULT FALSE,
    created_by_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, parent_folder_id, name)
);

CREATE INDEX IF NOT EXISTS folders_project_id_idx ON folders (project_id);
CREATE INDEX IF NOT EXISTS folders_parent_folder_id_idx ON folders (parent_folder_id);

CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE RESTRICT,
    uploaded_by_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    original_filename TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    sha256 TEXT NOT NULL,
    title TEXT,
    authors_text TEXT,
    year TEXT,
    doi TEXT,
    abstract_text TEXT,
    source_metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    bibtex_source TEXT,
    bibtex_formatted TEXT,
    citation_key TEXT,
    processing_status TEXT NOT NULL CHECK (processing_status IN ('queued', 'ready', 'error')),
    processing_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS documents_project_id_idx ON documents (project_id);
CREATE INDEX IF NOT EXISTS documents_folder_id_idx ON documents (folder_id);
CREATE INDEX IF NOT EXISTS documents_sha256_idx ON documents (sha256);
CREATE INDEX IF NOT EXISTS documents_doi_idx ON documents (doi);

CREATE TABLE IF NOT EXISTS document_notes (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    author_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS document_notes_document_id_idx ON document_notes (document_id);

CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    created_by_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, normalized_name)
);

CREATE INDEX IF NOT EXISTS tags_project_id_idx ON tags (project_id);

CREATE TABLE IF NOT EXISTS document_tags (
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    source TEXT NOT NULL CHECK (source IN ('manual', 'auto')),
    confidence REAL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (document_id, tag_id)
);

CREATE INDEX IF NOT EXISTS document_tags_tag_id_idx ON document_tags (tag_id);
