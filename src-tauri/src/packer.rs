// packer.rs — Motor de empaquetado con streaming
// Fluxi v1.0.0 — Fluxionics — Apache 2.0

use std::fs::{self, File};
use std::io::{Read, Write, BufWriter, BufReader};
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
// Buffer de 4MB para streaming — nunca carga el archivo completo en RAM
const STREAM_BUF: usize = 4 * 1024 * 1024;

fn emit(window: &Window, percent: u32, message: &str, current_file: &str) {
    let _ = window.emit("pack_progress", ProgressEvent {
        percent,
        message: message.to_string(),
        current_file: current_file.to_string(),
    });
}

#[tauri::command]
pub fn get_file_size(path: String) -> u64 {
    let p = Path::new(&path);
    if p.is_file() {
        fs::metadata(p).map(|m| m.len()).unwrap_or(0)
    } else if p.is_dir() {
        WalkDir::new(p)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
            .sum()
    } else {
        0
    }
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

/// Empaquetado principal — streaming directo a disco, sin cargar en RAM
#[tauri::command]
pub async fn pack_to_exe(window: Window, config: PackConfig) -> Result<PackResult, String> {
    emit(&window, 2, "Validando archivos...", "");

    for f in &config.files {
        if !Path::new(f).exists() {
            return Err(format!("Archivo no encontrado: {}", f));
        }
    }

    // Calcular total de archivos para progreso
    emit(&window, 5, "Calculando tamaño total...", "");
    let total_files = count_files(&config.files);
    let total_size  = calculate_total_size(&config.files);

    // Ruta del ZIP temporal en disco (no en RAM)
    let zip_tmp = PathBuf::from(&config.output_path).with_extension("fluxi_tmp.zip");

    emit(&window, 8, "Creando ZIP en disco (streaming)...", "");

    // ── FASE 1: Escribir ZIP directo al disco en streaming
    {
        let zip_file = File::create(&zip_tmp)
            .map_err(|e| format!("No se pudo crear ZIP temporal: {}", e))?;
        let buf_writer = BufWriter::with_capacity(STREAM_BUF, zip_file);
        let mut zip = ZipWriter::new(buf_writer);

        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        let mut file_count = 0usize;

        for src_str in &config.files {
            let src = Path::new(src_str);
            let base_name = src.file_name().unwrap_or_default().to_string_lossy().to_string();

            if src.is_file() {
                file_count += 1;
                let pct = 8 + ((file_count as f32 / total_files.max(1) as f32) * 72.0) as u32;
                emit(&window, pct, &format!("Comprimiendo: {}", base_name), &base_name);
                stream_file_to_zip(&mut zip, src, &base_name, options)
                    .map_err(|e| format!("Error comprimiendo {}: {}", base_name, e))?;

            } else if src.is_dir() {
                for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    let rel  = path.strip_prefix(src.parent().unwrap_or(src))
                        .unwrap_or(path);
                    let zip_name = rel.to_string_lossy().replace('\\', "/");

                    if path.is_file() {
                        file_count += 1;
                        let pct = 8 + ((file_count as f32 / total_files.max(1) as f32) * 72.0) as u32;
                        let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        emit(&window, pct.min(79), &format!("Comprimiendo: {}", fname), &fname);
                        stream_file_to_zip(&mut zip, path, &zip_name, options)
                            .map_err(|e| format!("Error comprimiendo {}: {}", zip_name, e))?;
                    } else if path.is_dir() && path != src {
                        let _ = zip.add_directory(&zip_name, options);
                    }
                }
            }
        }

        zip.finish().map_err(|e| format!("Error finalizando ZIP: {}", e))?;
    }

    emit(&window, 80, "Construyendo metadatos Fluxionics...", "");

    // ── FASE 2: Construir stub + metadata
    let stub = build_stub(&config.entry_point, &config.app_name);
    let stub_bytes = stub.as_bytes();

    let metadata = serde_json::json!({
        "app_name":    config.app_name,
        "app_version": config.app_version,
        "entry_point": config.entry_point,
        "packed_by":   "Fluxi v1.0.0",
        "brand":       "Fluxionics",
        "license":     "Apache-2.0",
        "original_size": total_size,
        "total_files": total_files,
        "timestamp":   std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs()).unwrap_or(0)
    });
    let meta_bytes = serde_json::to_vec(&metadata).map_err(|e| e.to_string())?;

    emit(&window, 84, "Ensamblando ejecutable portable...", "");

    // ── FASE 3: Escribir .exe final en streaming
    // Layout: [STUB][MAGIC][META_LEN u32le][META][ZIP_LEN u64le][ZIP_DATA][MAGIC][STUB_LEN u32le]
    {
        let out_file = File::create(&config.output_path)
            .map_err(|e| format!("No se pudo crear .exe: {}", e))?;
        let mut out = BufWriter::with_capacity(STREAM_BUF, out_file);

        // Stub
        out.write_all(stub_bytes).map_err(|e| e.to_string())?;

        // Magic + metadata
        out.write_all(FLUXI_MAGIC).map_err(|e| e.to_string())?;
        out.write_all(&(meta_bytes.len() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
        out.write_all(&meta_bytes).map_err(|e| e.to_string())?;

        // ZIP size (u64 para soportar >4GB)
        let zip_size = fs::metadata(&zip_tmp).map(|m| m.len()).unwrap_or(0);
        out.write_all(&zip_size.to_le_bytes()).map_err(|e| e.to_string())?;

        emit(&window, 88, "Copiando payload al ejecutable...", "");

        // Copiar ZIP al exe en chunks de 4MB — nunca en RAM
        let mut zip_reader = BufReader::with_capacity(STREAM_BUF,
            File::open(&zip_tmp).map_err(|e| e.to_string())?);
        let mut buf = vec![0u8; STREAM_BUF];
        let mut copied = 0u64;

        loop {
            let n = zip_reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 { break; }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            copied += n as u64;

            // Progreso del copiado
            if zip_size > 0 {
                let copy_pct = (copied as f64 / zip_size as f64 * 10.0) as u32;
                emit(&window, (88 + copy_pct).min(97),
                    &format!("Copiando payload... {:.1} MB / {:.1} MB",
                        copied as f64 / 1_048_576.0,
                        zip_size as f64 / 1_048_576.0),
                    "");
            }
        }

        // Footer
        out.write_all(FLUXI_MAGIC).map_err(|e| e.to_string())?;
        out.write_all(&(stub_bytes.len() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
    }

    // Limpiar ZIP temporal
    let _ = fs::remove_file(&zip_tmp);

    emit(&window, 100, "¡Empaquetado completado!", "");

    let final_size = fs::metadata(&config.output_path).map(|m| m.len()).unwrap_or(0);

    Ok(PackResult {
        success: true,
        output_file: config.output_path.clone(),
        size_bytes: final_size,
        message: format!(
            "{} v{} empaquetado — {:.1} MB — {} archivos — by Fluxionics",
            config.app_name,
            config.app_version,
            final_size as f64 / 1_048_576.0,
            total_files
        ),
    })
}

/// Copia un archivo al ZIP en chunks — nunca carga el archivo completo en RAM
fn stream_file_to_zip(
    zip: &mut ZipWriter<BufWriter<File>>,
    path: &Path,
    name: &str,
    options: FileOptions,
) -> anyhow::Result<()> {
    zip.start_file(name, options)?;
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(STREAM_BUF, file);
    let mut buf = vec![0u8; STREAM_BUF];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        zip.write_all(&buf[..n])?;
    }
    Ok(())
}

fn count_files(paths: &[String]) -> usize {
    let mut count = 0;
    for p in paths {
        let path = Path::new(p);
        if path.is_file() {
            count += 1;
        } else if path.is_dir() {
            count += WalkDir::new(path).into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .count();
        }
    }
    count
}

fn calculate_total_size(paths: &[String]) -> u64 {
    let mut total = 0u64;
    for p in paths {
        let path = Path::new(p);
        if path.is_file() {
            total += fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        } else if path.is_dir() {
            total += WalkDir::new(path).into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
                .sum::<u64>();
        }
    }
    total
}

fn build_stub(entry_point: &str, app_name: &str) -> String {
    let safe = app_name.to_lowercase().replace(' ', "_");
    format!(
        "@echo off\r\n:: Fluxi Portable Launcher — by Fluxionics\r\nsetlocal\r\nset \"TMP_DIR=%TEMP%\\fluxi_{safe}\"\r\nif not exist \"%TMP_DIR%\" mkdir \"%TMP_DIR%\"\r\npowershell -NoProfile -Command \"Expand-Archive -Force -Path '%~f0' -DestinationPath '%TMP_DIR%'\"\r\nstart \"\" \"%TMP_DIR%\\{entry}\"\r\nendlocal\r\nexit /b\r\n",
        safe = safe,
        entry = entry_point,
    )
}
