CREATE TABLE IF NOT EXISTS artifact_file_links (
    artifact_id TEXT NOT NULL,
    path TEXT NOT NULL,
    relation TEXT NOT NULL,
    source TEXT NOT NULL,
    additions INTEGER,
    deletions INTEGER,
    staged INTEGER NOT NULL DEFAULT 0,
    untracked INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (artifact_id, path, relation)
);

CREATE INDEX IF NOT EXISTS idx_artifact_file_links_artifact
    ON artifact_file_links(artifact_id, relation, path);

CREATE INDEX IF NOT EXISTS idx_artifact_file_links_path
    ON artifact_file_links(path, relation, updated_at DESC);
