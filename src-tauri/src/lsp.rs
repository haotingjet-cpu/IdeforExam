use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LspSessionInfo {
    pub session_id: String,
    pub event_name: String,
    pub root_uri: String,
    pub document_uri: String,
    pub clangd_path: String,
}

struct Session {
    writer: Sender<String>,
    child: Mutex<Child>,
    compile_database: PathBuf,
}

#[derive(Default)]
pub struct LspManager {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
}

impl LspManager {
    pub fn start(
        &self,
        app: AppHandle,
        workspace_path: &Path,
        document_path: &Path,
    ) -> Result<LspSessionInfo, String> {
        let workspace_path = workspace_path
            .canonicalize()
            .map_err(|error| format!("無法開啟 C++ 專案：{error}"))?;
        if !workspace_path.is_dir() {
            return Err("C++ 專案路徑不是資料夾。".into());
        }
        let document_path = document_path
            .canonicalize()
            .map_err(|error| format!("無法開啟 C++ 檔案：{error}"))?;

        let clangd_path = find_clangd(&workspace_path).ok_or_else(|| {
            "找不到 clangd。請將 clangd 放在 clangd_<version>/bin，或設定 IDEFOREXAM_CLANGD。"
                .to_string()
        })?;
        let session_number = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed);
        let session_id = format!("{}-{session_number}", std::process::id());
        let event_name = format!("clangd-{session_id}");
        let database_dir = std::env::temp_dir()
            .join("ideforexam-clangd")
            .join(&session_id);
        std::fs::create_dir_all(&database_dir)
            .map_err(|error| format!("無法建立 clangd 設定：{error}"))?;
        write_compile_database(&database_dir, &workspace_path)?;

        let mut command = Command::new(&clangd_path);
        command
            .arg("--background-index")
            .arg("--header-insertion=never")
            .arg("--log=error")
            .arg(format!("--compile-commands-dir={}", database_dir.display()))
            .current_dir(&workspace_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(compiler) = crate::engine::compiler_path() {
            command.arg(format!("--query-driver={}", compiler.display()));
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("無法啟動 clangd：{error}"))?;
        let stdin = child.stdin.take().ok_or("無法建立 clangd 輸入管道。")?;
        let stdout = child.stdout.take().ok_or("無法建立 clangd 輸出管道。")?;
        let stderr = child.stderr.take().ok_or("無法建立 clangd 診斷管道。")?;

        let (writer, messages) = mpsc::channel::<String>();
        thread::spawn(move || write_messages(stdin, messages));
        let output_event = event_name.clone();
        thread::spawn(move || read_messages(app, output_event, stdout));
        thread::spawn(move || discard_stderr(stderr));

        let session = Arc::new(Session {
            writer,
            child: Mutex::new(child),
            compile_database: database_dir,
        });
        self.sessions
            .lock()
            .map_err(|_| "clangd session 狀態暫時無法使用。".to_string())?
            .insert(session_id.clone(), session);

        let root_uri = url::Url::from_directory_path(&workspace_path)
            .map_err(|_| "無法將專案路徑轉換成 URI。".to_string())?
            .to_string();
        let document_uri = url::Url::from_file_path(&document_path)
            .map_err(|_| "無法將程式檔案路徑轉換成 URI。".to_string())?
            .to_string();
        Ok(LspSessionInfo {
            session_id,
            event_name,
            root_uri,
            document_uri,
            clangd_path: clangd_path.to_string_lossy().into_owned(),
        })
    }

    pub fn send(&self, session_id: &str, message: String) -> Result<(), String> {
        let session = self.get_session(session_id)?;
        session
            .writer
            .send(message)
            .map_err(|_| "clangd 輸入管道已關閉。".to_string())
    }

    pub fn stop(&self, session_id: &str) -> Result<(), String> {
        let session = self
            .sessions
            .lock()
            .map_err(|_| "clangd session 狀態暫時無法使用。".to_string())?
            .remove(session_id);
        let Some(session) = session else {
            return Ok(());
        };
        drop(session.writer.clone());
        if let Ok(mut child) = session.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&session.compile_database);
        Ok(())
    }

    fn get_session(&self, session_id: &str) -> Result<Arc<Session>, String> {
        self.sessions
            .lock()
            .map_err(|_| "clangd session 狀態暫時無法使用。".to_string())?
            .get(session_id)
            .cloned()
            .ok_or_else(|| "clangd session 已關閉。".to_string())
    }
}

