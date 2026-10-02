// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod engine;
mod get_toij;
mod lsp;
mod toolchains;

#[tauri::command]
fn create_project(parent_path: String, name: String) -> Result<String, String> {
    let project_name = std::path::Path::new(&name);
    if name.trim().is_empty()
        || project_name.components().count() != 1
        || !matches!(
            project_name.components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return Err("專案名稱只能包含一般檔名字元。".into());
    }

    let project_path = std::path::Path::new(&parent_path).join(project_name);
    std::fs::create_dir_all(&project_path).map_err(|error| format!("無法建立專案：{error}"))?;
    let source_path = project_path.join("main.cpp");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&source_path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(b"#include <iostream>\n\nint main() {\n    return 0;\n}\n")
                .map_err(|error| format!("無法建立 main.cpp：{error}"))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("無法建立 main.cpp：{error}")),
    }
    Ok(project_path.to_string_lossy().into_owned())
}

#[tauri::command]
fn list_source_files(project_path: String) -> Result<Vec<String>, String> {
    fn collect(directory: &std::path::Path, files: &mut Vec<String>) -> Result<(), String> {
        for entry in
            std::fs::read_dir(directory).map_err(|error| format!("無法讀取專案資料夾：{error}"))?
        {
            let entry = entry.map_err(|error| format!("無法讀取專案檔案：{error}"))?;
            let file_type = entry
                .file_type()
                .map_err(|error| format!("無法取得檔案資訊：{error}"))?;
            if file_type.is_dir() {
                collect(&entry.path(), files)?;
            } else if file_type.is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(std::ffi::OsStr::to_str)
                    .is_some_and(|extension| {
                        matches!(
                            extension.to_ascii_lowercase().as_str(),
                            "cpp" | "cc" | "cxx" | "h" | "hpp"
                        )
                    })
            {
                files.push(entry.path().to_string_lossy().into_owned());
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    collect(std::path::Path::new(&project_path), &mut files)?;
    files.sort();
    Ok(files)
}

#[tauri::command]
fn read_source(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|error| format!("無法開啟檔案：{error}"))
}

#[tauri::command]
fn save_source(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|error| format!("無法儲存檔案：{error}"))
}

#[tauri::command]
async fn ensure_toolchains(
    app: tauri::AppHandle,
    manager: tauri::State<'_, toolchains::ToolchainManager>,
) -> Result<toolchains::ToolchainInfo, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.ensure(&app))
        .await
        .map_err(|error| format!("工具鏈初始化工作失敗：{error}"))?
}

#[tauri::command]
async fn compile_source(
    app: tauri::AppHandle,
    toolchains: tauri::State<'_, toolchains::ToolchainManager>,
    path: String,
) -> Result<engine::CompileResult, String> {
    let toolchains = toolchains.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        toolchains.ensure(&app)?;
        engine::compile(std::path::Path::new(&path))
    })
    .await
    .map_err(|error| format!("編譯工作失敗：{error}"))?
}

#[tauri::command]
async fn run_source(
    registry: tauri::State<'_, engine::RunRegistry>,
    run_id: String,
    path: String,
    input: String,
    timeout_ms: u64,
) -> Result<engine::RunResult, String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        engine::run_source(
            &registry,
            &run_id,
            std::path::Path::new(&path),
            &input,
            timeout_ms,
        )
    })
    .await
    .map_err(|error| format!("執行工作失敗：{error}"))?
}

#[tauri::command]
fn register_run(registry: tauri::State<'_, engine::RunRegistry>, run_id: String) {
    registry.register(&run_id);
}

#[tauri::command]
fn cancel_run(registry: tauri::State<'_, engine::RunRegistry>, run_id: String) -> bool {
    registry.cancel(&run_id)
}

#[tauri::command]
fn finish_run(registry: tauri::State<'_, engine::RunRegistry>, run_id: String) {
    registry.finish(&run_id);
}

#[tauri::command]
fn compare_output(expected: String, actual: String) -> engine::CompareResult {
    engine::compare_output(&expected, &actual)
}

#[tauri::command]
async fn start_clangd(
    app: tauri::AppHandle,
    toolchains: tauri::State<'_, toolchains::ToolchainManager>,
    manager: tauri::State<'_, lsp::LspManager>,
    workspace_path: String,
    document_path: String,
) -> Result<lsp::LspSessionInfo, String> {
    let toolchains = toolchains.inner().clone();
    let toolchain_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || toolchains.ensure(&toolchain_app))
        .await
        .map_err(|error| format!("工具鏈初始化工作失敗：{error}"))??;
    manager.start(
        app,
        std::path::Path::new(&workspace_path),
        std::path::Path::new(&document_path),
    )
}

#[tauri::command]
fn send_clangd(
    manager: tauri::State<'_, lsp::LspManager>,
    session_id: String,
    message: String,
) -> Result<(), String> {
    manager.send(&session_id, message)
}

#[tauri::command]
fn stop_clangd(
    manager: tauri::State<'_, lsp::LspManager>,
    session_id: String,
) -> Result<(), String> {
    manager.stop(&session_id)
}

#[tauri::command]
fn file_uri(path: String) -> Result<String, String> {
    let path = std::path::Path::new(&path)
        .canonicalize()
        .map_err(|error| format!("無法解析程式檔案路徑：{error}"))?;
    url::Url::from_file_path(path)
        .map(|uri| uri.to_string())
        .map_err(|_| "無法將程式檔案路徑轉換成 URI。".into())
}

#[tauri::command]
fn get_problem(target_url: String) -> Result<get_toij::ATiojProblem, String> {
    get_toij::ATiojProblem::get(&target_url)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_prevent_default::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(engine::RunRegistry::default())
        .manage(lsp::LspManager::default())
        .manage(toolchains::ToolchainManager::default())
        .invoke_handler(tauri::generate_handler![
            ensure_toolchains,
            create_project,
            list_source_files,
            read_source,
            save_source,
            compile_source,
            run_source,
            register_run,
            cancel_run,
            finish_run,
            compare_output,
            start_clangd,
            send_clangd,
            stop_clangd,
            file_uri,
            get_problem
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
