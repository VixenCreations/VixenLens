mod config;
mod db;
mod model;

use db::*;
use tauri::Manager;

use config::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            add_folder,
            get_all_folders,
            delete_folder,
            search_files_in_folders,
            scan_and_register_images,
						get_thumbnails_chunk,
            get_image_metadata,
            scan_and_register_images_with_progress,
            get_config,
            set_config,
            add_ignore_folder,
            delete_ignore_folder,
            get_all_ignore_folders,
            search_images
        ])
        .setup(|app| {
            let app_handle = app.handle();

            // Ensure config exists and is readable at startup
            if let Err(e) = get_config(app_handle.clone()) {
                panic!("Failed to load config at startup: {e}");
            }

            // Initialize database AFTER config exists
            init_db(&app_handle)
                .expect("Failed to initialize database");

            #[cfg(debug_assertions)]
            app.get_webview_window("main")
                .unwrap()
                .open_devtools();

            println!("Tauri application is starting!");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
