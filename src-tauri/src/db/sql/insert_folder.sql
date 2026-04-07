INSERT INTO search_folders (path, uuid)
VALUES (?, ?)
ON CONFLICT(path) DO NOTHING;