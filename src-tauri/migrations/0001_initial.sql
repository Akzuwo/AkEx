PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS volumes (
  id INTEGER PRIMARY KEY,
  volume_id TEXT NOT NULL UNIQUE,
  root_path TEXT NOT NULL COLLATE NOCASE UNIQUE,
  label TEXT,
  filesystem_type TEXT,
  total_bytes INTEGER,
  free_bytes INTEGER,
  last_full_scan TEXT,
  last_change_checkpoint TEXT,
  index_status TEXT NOT NULL DEFAULT 'NotIndexed',
  last_error TEXT,
  entry_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS entries (
  id INTEGER PRIMARY KEY,
  parent_id INTEGER REFERENCES entries(id) ON DELETE CASCADE,
  volume_id INTEGER NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
  name TEXT NOT NULL COLLATE NOCASE,
  full_path TEXT NOT NULL COLLATE NOCASE,
  extension TEXT COLLATE NOCASE,
  is_directory INTEGER NOT NULL,
  size INTEGER NOT NULL DEFAULT 0,
  recursive_size INTEGER NOT NULL DEFAULT 0,
  created_at TEXT,
  modified_at TEXT,
  indexed_at TEXT NOT NULL,
  file_attributes INTEGER,
  hidden INTEGER NOT NULL DEFAULT 0,
  read_only INTEGER NOT NULL DEFAULT 0,
  system INTEGER NOT NULL DEFAULT 0,
  UNIQUE(volume_id, full_path)
);

CREATE INDEX IF NOT EXISTS idx_entries_name ON entries(name);
CREATE INDEX IF NOT EXISTS idx_entries_parent_id ON entries(parent_id, is_directory DESC, name);
CREATE INDEX IF NOT EXISTS idx_entries_full_path ON entries(full_path);
CREATE INDEX IF NOT EXISTS idx_entries_extension ON entries(extension);
CREATE INDEX IF NOT EXISTS idx_entries_size ON entries(size DESC);
CREATE INDEX IF NOT EXISTS idx_entries_recursive_size ON entries(recursive_size DESC);
CREATE INDEX IF NOT EXISTS idx_entries_modified_at ON entries(modified_at DESC);
CREATE INDEX IF NOT EXISTS idx_entries_volume ON entries(volume_id);

CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
  name,
  full_path,
  extension,
  content='entries',
  content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS entries_ai AFTER INSERT ON entries BEGIN
  INSERT INTO entries_fts(rowid, name, full_path, extension)
  VALUES (new.id, new.name, new.full_path, COALESCE(new.extension, ''));
END;

CREATE TRIGGER IF NOT EXISTS entries_ad AFTER DELETE ON entries BEGIN
  INSERT INTO entries_fts(entries_fts, rowid, name, full_path, extension)
  VALUES ('delete', old.id, old.name, old.full_path, COALESCE(old.extension, ''));
END;

CREATE TRIGGER IF NOT EXISTS entries_au AFTER UPDATE OF name, full_path, extension ON entries BEGIN
  INSERT INTO entries_fts(entries_fts, rowid, name, full_path, extension)
  VALUES ('delete', old.id, old.name, old.full_path, COALESCE(old.extension, ''));
  INSERT INTO entries_fts(rowid, name, full_path, extension)
  VALUES (new.id, new.name, new.full_path, COALESCE(new.extension, ''));
END;
