// packer.rs — Motor de empaquetado real
// Fluxi v1.0.0 — Fluxionics — Apache 2.0

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::ZipWriter;
use serde::{Deserialize, Serialize};
use tauri::Window;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackConfig {
    pub files: Vec<String>,
    pub output_path: String,
    pub app_name: String,
    pub app_version: String,
    pub entry_point: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackResult {
    pub success: bool,
    pub output_file: String,
    pub size_bytes: u64,
    pub estimated_compressed: u64,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProgressEvent {
    pub percent: u32,
    pub message: String,
    pub current_file: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

const FLUXI_MAGIC: &[u8] = b"FLUXI_FLUXIONICS_V100\x00";

fn emit_progress(window: &Window, percent: u32, message: &str, current_file: &str) {
    let _ = window.emit("pack_progress", ProgressEvent {
        percent,
        message: message.to_string(),
        current_file: current_file.to_string(),
    });
}

#[tauri::command]
pub fn get_file_info(path: String) -> Result<Vec<FileInfo>, String> {
    let p = Path::new(&path);
    let mut entries = Vec::new();

    if p.is_file() {
        let meta = fs::metadata(p).map_err(|e| e.to_string())?;
        entries.push(FileInfo {
            name: p.file_name().unwrap_or_default().to_string_lossy().to_string(),
            path: path.clone(),
            size: meta.len(),
            is_dir: false,
        });
    } else if p.is_dir() {
        for entry in WalkDir::new(p).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            let meta = entry.metadata().map_err(|e| e.to_string())?;
            entries.push(FileInfo {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path().to_string_lossy().to_string(),
                size: meta.len(),
                is_dir: meta.is_dir(),
            });
        }
    }
    Ok(entries)
}

#[tauri::command]
pub async fn pack_to_exe(window: Window, config: PackConfig) -> Result<PackResult, String> {
    emit_progress(&window, 5, "Validando archivos...", "");

    // Validar archivos
    for f in &config.files {
        if !Path::new(f).exists() {
            return Err(format!("Archivo no encontrado: {}", f));
        }
    }

    emit_progress(&window, 10, "Calculando tamaño total...", "");

    // Calcular tamaño total real
    let total_size = calculate_total_size(&config.files);

    emit_progress(&window, 20, "Iniciando compresión ZIP...", "");

    // Construir ZIP con progreso por archivo
    let zip_data = build_zip_with_progress(&window, &config.files)
        .map_err(|e| e.to_string())?;

    emit_progress(&window, 75, "Construyendo metadatos Fluxionics...", "");

    // Metadata
    let metadata = serde_json::json!({
        "app_name": config.app_name,
        "app_version": config.app_version,
        "entry_point": config.entry_point,
        "packed_by": "Fluxi v1.0.0",
        "brand": "Fluxionics",
        "license": "Apache-2.0",
        "original_size": total_size,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    });
    let meta_bytes = serde_json::to_vec(&metadata).map_err(|e| e.to_string())?;

    emit_progress(&window, 82, "Generando stub launcher...", "");

    // Stub
    let stub = build_stub_script(&config.entry_point, &config.app_name);
    let stub_bytes = stub.as_bytes();

    emit_progress(&window, 88, "Ensamblando ejecutable portable...", "");

    // Ensamblar binario
    let mut output: Vec<u8> = Vec::new();
    output.extend_from_slice(stub_bytes);
    output.extend_from_slice(FLUXI_MAGIC);
    let meta_len = meta_bytes.len() as u32;
    output.extend_from_slice(&meta_len.to_le_bytes());
    output.extend_from_slice(&meta_bytes);
    let zip_len = zip_data.len() as u32;
    output.extend_from_slice(&zip_len.to_le_bytes());
    output.extend_from_slice(&zip_data);
    output.extend_from_slice(FLUXI_MAGIC);
    output.extend_from_slice(&(stub_bytes.len() as u32).to_le_bytes());

    emit_progress(&window, 94, "Escribiendo archivo final...", &config.output_path);

    // Escribir archivo
    let out_path = PathBuf::from(&config.output_path);
    fs::write(&out_path, &output).map_err(|e| format!("Error escribiendo .exe: {}", e))?;

    let final_size = output.len() as u64;
    // Estimado comprimido es ~65% del zip + stub overhead
    let estimated = (zip_data.len() as f64 * 0.65) as u64 + stub_bytes.len() as u64;

    emit_progress(&window, 100, "¡Empaquetado completado!", &out_path.file_name().unwrap_or_default().to_string_lossy());

    Ok(PackResult {
        success: true,
        output_file: out_path.to_string_lossy().to_string(),
        size_bytes: final_size,
        estimated_compressed: estimated,
        message: format!(
            "{} v{} empaquetado — {} bytes — by Fluxionics",
            config.app_name, config.app_version, final_size
        ),
    })
}

fn calculate_total_size(paths: &[String]) -> u64 {
    let mut total = 0u64;
    for p in paths {
        let path = Path::new(p);
        if path.is_file() {
            total += fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        } else if path.is_dir() {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.path().is_file() {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
    }
    total
}

fn build_zip_with_progress(window: &Window, paths: &[String]) -> anyhow::Result<Vec<u8>> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    let total = paths.len().max(1);

    for (i, src_str) in paths.iter().enumerate() {
        let src = Path::new(src_str);
        let name = src.file_name().unwrap_or_default().to_string_lossy().to_string();
        let pct = 20 + ((i as f32 / total as f32) * 50.0) as u32;

        emit_progress(window, pct, &format!("Comprimiendo: {}", name), &name);

        if src.is_file() {
            zip_add_file(&mut zip, src, &name, options)?;
        } else if src.is_dir() {
            zip_add_dir(&mut zip, src, &name, options)?;
        }
    }

    Ok(zip.finish()?.into_inner())
}

fn zip_add_file(
    zip: &mut ZipWriter<std::io::Cursor<Vec<u8>>>,
    path: &Path,
    name: &str,
    options: FileOptions,
) -> anyhow::Result<()> {
    zip.start_file(name, options)?;
    let mut buf = Vec::new();
    fs::File::open(path)?.read_to_end(&mut buf)?;
    zip.write_all(&buf)?;
    Ok(())
}

fn zip_add_dir(
    zip: &mut ZipWriter<std::io::Cursor<Vec<u8>>>,
    dir: &Path,
    base: &str,
    options: FileOptions,
) -> anyhow::Result<()> {
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel = path.strip_prefix(dir.parent().unwrap_or(dir))?;
        let zip_name = rel.to_string_lossy().replace('\\', "/");

        if path.is_file() {
            zip.start_file(&zip_name, options)?;
            let mut buf = Vec::new();
            fs::File::open(path)?.read_to_end(&mut buf)?;
            zip.write_all(&buf)?;
        } else if path.is_dir() && path != dir {
            zip.add_directory(&zip_name, options)?;
        }
    }
    Ok(())
}

fn build_stub_script(entry_point: &str, app_name: &str) -> String {
    let safe_name = app_name.to_lowercase().replace(' ', "_");
    format!(
        "@echo off\r\n:: Fluxi Portable Launcher — by Fluxionics\r\n:: App: {app_name}\r\nsetlocal\r\nset \"FLUXI_TMP=%TEMP%\\fluxi_{safe_name}\"\r\nif not exist \"%FLUXI_TMP%\" mkdir \"%FLUXI_TMP%\"\r\npowershell -NoProfile -Command \"$src='%~f0'; $dst='%FLUXI_TMP%\\payload.zip'; [IO.File]::WriteAllBytes($dst,(Get-Content $src -Encoding Byte | Select-Object -Last (Get-Item $src).Length)); Expand-Archive -Force -Path $dst -DestinationPath '%FLUXI_TMP%'\"\r\nstart \"\" \"%FLUXI_TMP%\\{entry_point}\"\r\nendlocal\r\nexit /b\r\n",
        app_name = app_name,
        safe_name = safe_name,
        entry_point = entry_point,
    )
}