fn find_clangd(workspace_path: &Path) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("IDEFOREXAM_CLANGD") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    #[cfg(windows)]
    let relative_paths = [
        Path::new("clangd_22.1.6").join("bin").join("clangd.exe"),
        Path::new("clangd_18.1.3").join("bin").join("clangd.exe"),
    ];
    #[cfg(not(windows))]
    let relative_paths = [
        Path::new("clangd_22.1.6").join("bin").join("clangd"),
        Path::new("clangd_18.1.3").join("bin").join("clangd"),
    ];

    let mut roots = vec![workspace_path.to_path_buf()];
    if let Ok(current_dir) = std::env::current_dir() {
        roots.push(current_dir);
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    for root in roots {
        for ancestor in root.ancestors() {
            for relative_path in &relative_paths {
                let candidate = ancestor.join(relative_path);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| {
            #[cfg(windows)]
            let name = "clangd.exe";
            #[cfg(not(windows))]
            let name = "clangd";
            directory.join(name)
        })
        .find(|candidate| candidate.is_file())
}

fn write_compile_database(directory: &Path, workspace_path: &Path) -> Result<(), String> {
    let compiler = crate::engine::compiler_path()
        .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "g++.exe" } else { "g++" }));
    let mut source_files = Vec::new();
    collect_sources(workspace_path, &mut source_files)?;
    let entries: Vec<_> = source_files
        .into_iter()
        .map(|file| {
            json!({
                "directory": workspace_path,
                "arguments": [compiler, "-std=c++17", "-c", file],
                "file": file
            })
        })
        .collect();
    let database = serde_json::to_vec(&entries)
        .map_err(|error| format!("無法建立 clangd 編譯資料庫：{error}"))?;
    std::fs::write(directory.join("compile_commands.json"), database)
        .map_err(|error| format!("無法寫入 clangd 編譯資料庫：{error}"))
}

fn collect_sources(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        std::fs::read_dir(directory).map_err(|error| format!("無法讀取專案檔案：{error}"))?
    {
        let entry = entry.map_err(|error| format!("無法讀取專案項目：{error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("無法取得檔案資訊：{error}"))?;
        if file_type.is_dir() {
            if !matches!(
                entry.file_name().to_str(),
                Some(
                    ".git"
                        | "node_modules"
                        | "target"
                        | "build"
                        | "mingw64"
                        | "clangd_22.1.6"
                        | "clangd_18.1.3"
                        | "src-tauri"
                )
            ) {
                collect_sources(&entry.path(), files)?;
            }
        } else if file_type.is_file()
            && entry
                .path()
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "cpp" | "cc" | "cxx"
                    )
                })
        {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn write_messages(mut stdin: impl Write, messages: mpsc::Receiver<String>) {
    while let Ok(message) = messages.recv() {
        if write_lsp_body(&mut stdin, message.as_bytes()).is_err() {
            break;
        }
    }
}

fn read_messages(app: AppHandle, event_name: String, stdout: impl Read) {
    let mut reader = BufReader::new(stdout);
    loop {
        let body = match read_lsp_body(&mut reader) {
            Ok(body) => body,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                let _ = app.emit(&event_name, "__clangd_closed__".to_string());
                return;
            }
            Err(error) => {
                let _ = app.emit(&event_name, format!("__clangd_error__:{error}"));
                return;
            }
        };
        let message = String::from_utf8_lossy(&body).into_owned();
        if app.emit(&event_name, message).is_err() {
            return;
        }
    }
}

fn write_lsp_body(writer: &mut impl Write, body: &[u8]) -> io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(body)?;
    writer.flush()
}

fn read_lsp_body(reader: &mut impl BufRead) -> io::Result<Vec<u8>> {
    let mut content_length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("Content-Length") {
                content_length = value.trim().parse::<usize>().ok();
            }
        }
    }
    let length = content_length
        .filter(|length| *length <= 64 * 1024 * 1024)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid Content-Length"))?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(body)
}

