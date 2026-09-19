ALTER TABLE projects
ADD COLUMN IF NOT EXISTS visibility TEXT NOT NULL DEFAULT 'private'
CHECK (visibility IN ('public', 'private'));

UPDATE projects
SET visibility = 'private'
WHERE visibility IS NULL;
