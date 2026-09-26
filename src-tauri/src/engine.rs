use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::{collections::HashMap, process::Child};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileResult {
    pub success: bool,
    pub output: String,
    pub executable_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub stdout: String,
    pub stderr: String,
    pub execution_time_ms: u128,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
}

#[derive(Clone, Default)]
pub struct RunRegistry {
    runs: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl RunRegistry {
    pub fn register(&self, run_id: &str) {
        if let Ok(mut runs) = self.runs.lock() {
            runs.entry(run_id.to_string())
                .or_insert_with(|| Arc::new(AtomicBool::new(false)));
        }
    }

    pub fn cancel(&self, run_id: &str) -> bool {
        let Ok(mut runs) = self.runs.lock() else {
            return false;
        };
        let cancellation = runs
            .entry(run_id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)));
        cancellation.store(true, Ordering::SeqCst);
        true
    }

    pub fn finish(&self, run_id: &str) {
        if let Ok(mut runs) = self.runs.lock() {
            runs.remove(run_id);
        }
    }

    fn cancellation_for(&self, run_id: &str) -> Result<Arc<AtomicBool>, String> {
        let mut runs = self
            .runs
            .lock()
            .map_err(|_| "執行狀態暫時無法使用。".to_string())?;
        Ok(runs
            .entry(run_id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareResult {
    pub accepted: bool,
    pub first_difference: Option<usize>,
}

pub fn compile(source_path: &Path) -> Result<CompileResult, String> {
    if !source_path.is_file() {
        return Err("Unable to open source file.".into());
    }

    let executable_path = executable_path_for(source_path)?;
    let _ = std::fs::remove_file(&executable_path);
    let output = compiler_command()
        .arg(source_path)
        .arg("-std=c++17")
        .arg("-O2")
        .arg("-o")
        .arg(&executable_path)
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "找不到 C++ Compiler。請安裝 GCC / G++ 並確認已加入 PATH。".to_string()
            } else {
                format!("無法啟動 GCC / G++：{error}")
            }
        })?;

    let compiler_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    if !output.status.success() {
        let _ = std::fs::remove_file(&executable_path);
        return Ok(CompileResult {
            success: false,
            output: compiler_output,
            executable_path: None,
        });
    }

    Ok(CompileResult {
        success: true,
        output: if compiler_output.is_empty() {
            "Build Successful".into()
        } else {
            compiler_output
        },
        executable_path: Some(executable_path.to_string_lossy().into_owned()),
    })
}

fn compiler_command() -> Command {
    if let Some(path) = std::env::var_os("IDEFOREXAM_GXX") {
        return Command::new(path);
    }

    #[cfg(windows)]
    let relative_path = Path::new("mingw64").join("bin").join("g++.exe");
    #[cfg(not(windows))]
    let relative_path = Path::new("mingw64").join("bin").join("g++");

    let mut roots = Vec::new();
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
            let candidate = ancestor.join(&relative_path);
            if candidate.is_file() {
                return Command::new(candidate);
            }
        }
    }

    Command::new("g++")
}

pub fn run_program(
    executable_path: &Path,
    input: &str,
    timeout_ms: u64,
    cancellation: Arc<AtomicBool>,
) -> Result<RunResult, String> {
    let mut child = Command::new(executable_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("無法執行程式：{error}"))?;

    let stdin = child.stdin.take().expect("piped stdin");
    let input = input.as_bytes().to_vec();
    let input_thread = thread::spawn(move || {
        let mut stdin = stdin;
        let _ = stdin.write_all(&input);
    });
    let stdout_thread = spawn_reader(child.stdout.take().expect("piped stdout"));
    let stderr_thread = spawn_reader(child.stderr.take().expect("piped stderr"));

    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms.clamp(100, 300_000));
    let mut timed_out = false;
    let mut cancelled = false;
    let status = loop {
        if cancellation.load(Ordering::SeqCst) {
            cancelled = true;
            break Some(terminate_child(&mut child)?);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("無法取得程式狀態：{error}"))?
        {
            break Some(status);
        }
        if start.elapsed() >= timeout {
            timed_out = true;
            break Some(terminate_child(&mut child)?);
        }
        thread::sleep(Duration::from_millis(10));
    };

    let _ = input_thread.join();
    let stdout = join_reader(stdout_thread)?;
    let stderr = join_reader(stderr_thread)?;

    Ok(RunResult {
        stdout,
        stderr,
        execution_time_ms: start.elapsed().as_millis(),
        exit_code: status.and_then(|status| status.code()),
        timed_out,
        cancelled,
    })
}

pub fn run_source(
    registry: &RunRegistry,
    run_id: &str,
    source_path: &Path,
    input: &str,
    timeout_ms: u64,
) -> Result<RunResult, String> {
    let executable_path = executable_path_for(source_path)?;
    if !executable_path.is_file() {
        return Err("請先成功編譯目前程式。".into());
    }
    let cancellation = registry.cancellation_for(run_id)?;
    run_program(&executable_path, input, timeout_ms, cancellation)
}

