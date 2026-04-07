use crate::model::search::SearchFolder;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use image::GenericImageView;
use std::collections::HashMap;
// サムネイル生成用
use chrono::{DateTime, Utc};
use png::Decoder;
use rusqlite::{params, params_from_iter, Connection, OpenFlags, OptionalExtension, Result};
use serde_json::{Number, Value};
use std::fs;
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tokio::task;
use walkdir::WalkDir;

// Parallel Dependacies
use rayon::prelude::*;

// Queries構造体をインポート
mod query;
use query::Queries;

// グローバルでクエリを一度読み込む
lazy_static::lazy_static! {
    static ref SQL_QUERIES: Queries = Queries::load();
}

//Initialize Database
pub fn init_db(app: &AppHandle) -> Result<Connection, rusqlite::Error> {
    let local_data_dir = app
        .path()
        .local_data_dir()
        .expect("local_data_dir unavailable");

    let app_root = local_data_dir.join("VRCX PhotoSearch");
    let db_root = app_root.join("database");
    let index_root = db_root.join("indexes");

    std::fs::create_dir_all(&db_root)
        .expect("Failed to create database root directory");
    std::fs::create_dir_all(&index_root)
        .expect("Failed to create index directory");

    let db_path = db_root.join("main.db");

    eprintln!("[db:init] main_db = {}", db_path.display());
    eprintln!("[db:init] index_dir = {}", index_root.display());

    let conn = Connection::open(&db_path)?;
    conn.execute_batch(SQL_QUERIES.create_tables)?;

    Ok(conn)
}

fn normalize_path(p: &Path) -> String {
    let mut s = p
        .canonicalize()
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .to_string();

    s = s.replace('/', "\\");

    if !s.ends_with('\\') {
        s.push('\\');
    }

    s
}

