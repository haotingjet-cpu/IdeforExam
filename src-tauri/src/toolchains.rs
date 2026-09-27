use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tauri::{AppHandle, Manager};
use zip::ZipArchive;

const MAX_ARCHIVE_ENTRIES: usize = 50_000;
const MAX_ARCHIVE_EXPANDED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
static ACTIVE_TOOLCHAINS: OnceLock<RwLock<Option<ToolchainPaths>>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct ToolchainPaths {
    pub gxx: PathBuf,
    pub clangd: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainInfo {
    pub gxx_path: String,
    pub clangd_path: String,
    pub extracted: bool,
}

#[derive(Clone, Default)]
pub struct ToolchainManager {
    paths: Arc<Mutex<Option<ToolchainPaths>>>,
}

impl ToolchainManager {
    pub fn ensure(&self, app: &AppHandle) -> Result<ToolchainInfo, String> {
        let mut cached = self
            .paths
            .lock()
            .map_err(|_| "工具鏈狀態暫時無法使用。".to_string())?;
        if let Some(paths) = cached.as_ref().filter(|paths| valid_paths(paths)) {
            publish(paths.clone());
            return Ok(info(paths, false));
        }

        let app_data = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("無法取得應用程式資料夾：{error}"))?;
        let toolchains_dir = app_data.join("toolchains");
        let mut extracted = false;

        let gxx = if let Some(path) = find_existing_gxx()
            .or_else(|| existing_file(toolchains_dir.join("mingw64/bin/g++.exe")))
        {
            path
        } else {
            let archive = find_archive(app, "mingw64.zip")
                .ok_or_else(|| "找不到 resources/mingw64.zip 工具鏈套件。".to_string())?;
            extract_toolchain_archive(
                &archive,
                &toolchains_dir,
                "mingw64",
                Path::new("mingw64/bin/g++.exe"),
            )?;
            extracted = true;
            toolchains_dir.join("mingw64/bin/g++.exe")
        };

        let clangd = if let Some(path) = find_existing_clangd()
            .or_else(|| existing_file(toolchains_dir.join("clangd_22.1.6/bin/clangd.exe")))
        {
            path
        } else {
            let archive = find_archive(app, "clangd_22.1.6.zip")
                .ok_or_else(|| "找不到 resources/clangd_22.1.6.zip 工具鏈套件。".to_string())?;
            extract_toolchain_archive(
                &archive,
                &toolchains_dir,
                "clangd_22.1.6",
                Path::new("clangd_22.1.6/bin/clangd.exe"),
            )?;
            extracted = true;
            toolchains_dir.join("clangd_22.1.6/bin/clangd.exe")
        };

        let paths = ToolchainPaths { gxx, clangd };
        *cached = Some(paths.clone());
        publish(paths.clone());
        Ok(info(&paths, extracted))
    }
}

fn existing_file(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

pub fn active_paths() -> Option<ToolchainPaths> {
    ACTIVE_TOOLCHAINS
        .get()?
        .read()
        .ok()?
        .as_ref()
        .filter(|paths| valid_paths(paths))
        .cloned()
}

fn publish(paths: ToolchainPaths) {
    let active = ACTIVE_TOOLCHAINS.get_or_init(|| RwLock::new(None));
    if let Ok(mut active) = active.write() {
        *active = Some(paths);
    }
}

fn valid_paths(paths: &ToolchainPaths) -> bool {
    paths.gxx.is_file() && paths.clangd.is_file()
}

fn info(paths: &ToolchainPaths, extracted: bool) -> ToolchainInfo {
    ToolchainInfo {
        gxx_path: paths.gxx.to_string_lossy().into_owned(),
        clangd_path: paths.clangd.to_string_lossy().into_owned(),
        extracted,
    }
}

fn find_existing_gxx() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("IDEFOREXAM_GXX") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    find_in_workspace("mingw64/bin/g++.exe")
}

fn find_existing_clangd() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("IDEFOREXAM_CLANGD") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    find_in_workspace("clangd_22.1.6/bin/clangd.exe")
        .or_else(|| find_in_workspace("clangd_18.1.3/bin/clangd.exe"))
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH")?)
                .map(|directory| directory.join("clangd.exe"))
                .find(|path| path.is_file())
        })
}

fn find_in_workspace(relative_path: &str) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        roots.push(current_dir);
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    roots
        .into_iter()
        .flat_map(|root| root.ancestors().map(Path::to_path_buf).collect::<Vec<_>>())
        .map(|root| root.join(relative_path))
        .find(|path| path.is_file())
}

fn find_archive(app: &AppHandle, filename: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("resources").join(filename));
        candidates.push(resource_dir.join(filename));
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("resources").join(filename));
    }
    let workspace_resources = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|root| root.join("resources").join(filename));
    candidates.extend(workspace_resources);
    candidates.into_iter().find(|path| path.is_file())
}