fn terminate_child(child: &mut Child) -> Result<std::process::ExitStatus, String> {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let terminated = Command::new("taskkill")
            .args(["/PID", &pid, "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !terminated && child.try_wait().ok().flatten().is_none() {
            child
                .kill()
                .map_err(|error| format!("無法終止執行中的程式：{error}"))?;
        }
    }
    #[cfg(not(windows))]
    child
        .kill()
        .map_err(|error| format!("無法終止執行中的程式：{error}"))?;

    child
        .wait()
        .map_err(|error| format!("無法等待程式終止：{error}"))
}

pub fn compare_output(expected: &str, actual: &str) -> CompareResult {
    let expected = expected.replace("\r\n", "\n");
    let actual = actual.replace("\r\n", "\n");
    let expected = expected.trim_end_matches('\n');
    let actual = actual.trim_end_matches('\n');
    let first_difference = expected
        .chars()
        .zip(actual.chars())
        .position(|(expected_char, actual_char)| expected_char != actual_char)
        .or_else(|| {
            let expected_len = expected.chars().count();
            (expected_len != actual.chars().count())
                .then_some(expected_len.min(actual.chars().count()))
        });

    CompareResult {
        accepted: first_difference.is_none(),
        first_difference,
    }
}

fn executable_path_for(source_path: &Path) -> Result<PathBuf, String> {
    let parent = source_path
        .parent()
        .ok_or_else(|| "無法取得程式檔案所在資料夾。".to_string())?;
    let stem = source_path
        .file_stem()
        .ok_or_else(|| "無效的程式檔名。".to_string())?
        .to_string_lossy();
    let extension = if cfg!(windows) { ".exe" } else { "" };
    Ok(parent.join(format!("{stem}.ideforexam{extension}")))
}

fn spawn_reader<R: Read + Send + 'static>(
    mut reader: R,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

fn join_reader(reader: thread::JoinHandle<std::io::Result<Vec<u8>>>) -> Result<String, String> {
    let bytes = reader
        .join()
        .map_err(|_| "讀取程式輸出時發生錯誤。".to_string())?
        .map_err(|error| format!("讀取程式輸出失敗：{error}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::{compare_output, compile, run_source, RunRegistry};
    use std::fs;
    use std::sync::atomic::Ordering;

    #[test]
    fn compares_exact_output_and_reports_first_difference() {
        assert!(compare_output("42\n", "42\n").accepted);

        let result = compare_output("42\n", "43\n");
        assert!(!result.accepted);
        assert_eq!(result.first_difference, Some(1));
    }

    #[test]
    fn ignores_trailing_newlines_but_preserves_internal_blank_lines() {
        let result = compare_output("answer\n", "answer\n\n");
        assert!(result.accepted);
        assert_eq!(result.first_difference, None);
        assert!(compare_output("answer\n", "answer").accepted);
        assert!(!compare_output("answer\n\nvalue\n", "answer\nvalue\n").accepted);
    }

    #[test]
    fn treats_windows_and_unix_line_endings_as_equivalent() {
        assert!(compare_output("25\n", "25\r\n").accepted);
    }

    #[test]
    fn cancellation_requests_are_shared_with_running_processes() {
        let registry = RunRegistry::default();
        registry.register("test-run");
        assert!(!registry
            .cancellation_for("test-run")
            .unwrap()
            .load(Ordering::SeqCst));
        assert!(registry.cancel("test-run"));
        assert!(registry
            .cancellation_for("test-run")
            .unwrap()
            .load(Ordering::SeqCst));
        registry.finish("test-run");
        assert!(!registry
            .cancellation_for("test-run")
            .unwrap()
            .load(Ordering::SeqCst));
    }

    #[test]
    fn compiles_and_runs_cpp_with_stdin_and_stdout() {
        let source_path = std::env::temp_dir().join(format!(
            "ideforexam-smoke-{}-{}.cpp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            &source_path,
            "#include <iostream>\nint main() { int n; std::cin >> n; std::cout << n * n << '\\n'; }\n",
        )
        .unwrap();

        let compiled = match compile(&source_path) {
            Ok(result) => result,
            Err(error) if error.contains("找不到 C++ Compiler") => {
                eprintln!("Skipping C++ integration test: g++ is not installed.");
                let _ = fs::remove_file(source_path);
                return;
            }
            Err(error) => panic!("compile failed: {error}"),
        };
        assert!(compiled.success, "{}", compiled.output);

        let registry = RunRegistry::default();
        registry.register("smoke-test");
        let result = run_source(&registry, "smoke-test", &source_path, "5\n", 2_000).unwrap();
        registry.finish("smoke-test");
        assert!(!result.timed_out);
        assert_eq!(result.exit_code, Some(0));
        assert!(compare_output("25\n", &result.stdout).accepted);

        let _ = fs::remove_file(source_path);
        if let Some(path) = compiled.executable_path {
            let _ = fs::remove_file(path);
        }
    }
}
