CREATE TABLE IF NOT EXISTS user_identities (
    provider TEXT NOT NULL,
    provider_subject TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (provider, provider_subject)
);

CREATE INDEX IF NOT EXISTS user_identities_user_id_idx ON user_identities (user_id);

INSERT INTO user_identities (provider, provider_subject, user_id)
SELECT 'gitlab', gitlab_user_id::TEXT, id
FROM users
WHERE gitlab_user_id IS NOT NULL
ON CONFLICT (provider, provider_subject) DO NOTHING;

ALTER TABLE users ALTER COLUMN gitlab_user_id DROP NOT NULL;