fn extract_toolchain_archive(
    archive_path: &Path,
    install_root: &Path,
    top_level_directory: &str,
    required_relative_file: &Path,
) -> Result<(), String> {
    let destination = install_root.join(top_level_directory);
    if destination
        .join(
            required_relative_file
                .strip_prefix(top_level_directory)
                .unwrap_or(required_relative_file),
        )
        .is_file()
    {
        return Ok(());
    }
    if destination.exists() {
        return Err(format!(
            "工具鏈目錄不完整，為避免覆蓋使用者檔案已停止安裝：{}",
            destination.display()
        ));
    }

    fs::create_dir_all(install_root).map_err(|error| format!("無法建立工具鏈資料夾：{error}"))?;
    let staging = install_root.join(format!(
        ".{top_level_directory}.installing-{}-{}",
        std::process::id(),
        NEXT_STAGE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir(&staging).map_err(|error| format!("無法建立暫存資料夾：{error}"))?;

    let extraction_result = extract_zip(archive_path, &staging);
    if let Err(error) = extraction_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let staged_toolchain = staging.join(top_level_directory);
    let required_file = required_relative_file
        .strip_prefix(top_level_directory)
        .unwrap_or(required_relative_file);
    if !staged_toolchain.join(required_file).is_file() {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!(
            "ZIP 缺少必要執行檔：{}",
            required_relative_file.display()
        ));
    }

    fs::rename(&staged_toolchain, &destination)
        .map_err(|error| format!("無法完成工具鏈安裝：{error}"))?;
    let _ = fs::remove_dir_all(&staging);
    Ok(())
}

static NEXT_STAGE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn extract_zip(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = File::open(archive_path)
        .map_err(|error| format!("無法讀取工具鏈 ZIP {}：{error}", archive_path.display()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("工具鏈 ZIP 格式錯誤：{error}"))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("工具鏈 ZIP 內檔案數量超出安全上限。".into());
    }
    let mut expanded_bytes = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("無法讀取 ZIP entry：{error}"))?;
        expanded_bytes = expanded_bytes
            .checked_add(entry.size())
            .filter(|size| *size <= MAX_ARCHIVE_EXPANDED_BYTES)
            .ok_or_else(|| "工具鏈 ZIP 展開大小超出 2 GiB 安全上限。".to_string())?;

        let relative_path = entry
            .enclosed_name()
            .ok_or_else(|| format!("ZIP 含有不安全路徑：{}", entry.name()))?
            .to_path_buf();
        if relative_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(format!("ZIP 含有不安全路徑：{}", entry.name()));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(format!("ZIP 不允許符號連結：{}", entry.name()));
        }

        let output_path = destination.join(relative_path);
        if entry.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|error| format!("無法建立工具鏈資料夾：{error}"))?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("無法建立工具鏈資料夾：{error}"))?;
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output_path)
            .map_err(|error| format!("無法建立工具鏈檔案：{error}"))?;
        io::copy(&mut entry, &mut output)
            .map_err(|error| format!("解壓工具鏈檔案失敗：{error}"))?;
        output
            .flush()
            .map_err(|error| format!("寫入工具鏈檔案失敗：{error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{extract_toolchain_archive, extract_zip};
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use zip::write::SimpleFileOptions;

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ideforexam-toolchain-test-{}-{}",
                std::process::id(),
                super::NEXT_STAGE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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

    fn create_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(path).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        for (name, contents) in entries {
            archive
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(contents).unwrap();
        }
        archive.finish().unwrap();
    }

    #[test]
    fn installs_and_reuses_a_toolchain_archive() {
        let temporary = TemporaryDirectory::new();
        let archive = temporary.0.join("mingw64.zip");
        create_zip(&archive, &[("mingw64/bin/g++.exe", b"compiler")]);
        let destination = temporary.0.join("installed");

        extract_toolchain_archive(
            &archive,
            &destination,
            "mingw64",
            Path::new("mingw64/bin/g++.exe"),
        )
        .unwrap();
        fs::remove_file(&archive).unwrap();
        extract_toolchain_archive(
            &archive,
            &destination,
            "mingw64",
            Path::new("mingw64/bin/g++.exe"),
        )
        .unwrap();

        assert_eq!(
            fs::read(destination.join("mingw64/bin/g++.exe")).unwrap(),
            b"compiler"
        );
    }

    #[test]
    fn rejects_zip_slip_paths_without_writing_outside_staging() {
        let temporary = TemporaryDirectory::new();
        let archive_path = temporary.0.join("unsafe.zip");
        create_zip(&archive_path, &[("../escaped.txt", b"escape")]);
        let destination = temporary.0.join("staging");
        fs::create_dir_all(&destination).unwrap();

        assert!(extract_zip(&archive_path, &destination).is_err());
        assert!(!temporary.0.join("escaped.txt").exists());
    }

    #[test]
    #[ignore = "Expands approximately 1 GB; run manually before release packaging."]
    fn extracts_bundled_toolchain_archives_into_expected_layout() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let resource_dir = workspace.join("resources");
        if !resource_dir.join("mingw64.zip").is_file()
            || !resource_dir.join("clangd_22.1.6.zip").is_file()
        {
            eprintln!("Skipping bundled archive test: resource ZIPs are not available.");
            return;
        }
        let temporary = TemporaryDirectory::new();
        let destination = temporary.0.join("installed");

        extract_toolchain_archive(
            &resource_dir.join("mingw64.zip"),
            &destination,
            "mingw64",
            Path::new("mingw64/bin/g++.exe"),
        )
        .unwrap();
        extract_toolchain_archive(
            &resource_dir.join("clangd_22.1.6.zip"),
            &destination,
            "clangd_22.1.6",
            Path::new("clangd_22.1.6/bin/clangd.exe"),
        )
        .unwrap();

        assert!(destination.join("mingw64/bin/g++.exe").is_file());
        assert!(destination.join("clangd_22.1.6/bin/clangd.exe").is_file());
    }
}
