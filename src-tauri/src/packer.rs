// packer.rs — Motor de empaquetado real
// Fluxi v1.0.0 — Fluxionics — Apache 2.0

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::ZipWriter;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackConfig {
    pub files: Vec<String>,
    pub output_path: String,
    pub app_name: String,
    pub app_version: String,
    pub entry_point: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PackResult {
    pub success: bool,
    pub output_file: String,
    pub size_bytes: u64,
    pub message: String,
}

/// Firma embebida de Fluxionics — identifica archivos generados por Fluxi
const FLUXI_MAGIC: &[u8] = b"FLUXI_FLUXIONICS_V100\x00";

/// Retorna info de archivos en una ruta dada
#[tauri::command]
pub fn get_file_info(path: String) -> Result<Vec<FileEntry>, String> {
    let p = Path::new(&path);
    let mut entries = Vec::new();

    if p.is_file() {
        let meta = fs::metadata(p).map_err(|e| e.to_string())?;
        entries.push(FileEntry {
            name: p.file_name().unwrap_or_default().to_string_lossy().to_string(),
            path: path.clone(),
            size: meta.len(),
            is_dir: false,
        });
    } else if p.is_dir() {
        for entry in WalkDir::new(p).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            let meta = entry.metadata().map_err(|e| e.to_string())?;
            entries.push(FileEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path().to_string_lossy().to_string(),
                size: meta.len(),
                is_dir: meta.is_dir(),
            });
        }
    }

    Ok(entries)
}

/// Empaqueta archivos y genera el ejecutable portable final
/// Estructura del .fluxi.exe resultante:
/// ┌─────────────────────┐
/// │   STUB LAUNCHER     │  ← Script batch/shell que extrae y ejecuta
/// │   FLUXI_MAGIC       │  ← Firma Fluxionics (22 bytes)
/// │   METADATA JSON     │  ← app_name, version, entry_point, etc.
/// │   ZIP PAYLOAD       │  ← Todos los archivos comprimidos
/// │   FOOTER (offsets)  │  ← Posiciones para que el stub sepa dónde extraer
/// └─────────────────────┘
#[tauri::command]
pub fn pack_to_exe(config: PackConfig) -> Result<PackResult, String> {
    // 1. Validar que existan los archivos
    for f in &config.files {
        if !Path::new(f).exists() {
            return Err(format!("Archivo no encontrado: {}", f));
        }
    }

    // 2. Construir ZIP en memoria con todos los archivos
    let zip_data = build_zip(&config.files).map_err(|e| e.to_string())?;

    // 3. Construir metadata JSON
    let metadata = serde_json::json!({
        "app_name": config.app_name,
        "app_version": config.app_version,
        "entry_point": config.entry_point,
        "packed_by": "Fluxi v1.0.0",
        "brand": "Fluxionics",
        "license": "Apache-2.0",
        "timestamp": chrono_now()
    });
    let meta_bytes = serde_json::to_vec(&metadata).map_err(|e| e.to_string())?;

    // 4. Construir stub launcher (script que extrae en temp y ejecuta)
    let stub = build_stub_script(&config.entry_point, &config.app_name);
    let stub_bytes = stub.as_bytes();

    // 5. Ensamblar binario final
    // Layout: [STUB] [MAGIC] [META_LEN u32le] [META] [ZIP_LEN u32le] [ZIP] [MAGIC] [STUB_LEN u32le]
    let mut output: Vec<u8> = Vec::new();

    // Stub
    let stub_start = 0usize;
    output.extend_from_slice(stub_bytes);

    // Magic
    output.extend_from_slice(FLUXI_MAGIC);

    // Metadata
    let meta_len = meta_bytes.len() as u32;
    output.extend_from_slice(&meta_len.to_le_bytes());
    output.extend_from_slice(&meta_bytes);

    // ZIP payload
    let zip_len = zip_data.len() as u32;
    output.extend_from_slice(&zip_len.to_le_bytes());
    output.extend_from_slice(&zip_data);

    // Footer: magic + stub_len so extractor knows where ZIP starts
    output.extend_from_slice(FLUXI_MAGIC);
    output.extend_from_slice(&(stub_bytes.len() as u32).to_le_bytes());

    // 6. Determinar ruta de salida
    let out_path = resolve_output_path(&config.output_path, &config.app_name);
    fs::write(&out_path, &output).map_err(|e| format!("Error escribiendo .exe: {}", e))?;

    let final_size = output.len() as u64;

    Ok(PackResult {
        success: true,
        output_file: out_path.to_string_lossy().to_string(),
        size_bytes: final_size,
        message: format!(
            "✅ {} v{} empaquetado — {} bytes — by Fluxionics",
            config.app_name, config.app_version, final_size
        ),
    })
}

/// Construye el ZIP con todos los archivos/carpetas indicados
fn build_zip(paths: &[String]) -> anyhow::Result<Vec<u8>> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    for src_str in paths {
        let src = Path::new(src_str);
        if src.is_file() {
            let name = src.file_name().unwrap().to_string_lossy().to_string();
            zip_add_file(&mut zip, src, &name, options)?;
        } else if src.is_dir() {
            let base = src.file_name().unwrap().to_string_lossy().to_string();
            zip_add_dir(&mut zip, src, &base, options)?;
        }
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
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

/// Genera el script stub que extrae y ejecuta al abrir el .exe
/// En Windows se embebe como batch script auto-ejecutable
fn build_stub_script(entry_point: &str, app_name: &str) -> String {
    format!(
        r#"@echo off
:: Fluxi Portable Launcher — by Fluxionics
:: App: {app_name}
setlocal
set "FLUXI_TMP=%TEMP%\fluxi_{app_name_clean}"
if not exist "%FLUXI_TMP%" mkdir "%FLUXI_TMP%"
powershell -NoProfile -Command ^
  "$src='%~f0'; $dst='%FLUXI_TMP%\payload.zip';" ^
  "[IO.File]::WriteAllBytes($dst, (Get-Content $src -Encoding Byte | Select-Object -Last (Get-Item $src).Length));" ^
  "Expand-Archive -Force -Path $dst -DestinationPath '%FLUXI_TMP%'"
start "" "%FLUXI_TMP%\{entry_point}"
endlocal
exit /b
"#,
        app_name = app_name,
        app_name_clean = app_name.to_lowercase().replace(' ', "_"),
        entry_point = entry_point,
    )
}

fn resolve_output_path(output_path: &str, app_name: &str) -> PathBuf {
    let p = PathBuf::from(output_path);
    if p.extension().is_some() {
        p
    } else {
        let safe = app_name.to_lowercase().replace(' ', "-");
        p.join(format!("{}-portable.exe", safe))
    }
}

fn chrono_now() -> String {
    // Simple timestamp without chrono dependency
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