fn prune_index_cache(
    app: &AppHandle,
    main: &Connection,
) -> Result<(), String> {
    let mut index = connect_index_db(app, "imageCache")
        .map_err(|e| e.to_string())?;

    // 1. Load active folders from MAIN DB
    let mut stmt = main
        .prepare("SELECT path FROM search_folders")
        .map_err(|e| e.to_string())?;

    let prefixes: Vec<String> = stmt
        .query_map([], |row| {
            let raw: String = row.get(0)?;
            Ok(normalize_path(Path::new(&raw)))
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

		// No folders → do nothing (scan will repopulate)
		if prefixes.is_empty() {
				eprintln!("[db:prune] skipped (no active folders)");
				return Ok(());
		}

    // 2. Build DELETE query
    let mut sql = String::from("DELETE FROM images WHERE NOT (");
    sql.push_str(
        &prefixes
            .iter()
            .enumerate()
            .map(|(i, _)| format!("file_path LIKE ?{}", i + 1))
            .collect::<Vec<_>>()
            .join(" OR "),
    );
    sql.push(')');

    let like_params: Vec<String> =
        prefixes.into_iter().map(|p| format!("{}%", p)).collect();

    // 3. Execute atomically
    let tx = index.transaction().map_err(|e| e.to_string())?;
    tx.execute(&sql, params_from_iter(like_params))
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}

fn db_roots(app: &AppHandle) -> (PathBuf, PathBuf) {
    let base = app
        .path()
        .local_data_dir()
        .expect("local_data_dir unavailable")
        .join("VRCX PhotoSearch")
        .join("database");

    let indexes = base.join("indexes");

    (base, indexes)
}

fn connect_index_db(app: &AppHandle, uuid: &str) -> Result<Connection> {
    let (_, index_root) = db_roots(app);

    create_dir_all(&index_root)
        .expect("Failed to create index database directory");

    let db_path = index_root.join(format!("{uuid}.db"));

    let conn = Connection::open(&db_path)?;
    conn.execute_batch(SQL_QUERIES.create_sub_index)?;
    Ok(conn)
}

fn connect_index_db_ro(app: &AppHandle, uuid: &str) -> Result<Connection> {
    let (_, index_root) = db_roots(app);
    let db_path = index_root.join(format!("{uuid}.db"));
		let uuid = "imageCache";

    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    Ok(conn)
}

#[tauri::command]
pub fn add_folder(app: AppHandle, path: String) -> Result<(), String> {
    let conn = init_db(&app).map_err(|e| e.to_string())?;

    // 1) Try to fetch existing UUID
    let existing: Option<String> = conn
        .query_row(
            "SELECT uuid FROM search_folders WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        // Folder already exists → reuse UUID
        return Ok(());
    }

    // 2) Insert only if missing
    let uuid = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO search_folders (path, uuid) VALUES (?1, ?2)",
        params![path, uuid],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn add_ignore_folder(app: AppHandle, path: String) -> Result<(), String> {
    let conn = init_db(&app).map_err(|e| e.to_string())?;

    let existing: Option<String> = conn
        .query_row(
            "SELECT uuid FROM ignore_folders WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Ok(());
    }

    let uuid = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO ignore_folders (path, uuid) VALUES (?1, ?2)",
        params![path, uuid],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_all_folders(app: AppHandle) -> Result<Vec<SearchFolder>, String> {
    let conn = init_db(&app).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(SQL_QUERIES.select_all_folders) // クエリを使用
        .map_err(|e| e.to_string())?;

    let folders = stmt
        .query_map([], |row| {
            Ok(SearchFolder {
                id: row.get(0)?,
                path: row.get(1)?,
                uuid: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())? // エラー処理
        .filter_map(Result::ok)
        .collect();

    Ok(folders)
}

#[tauri::command]
pub fn get_all_ignore_folders(app: AppHandle) -> Result<Vec<SearchFolder>, String> {
    let conn = init_db(&app).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(SQL_QUERIES.select_all_ignore_folders) // クエリを使用
        .map_err(|e| e.to_string())?;

    let folders = stmt
        .query_map([], |row| {
            Ok(SearchFolder {
                id: row.get(0)?,
                path: row.get(1)?,
                uuid: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())? // エラー処理
        .filter_map(Result::ok)
        .collect();

    Ok(folders)
}

#[tauri::command]
pub fn delete_folder(app: AppHandle, id: i32) -> Result<(), String> {
    let conn = init_db(&app).map_err(|e| e.to_string())?;

    let uuid: String = conn
        .query_row(
            "SELECT uuid FROM search_folders WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Remove folder from MainDB
    conn.execute(SQL_QUERIES.delete_folder, params![id])
        .map_err(|e| e.to_string())?;

    // Prune index cache rows
    let index_conn = connect_index_db(&app, "imageCache")
        .map_err(|e| e.to_string())?;

    index_conn
        .execute("DELETE FROM images WHERE uuid = ?1", params![uuid])
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn delete_ignore_folder(app: AppHandle, id: i32) -> Result<(), String> {
    let conn = init_db(&app).map_err(|e| e.to_string())?;

    // Remove ignore folder from MainDB
    conn.execute(
        SQL_QUERIES.delete_ignore_folder,
        params![id],
    )
    .map_err(|e| e.to_string())?;

    // Mark cache dirty (soft invalidation)
    // This forces next scan to re-evaluate ignored paths
    conn.execute(
        "UPDATE app_state SET value = 'true' WHERE key = 'index_dirty'",
        [],
    ).ok(); // best-effort, do not hard fail

    Ok(())
}


//Search Files for Data Specified
#[tauri::command]
pub async fn search_files_in_folders(app: AppHandle) -> Result<Vec<(String, String)>, String> {
    // Move heavy work off the async executor thread
    tauri::async_runtime::spawn_blocking(move || {
        let conn = init_db(&app).map_err(|e| e.to_string())?;

        // Folders
        let mut stmt = conn
            .prepare(SQL_QUERIES.select_all_folders)
            .map_err(|e| e.to_string())?;

        let paths: Vec<(String, String)> = stmt
            .query_map([], |row| {
                let folder: String = row.get(1)?;
                let uuid: String = row.get(2)?;
                Ok((folder, uuid))
            })
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();

        // Ignore folders
        stmt = conn
            .prepare(SQL_QUERIES.select_all_ignore_folders)
            .map_err(|e| e.to_string())?;

        let ignore_paths_raw: Vec<(String, String)> = stmt
            .query_map([], |row| {
                let folder: String = row.get(1)?;
                let uuid: String = row.get(2)?;
                Ok((folder, uuid))
            })
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();

        // Precompute ignore list ONCE (no per-file allocations)
        let ignore_prefixes: Vec<PathBuf> = ignore_paths_raw
            .iter()
            .map(|p| PathBuf::from(&p.0))
            .collect();

        // Protect UI + IPC: cap results (tune as needed)
        const MAX_RESULTS: usize = 10_000;

        let mut all_files: Vec<(String, String)> = Vec::new();

        for (path, uuid) in paths {
            // Skip missing/unmounted folders quickly
            if !Path::new(&path).exists() {
                continue;
            }

            for entry in WalkDir::new(&path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();

                if !p.is_file() {
                    continue;
                }
                if !is_image_file(p) {
                    continue;
                }
								if is_ignored(p, &ignore_prefixes) {
										continue;
								}

                all_files.push((p.to_string_lossy().to_string(), uuid.clone()));

                if all_files.len() >= MAX_RESULTS {
                    return Ok(all_files);
                }
            }
        }

        Ok(all_files)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ignore check: treat ignore folders as path prefixes
fn is_ignored(file_path: &Path, ignore_prefixes: &[PathBuf]) -> bool {
    ignore_prefixes.iter().any(|prefix| file_path.starts_with(prefix))
}

/// The program searches for image files in the specified folder, generates thumbnails, and registers them in the database.
#[tauri::command]
pub fn scan_and_register_images(app: AppHandle) -> Result<(), String> {
    let conn = init_db(&app).map_err(|e| e.to_string())?;

    // 検索フォルダを取得
    let mut stmt = conn
        .prepare(SQL_QUERIES.select_all_folders)
        .map_err(|e| e.to_string())?;

    let folders: Vec<(String, String)> = stmt
        .query_map([], |row| {
            let folder: String = row.get(1)?; // フォルダパス
            let uuid: String = row.get(2)?;   // UUID
            Ok((folder, uuid))
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    // 除外フォルダを取得（prefix list）
    let mut stmt = conn
        .prepare(SQL_QUERIES.select_all_ignore_folders)
        .map_err(|e| e.to_string())?;

    let ignore_prefixes: Vec<PathBuf> = stmt
        .query_map([], |row| {
            let folder: String = row.get(1)?; // フォルダパス
            Ok(PathBuf::from(folder))
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    // 各フォルダ内を再帰探索
    for (folder, uuid) in folders {
        let root = PathBuf::from(&folder);

        // 早期スキップ（存在しない/ディレクトリではない）
        if !root.is_dir() {
            continue;
        }

        for entry in WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();

            // Filter cheap-first
            if !p.is_file() {
                continue;
            }
            if is_ignored(p, &ignore_prefixes) {
                continue;
            }
            if !is_image_file(p) {
                continue;
            }

            match process_image_file(&app, p, &uuid) {
                Ok(_) => println!("登録成功: {:?}", p),
                Err(e) => eprintln!("登録失敗: {:?}, エラー: {}", p, e),
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn scan_and_register_images_with_progress(
    app: AppHandle,
    folder_list: Vec<String>,
    event_id: String,
) -> Result<(), String> {
    let con = init_db(&app).map_err(|e| e.to_string())?;
    let conn = Arc::new(Mutex::new(con));

    //ONE index DB per scan (stops the DB explosion)
    let scan_uuid = "imageCache".to_string();

		// After folder_list empty check
    if folder_list.is_empty() {
        return Err("フォルダがありません。".to_string());
    }
		
		// Prune index cache ONCE per scan
		{
				let conn = init_db(&app).map_err(|e| e.to_string())?;
				if let Err(e) = prune_index_cache(&app, &conn) {
						eprintln!("[scan][prune] {}", e);
				}
		}

    let placeholders = folder_list
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");

    //Dedupe folder → uuid (still useful for folder selection sanity)
    use std::collections::HashMap as StdHashMap;
    let mut folder_uuid_map: StdHashMap<String, String> = StdHashMap::new();

    conn.lock()
        .await
        .prepare(&format!(
            "SELECT path, uuid FROM search_folders WHERE path IN ({})",
            placeholders
        ))
        .map_err(|e| e.to_string())?
        .query_map(params_from_iter(folder_list.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .for_each(|(path, uuid)| {
            folder_uuid_map.entry(path).or_insert(uuid);
        });

    // PRECOMPUTE ignore prefixes ONCE
    let ignore_prefixes: Vec<PathBuf> = conn
        .lock()
        .await
        .prepare(SQL_QUERIES.select_all_ignore_folders)
        .map_err(|e| e.to_string())?
        .query_map([], |row| {
            let folder: String = row.get(1)?;
            Ok(PathBuf::from(folder))
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    // ─────────────────────────────────────────────
    // Pass 1: build file list + total
    // ─────────────────────────────────────────────
    let mut total: u32 = 0;
    let mut all_image_files: Vec<PathBuf> = Vec::new();

    for (folder, _uuid) in folder_uuid_map {
        let path = PathBuf::from(&folder);
        if !path.is_dir() {
            continue;
        }

        let mut image_files: Vec<PathBuf> = WalkDir::new(&path)
            .into_iter()
            .par_bridge()
            .filter_map(Result::ok)
            .filter(|entry| {
                let p = entry.path();
                p.is_file() && is_image_file(p) && !is_ignored(p, &ignore_prefixes)
            })
            .map(|entry| entry.into_path())
            .collect();

        total = total.saturating_add(image_files.len() as u32);
        all_image_files.append(&mut image_files);
    }

    // Emit scan start (ONCE)
    app.emit(
        "scan_progress",
        &Value::Object(
            vec![
                ("event_id".to_string(), Value::String(event_id.clone())),
                ("processed".to_string(), Value::Number(Number::from(0u32))),
                ("total".to_string(), Value::Number(Number::from(total))),
                ("progress".to_string(), Value::Number(Number::from(0u32))),
                ("message".to_string(), Value::String("スキャン開始...".to_string())),
            ]
            .into_iter()
            .collect(),
        ),
    )
    .ok();

    // ─────────────────────────────────────────────
    // Pass 2: process EVERYTHING in ONE index DB
    // (no per-folder loop => no per-folder db files)
    // ─────────────────────────────────────────────
    let processed: u32 = match process_images_in_transaction_async(
        all_image_files,
        scan_uuid.clone(),
        Arc::new(app.clone()),
    )
    .await
    {
        Ok(count) => count,
        Err(e) => {
            app.emit(
                "scan_progress",
                &Value::Object(
                    vec![
                        ("event_id".to_string(), Value::String(event_id.clone())),
                        ("processed".to_string(), Value::Number(Number::from(0u32))),
                        ("total".to_string(), Value::Number(Number::from(total))),
                        ("progress".to_string(), Value::Number(Number::from(0u32))),
                        (
                            "message".to_string(),
                            Value::String(format!("スキャン失敗: {}", e)),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )
            .ok();
            return Err(e);
        }
    };

    // Emit final progress
    app.emit(
        "scan_progress",
        &Value::Object(
            vec![
                ("event_id".to_string(), Value::String(event_id.clone())),
                ("processed".to_string(), Value::Number(Number::from(processed))),
                ("total".to_string(), Value::Number(Number::from(total))),
                ("progress".to_string(), Value::Number(Number::from(processed))),
                ("message".to_string(), Value::String("スキャン完了".to_string())),
            ]
            .into_iter()
            .collect(),
        ),
    )
    .ok();

    Ok(())
}

/// ファイルが画像かどうかを判定
fn is_image_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        matches!(
            ext.to_string_lossy().to_lowercase().as_str(),
            "jpg" | "jpeg" | "png"
        )
    } else {
        false
    }
}

fn process_image_file(app: &AppHandle, file_path: &Path, uuid: &str) -> Result<(), String> {
    let conn = connect_index_db(app, uuid).expect("sub index db not found");

    let db_meta = conn
        .prepare("SELECT file_size,length(thumbnail),updated_at FROM images WHERE file_path = ?")
        .map_err(|e| e.to_string())?
        .query_row([file_path.to_string_lossy().to_string()], |row| {
            let file_size: i32 = row.get(0)?;
            let thumbnail_size: i32 = row.get(1)?;
            let updated_at: String = row.get(2)?;
            Ok((file_size, thumbnail_size, updated_at))
        })
        .optional()
        .map_err(|e| e.to_string());

    let metadata = fs::metadata(file_path).map_err(|e| e.to_string())?;
    let file_size = metadata.len() as i32;
    match db_meta {
        Ok(Some(db_meta)) => {
            if file_size == db_meta.0
                && db_meta.1 > 0
                && SystemTime::UNIX_EPOCH
                    .checked_add(Duration::from_secs(
                        db_meta.2.parse::<DateTime<Utc>>().unwrap().timestamp() as u64,
                    ))
                    .unwrap()
                    <= metadata.modified().unwrap()
            {
                return Ok(());
            }
        }
        Ok(None) => {}
        Err(e) => return Err(e.to_string()),
    }

    // サムネイル生成
    let thumbnail = generate_thumbnail(app, file_path).map_err(|e| e.to_string())?;

    // 画像の幅と高さを取得
    let image = image::open(file_path).map_err(|e| e.to_string())?;
    let (width, height) = image.dimensions();

    // iTXtチャンクからメタデータを取得
    let metadata_json = extract_metadata(file_path).unwrap_or(None);
    let file_created_at_time: DateTime<Utc> = fs::metadata(file_path)
        .map_err(|e| {
            format!(
                "Failed to get metadata for file {}: {}",
                file_path.to_string_lossy(),
                e
            )
        })?
        .created()
        .map_err(|e| {
            format!(
                "Failed to get created date for file {}: {}",
                file_path.to_string_lossy(),
                e
            )
        })?
        .into();
    let file_created_at = file_created_at_time.to_rfc3339();
    // 現在時刻を取得
    let created_at = Utc::now().to_rfc3339();
    let updated_at = created_at.clone();

    // 保存処理を実行
    conn.execute(
        SQL_QUERIES.insert_image,
        params![
            file_path.to_string_lossy().to_string(),
            thumbnail,
            width as i32,
            height as i32,
            file_size,
            metadata_json.unwrap_or_default(), // JSONデータがない場合は空文字列
            file_created_at,
            created_at,
            updated_at,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

async fn process_images_in_transaction_async(
    file_paths: Vec<PathBuf>,
    uuid: String,
    app: Arc<AppHandle>,
) -> std::result::Result<u32, String> {
    let transaction_result = task::spawn_blocking(move || {
        // Tune this: 250 is safe, 500 is faster on NVMe
        let chunk_size: usize = 250;

        // Reuse ONE connection (huge win)
        let mut conn = connect_index_db(app.as_ref(), &uuid).map_err(|e| e.to_string())?;

        let scan_start = Instant::now();
        let mut inserted_or_updated: u32 = 0;

        // Optional: show some file samples per chunk (keep low)
        const PRINT_SAMPLES_PER_CHUNK: usize = 5;

        println!(
            "[scan][{}] start: files={} chunk_size={}",
            uuid,
            file_paths.len(),
            chunk_size
        );

        for (chunk_index, chunk) in file_paths.chunks(chunk_size).enumerate() {
            let chunk_start = Instant::now();

            // ─────────────────────────────────────────────
            // 1) Batch fetch existing db metadata for chunk
            // ─────────────────────────────────────────────
            let mut existing: HashMap<String, (i32, i32, String)> = HashMap::new();

            if !chunk.is_empty() {
                let placeholders = std::iter::repeat("?")
                    .take(chunk.len())
                    .collect::<Vec<_>>()
                    .join(", ");

                let sql = format!(
                    "SELECT file_path, file_size, length(thumbnail), updated_at
                     FROM images
                     WHERE file_path IN ({})",
                    placeholders
                );

                let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
                let rows = stmt
                    .query_map(
                        params_from_iter(chunk.iter().map(|p| p.to_string_lossy().to_string())),
                        |row| {
                            let file_path: String = row.get(0)?;
                            let file_size: i32 = row.get(1)?;
                            let thumb_len: i32 = row.get(2)?;
                            let updated_at: String = row.get(3)?;
                            Ok((file_path, (file_size, thumb_len, updated_at)))
                        },
                    )
                    .map_err(|e| e.to_string())?;

                for r in rows {
                    if let Ok((k, v)) = r {
                        existing.insert(k, v);
                    }
                }
            }

            // ─────────────────────────────────────────────
            // 2) Decide which files actually need work
            // ─────────────────────────────────────────────
            let mut to_process: Vec<PathBuf> = Vec::with_capacity(chunk.len());

            for file_path in chunk {
                let path_str = file_path.to_string_lossy().to_string();

                let meta = match fs::metadata(file_path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let file_size = meta.len() as i32;

                if let Some((db_size, db_thumb_len, db_updated_at)) = existing.get(&path_str) {
                    if file_size == *db_size && *db_thumb_len > 0 {
                        if let Ok(parsed) = db_updated_at.parse::<DateTime<Utc>>() {
                            if let Ok(modified) = meta.modified() {
                                let db_time = SystemTime::UNIX_EPOCH
                                    .checked_add(Duration::from_secs(parsed.timestamp() as u64))
                                    .unwrap_or(SystemTime::UNIX_EPOCH);

                                if db_time <= modified {
                                    continue; // unchanged → skip
                                }
                            }
                        }
                    }
                }

                to_process.push(file_path.to_path_buf());
            }

            if to_process.is_empty() {
                // stdout: cheap progress line
                let elapsed = chunk_start.elapsed();
                println!(
                    "[scan][{}] chunk={} skipped_all ({}ms) total_indexed={}",
                    uuid,
                    chunk_index,
                    elapsed.as_millis(),
                    inserted_or_updated
                );
                continue;
            }

            // ─────────────────────────────────────────────
            // 3) CPU-heavy work in parallel (Rayon)
            // ─────────────────────────────────────────────
            #[derive(Clone)]
            struct RowData {
                file_path: String,
                thumbnail_b64: String,
                width: i32,
                height: i32,
                file_size: i32,
                metadata_json: String,
                file_created_at: String,
                created_at: String,
                updated_at: String,
            }

            let now = Utc::now().to_rfc3339();

						let rows: Vec<RowData> = to_process
								.par_iter()
								.filter_map(|file_path| {
										let meta = fs::metadata(file_path).ok()?;
										let file_size = meta.len() as i32;

										// decode once
										let img = image::open(file_path).ok()?;
										let (w, h) = img.dimensions();

										// thumbnail from same decoded image
										let thumb = img.thumbnail(256, 256);

										let mut buffer = Vec::new();
										thumb.write_to(
												&mut std::io::Cursor::new(&mut buffer),
												image::ImageFormat::Png,
										).ok()?;

										let thumbnail_b64 = STANDARD.encode(&buffer);

										let metadata_json = extract_metadata(file_path)
												.unwrap_or(None)
												.unwrap_or_default();

										let file_created_at = meta
												.created()
												.ok()
												.map(|t| DateTime::<Utc>::from(t).to_rfc3339())
												.unwrap_or_default();

										Some(RowData {
												file_path: file_path.to_string_lossy().to_string(),
												thumbnail_b64,
												width: w as i32,
												height: h as i32,
												file_size,
												metadata_json,
												file_created_at,
												created_at: now.clone(),
												updated_at: now.clone(),
										})
								})
								.collect();

            if rows.is_empty() {
                let elapsed = chunk_start.elapsed();
                println!(
                    "[scan][{}] chunk={} processed=0 ({}ms) total_indexed={}",
                    uuid,
                    chunk_index,
                    elapsed.as_millis(),
                    inserted_or_updated
                );
                continue;
            }

            // stdout samples (limited, avoids perf cliff)
            for sample in rows.iter().take(PRINT_SAMPLES_PER_CHUNK) {
                println!("[scan][{}] + {}", uuid, sample.file_path);
            }

            // ─────────────────────────────────────────────
            // 4) Single transaction for the whole chunk
            // ─────────────────────────────────────────────
            let tx = conn.transaction().map_err(|e| e.to_string())?;

            let mut insert_stmt = tx
                .prepare(SQL_QUERIES.insert_image)
                .map_err(|e| e.to_string())?;

            for r in rows {
                insert_stmt
                    .execute(params![
                        r.file_path,
                        r.thumbnail_b64,
                        r.width,
                        r.height,
                        r.file_size,
                        r.metadata_json,
                        r.file_created_at,
                        r.created_at,
                        r.updated_at,
                    ])
                    .map_err(|e| e.to_string())?;

                inserted_or_updated += 1;
            }

            drop(insert_stmt);
            tx.commit().map_err(|e| e.to_string())?;

            // stdout: chunk summary
            let elapsed = chunk_start.elapsed();
            let ms = elapsed.as_millis().max(1) as u128;
            let rate = (chunk_size as u128 * 1000) / ms; // approximate
            println!(
                "[scan][{}] chunk={} indexed~{} ({}ms ~{} files/s) total_indexed={}",
                uuid,
                chunk_index,
                chunk_size.min(chunk.len()),
                elapsed.as_millis(),
                rate,
                inserted_or_updated
            );
        }

        println!(
            "[scan][{}] done: indexed={} elapsed={}ms",
            uuid,
            inserted_or_updated,
            scan_start.elapsed().as_millis()
        );

        Ok::<u32, String>(inserted_or_updated)
    })
    .await
    .map_err(|e| format!("トランザクションエラー: {:?}", e))?;

    transaction_result
}

fn extract_metadata(file_path: &Path) -> Result<Option<String>, String> {
    // ファイルを開く
    let file = File::open(file_path).map_err(|e| format!("ファイルを開けませんでした: {}", e))?;

    // PNGデコーダーを作成
    let decoder = Decoder::new(file);
    let reader = decoder
        .read_info()
        .map_err(|e| format!("PNG解析エラー: {}", e))?;

    // PNGのメタデータチャンク（iTXt）を検索
    if let Some(itxt) = reader
        .info()
        .utf8_text
        .iter()
        .find(|t| t.keyword == "Description")
    {
        // iTXtの内容を取得
        let data = itxt.get_text().unwrap_or("".parse().unwrap());
        // JSONとしてパース可能な場合はパース
        match serde_json::from_str::<Value>(&data) {
            Ok(parsed_json) => {
                Ok(Some(parsed_json.to_string())) // JSON文字列として返却
            }
            Err(_) => {
                // JSONでない場合は元データをそのまま返す
                Ok(Some(data.clone()))
            }
        }
    } else {
        // iTXtが存在しない場合
        Ok(None)
    }
}

/// サムネイル生成
fn generate_thumbnail(_app: &AppHandle, file_path: &Path) -> Result<Vec<u8>> {
    let image = image::open(file_path)
        .map_err(|_| "画像を開けませんでした")
        .unwrap();
    let thumbnail = image.thumbnail(256, 256); // サムネイルサイズは256に縮小

    let mut buffer = Vec::new();
    thumbnail
        .write_to(
            &mut std::io::Cursor::new(&mut buffer),
            image::ImageFormat::Png,
        )
        .map_err(|_| "サムネイルのエンコードに失敗しました")
        .expect("");
    //TODO: ERR
    Ok(buffer)
}

#[tauri::command]
pub fn get_image_metadata(
    app: AppHandle,
    uuid: String,
    file_path: String,
) -> Result<Value, String> {
    // ファイルの存在をチェック
    let path = Path::new(&file_path);
    if !path.exists() || !path.is_file() {
        return Err("指定されたファイルが存在しません。".to_string());
    }

    // UUIDに基づきデータベースに接続
    let conn = connect_index_db(&app, &uuid).map_err(|e| {
        let msg = format!(
            "サブデータベース接続エラー: {} (UUID: {}, Path: {:?})",
            e,
            uuid,
            app.path().app_data_dir()
        );
        msg
    })?;

    // DBからメタデータ取得
    let (metadata_json, file_created_at): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT metadata_json, file_created_at FROM images WHERE file_path = ?",
            params![file_path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("クエリエラー: {}", e))?
        .unwrap_or((None, None));

    if let Some(metadata) = metadata_json {
        // ファイルを読み取りBase64エンコード
        return match fs::read(path) {
            Ok(file_data) => {
                let base64_content = STANDARD.encode(&file_data);
                let mime_type = mime_guess::from_path(&file_path)
                    .first_or_octet_stream()
                    .essence_str()
                    .to_string();
                let data_url = format!("data:{};base64,{}", mime_type, base64_content);

                let metadata_value: Value = serde_json::from_str(&metadata).unwrap_or(Value::Null);
                let result = Value::Object(
                    vec![
                        ("metadata".to_string(), metadata_value),
                        ("data_url".to_string(), Value::String(data_url)),
                        (
                            "file_created_at".to_string(),
                            Value::String(file_created_at.unwrap_or("".to_string())),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                );
                Ok(result)
            }
            Err(e) => Err(format!("ファイル読み取りエラー: {}", e)),
        };
    }
    Err("指定されたファイルのメタデータが見つかりません。".to_string())
}

#[tauri::command]
pub fn get_thumbnails_chunk(
    app: AppHandle,
    offset: i64,
    limit: i64,
) -> Result<Vec<(String, String, String)>, String> {
    // Single unified index DB
    const INDEX_DB: &str = "imageCache";

    let conn = connect_index_db_ro(&app, INDEX_DB).map_err(|e| {
        format!("Unable to open index database (imageCache): {}", e)
    })?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                file_path,
                thumbnail
            FROM images
            WHERE thumbnail IS NOT NULL
              AND thumbnail != ''
            ORDER BY file_created_at DESC
            LIMIT ?1 OFFSET ?2
            "#,
        )
        .map_err(|e| format!("Failed to prepare thumbnail query: {}", e))?;

    let rows = stmt
        .query_map(params![limit, offset], |row| {
            let file_path: String = row.get(0)?;
            let thumbnail_b64: String = row.get(1)?;

            Ok((
                file_path,
                format!("data:image/png;base64,{}", thumbnail_b64),
                INDEX_DB.to_string(), // hard-locked
            ))
        })
        .map_err(|e| format!("Failed to query thumbnails: {}", e))?;

    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn search_images(
    app: AppHandle,
    conditions: Vec<HashMap<String, String>>,
) -> Result<Vec<(String, String, String)>, String> {
    let _ = init_db(&app).map_err(|e| e.to_string())?;

    // Single shared index DB (matches scan logic)
    let scan_uuid = "imageCache".to_string();

    // ─────────────────────────────────────────────
    // Build SQL query dynamically
    // ─────────────────────────────────────────────
    let mut query = String::from(
        "SELECT DISTINCT file_path, thumbnail
         FROM images
         WHERE json_valid(metadata_json) = 1",
    );

    let mut params: Vec<String> = Vec::new();
    let mut segments: Vec<String> = Vec::new();
    let mut player_alias_idx = 0;

    for condition in conditions {
        let logic = match condition.get("logic").map(|s| s.to_uppercase()).as_deref() {
            Some("OR") => "OR",
            _ => "AND",
        };

        let operator = match condition.get("operator").map(|s| s.to_uppercase()).as_deref() {
            Some("NE") => "!=",
            Some("GT") => ">",
            Some("GE") => ">=",
            Some("LT") => "<",
            Some("LE") => "<=",
            Some("LIKE") => "LIKE",
            _ => "=",
        };

        let field = condition.get("field").map(|s| s.as_str()).unwrap_or("");
        let value = condition.get("value").map(|s| s.as_str()).unwrap_or("");
        if value.is_empty() {
            continue;
        }

        let mut segment = match field {
            // ───────────── Players ─────────────
            "player" | "player_name" => {
                player_alias_idx += 1;
                let alias = format!("p{}", player_alias_idx);
                format!(
                    "EXISTS (
                        SELECT 1 FROM json_each(metadata_json, '$.players') AS {alias}
                        WHERE json_extract({alias}.value, '$.displayName') {op} ?
                    )",
                    alias = alias,
                    op = operator
                )
            }

            "player_id" => {
                player_alias_idx += 1;
                let alias = format!("p{}", player_alias_idx);
                format!(
                    "EXISTS (
                        SELECT 1 FROM json_each(metadata_json, '$.players') AS {alias}
                        WHERE json_extract({alias}.value, '$.id') {op} ?
                    )",
                    alias = alias,
                    op = operator
                )
            }

            // ───────────── World ─────────────
            "world" | "world_name" =>
                format!("json_extract(metadata_json, '$.world.name') {op} ?", op = operator),

            "world_id" =>
                format!("json_extract(metadata_json, '$.world.id') {op} ?", op = operator),

            "instance_id" =>
                format!("json_extract(metadata_json, '$.world.instanceId') {op} ?", op = operator),

            // ───────────── Author ─────────────
            "author_id" =>
                format!("json_extract(metadata_json, '$.author.id') {op} ?", op = operator),

            "author_name" =>
                format!("json_extract(metadata_json, '$.author.displayName') {op} ?", op = operator),

            // ───────────── Meta ─────────────
            "application" =>
                format!("json_extract(metadata_json, '$.application') {op} ?", op = operator),

            "version" =>
                format!("json_extract(metadata_json, '$.version') {op} ?", op = operator),

            "timestamp" =>
                format!("json_extract(metadata_json, '$.timestamp') {op} ?", op = operator),

            // ───────────── Filesystem ─────────────
            "created_at" =>
                format!("file_created_at {op} ?", op = operator),

            // ───────────── Fallback ─────────────
            _ =>
                "file_path LIKE ?".to_string(),
        };

        let bind_value = if operator.eq_ignore_ascii_case("LIKE") {
            format!("%{}%", value)
        } else {
            value.to_string()
        };

        if !segments.is_empty() {
            segment = format!("{logic} {segment}");
        }

        segments.push(segment);
        params.push(bind_value);
    }

    if !segments.is_empty() {
        query.push_str(" AND (");
        query.push_str(&segments.join(" "));
        query.push(')');
    }

    // ─────────────────────────────────────────────
    // Execute against ONE index DB
    // ─────────────────────────────────────────────
    let conn = connect_index_db_ro(&app, &scan_uuid)
        .map_err(|e| format!("index db open failed: {}", e))?;

    let mut stmt = conn
        .prepare(&query)
        .map_err(|_| "検索クエリエラー".to_string())?;

    let rows = stmt
        .query_map(params_from_iter(params.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                format!("data:image/png;base64,{}", row.get::<_, String>(1)?),
                scan_uuid.clone(),
            ))
        })
        .map_err(|e| e.to_string())?;

    Ok(rows.filter_map(Result::ok).collect())
}