fn discard_stderr(stderr: impl Read) {
    let mut reader = BufReader::new(stderr);
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|read| read > 0) {
        line.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{find_clangd, read_lsp_body, write_compile_database, write_lsp_body};
    use serde_json::{json, Value};
    use std::fs;
    use std::io::{BufReader, Cursor, Write};
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::Ordering;

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ideforexam-clangd-test-{}-{}",
                std::process::id(),
                super::NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct ChildGuard(Child);

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn send_message(stdin: &mut impl Write, message: &Value) {
        write_lsp_body(stdin, &serde_json::to_vec(message).unwrap()).unwrap();
    }

    fn receive_message(stdout: &mut impl std::io::BufRead) -> Value {
        serde_json::from_slice(&read_lsp_body(stdout).unwrap()).unwrap()
    }

    #[test]
    fn lsp_frames_round_trip_json_messages() {
        let message = br#"{"jsonrpc":"2.0","id":3,"method":"initialized"}"#;
        let mut frame = Vec::new();
        write_lsp_body(&mut frame, message).unwrap();
        let mut reader = BufReader::new(Cursor::new(frame));
        assert_eq!(read_lsp_body(&mut reader).unwrap(), message);
    }

    #[test]
    fn clangd_completes_standard_library_symbols_when_header_is_used() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace = manifest_dir.parent().unwrap();
        let Some(clangd) = find_clangd(workspace) else {
            eprintln!("Skipping clangd completion test: clangd is not installed.");
            return;
        };
        let Some(compiler) = crate::engine::compiler_path() else {
            eprintln!("Skipping clangd completion test: g++ is not installed.");
            return;
        };

        let temporary = TemporaryDirectory::new();
        let source_path = temporary.0.join("main.cpp");
        let initial_source =
            "#include <iostream>\nint main() { int value = 0; std::cin >> value; std::cout << value; return 0; }\n";
        let edited_source =
            "#include <iostream>\nint main() { int value = 0; std::cin >> value; std::cout << value; std::co; }\n";
        fs::write(&source_path, initial_source).unwrap();
        write_compile_database(&temporary.0, &temporary.0).unwrap();
        let child = Command::new(clangd)
            .arg("--log=error")
            .arg("--background-index")
            .arg("--header-insertion=never")
            .arg(format!("--compile-commands-dir={}", temporary.0.display()))
            .arg(format!("--query-driver={}", compiler.display()))
            .current_dir(&temporary.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start clangd");
        let mut child = ChildGuard(child);
        let mut stdin = child.0.stdin.take().unwrap();
        let stdout = child.0.stdout.take().unwrap();
        let mut stdout = BufReader::new(stdout);
        let root_uri = url::Url::from_directory_path(&temporary.0)
            .unwrap()
            .to_string();
        let document_uri = url::Url::from_file_path(&source_path).unwrap().to_string();

        send_message(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": std::process::id(),
                    "rootUri": root_uri,
                    "capabilities": {
                        "textDocument": { "completion": { "completionItem": { "snippetSupport": true } } },
                        "workspace": { "workspaceFolders": true }
                    },
                    "workspaceFolders": [{ "uri": root_uri, "name": "smoke" }]
                }
            }),
        );
        let initialize_response = loop {
            let response = receive_message(&mut stdout);
            if response.get("id") == Some(&json!(1)) {
                assert!(response.get("error").is_none(), "{response}");
                break response;
            }
        };
        send_message(
            &mut stdin,
            &json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }),
        );
        send_message(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": { "textDocument": { "uri": document_uri, "languageId": "cpp", "version": 1, "text": initial_source } }
            }),
        );
        send_message(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": document_uri, "version": 2 },
                    "contentChanges": [{ "text": edited_source }]
                }
            }),
        );
        let diagnostics = loop {
            let message = receive_message(&mut stdout);
            if message["method"] == "textDocument/publishDiagnostics"
                && message.pointer("/params/version") == Some(&json!(2))
            {
                break message;
            }
        };
        let completion_line =
            "int main() { int value = 0; std::cin >> value; std::cout << value; std::co";
        send_message(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": { "uri": document_uri },
                    "position": { "line": 1, "character": completion_line.encode_utf16().count() },
                    "context": { "triggerKind": 1 }
                }
            }),
        );
        let response = loop {
            let response = receive_message(&mut stdout);
            if response.get("id") == Some(&json!(2)) {
                break response;
            }
        };
        let items = response
            .pointer("/result/items")
            .or_else(|| response.pointer("/result"))
            .and_then(Value::as_array)
            .expect("completion items");
        assert!(
            initialize_response
                .pointer("/result/capabilities/completionProvider")
                .is_some_and(Value::is_object),
            "clangd did not advertise completion support"
        );
        let diagnostic_items = diagnostics["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics array");
        assert!(
            !diagnostic_items
                .iter()
                .any(|diagnostic| diagnostic["code"] == "unused-includes"),
            "iostream should be used by the test program: {diagnostic_items:?}"
        );
        assert!(
            items.iter().any(|item| item["label"]
                .as_str()
                .is_some_and(|label| label.ends_with("cout"))),
            "clangd did not return std::cout: {items:?}; response: {response}; diagnostics: {diagnostic_items:?}"
        );
    }
}
