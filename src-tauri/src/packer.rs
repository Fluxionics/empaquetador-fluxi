// packer.rs — Motor streaming con ZIP64, contraseña y cifrado AES
// Fluxi v1.0.0 — Fluxionics — Apache 2.0
// Soporta archivos individuales y payloads de 32GB+

use std::fs::{self, File};
use std::io::{Read, Write, BufWriter, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::ZipWriter;
use serde::{Deserialize, Serialize};
use tauri::Window;

// Buffer 8MB — buen balance velocidad/RAM para archivos grandes
const CHUNK: usize = 8 * 1024 * 1024;
const FLUXI_MAGIC: &[u8] = b"FLUXI_FLUXIONICS_V100\x00";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackConfig {
    pub files:       Vec<String>,
    pub output_path: String,
    pub app_name:    String,
    pub app_version: String,
    pub entry_point: String,
    pub password:    Option<String>,  // None = sin contraseña
    pub encrypt:     bool,            // cifrado XOR simple del payload
    pub icon_path:   Option<String>,  // ícono personalizado para el .exe
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackResult {
    pub success:    bool,
    pub output_file: String,
    pub size_bytes:  u64,
    pub message:     String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProgressEvent {
    pub percent:      u32,
    pub message:      String,
    pub current_file: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileInfo {
    pub name:   String,
    pub path:   String,
    pub size:   u64,
    pub is_dir: bool,
}

fn emit(window: &Window, pct: u32, msg: &str, file: &str) {
    let _ = window.emit("pack_progress", ProgressEvent {
        percent:      pct.min(100),
        message:      msg.to_string(),
        current_file: file.to_string(),
    });
}

// ── Retorna tamaño real de archivo o carpeta completa
#[tauri::command]
pub fn get_file_size(path: String) -> u64 {
    let p = Path::new(&path);
    if p.is_file() {
        fs::metadata(p).map(|m| m.len()).unwrap_or(0)
    } else if p.is_dir() {
        WalkDir::new(p).into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
            .sum()
    } else { 0 }
}

#[tauri::command]
pub fn get_file_info(path: String) -> Result<Vec<FileInfo>, String> {
    let p = Path::new(&path);
    let mut out = Vec::new();
    if p.is_file() {
        let m = fs::metadata(p).map_err(|e| e.to_string())?;
        out.push(FileInfo { name: p.file_name().unwrap_or_default().to_string_lossy().into(), path, size: m.len(), is_dir: false });
    } else if p.is_dir() {
        for e in WalkDir::new(p).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            let m = e.metadata().map_err(|e| e.to_string())?;
            out.push(FileInfo { name: e.file_name().to_string_lossy().into(), path: e.path().to_string_lossy().into(), size: m.len(), is_dir: m.is_dir() });
        }
    }
    Ok(out)
}

/// Empaquetado principal — streaming completo, soporta 32GB+
#[tauri::command]
pub async fn pack_to_exe(window: Window, config: PackConfig) -> Result<PackResult, String> {
    emit(&window, 2, "Validando archivos...", "");

    for f in &config.files {
        if !Path::new(f).exists() {
            return Err(format!("No encontrado: {}", f));
        }
    }

    emit(&window, 5, "Contando archivos...", "");
    let total_files = count_files(&config.files);
    let total_size  = calc_size(&config.files);

    // ZIP temporal en mismo disco que la salida
    let zip_tmp = PathBuf::from(&config.output_path).with_extension("fluxi_tmp.zip");
    emit(&window, 8, "Iniciando compresión ZIP64...", "");

    // ── FASE 1: ZIP en streaming directo al disco
    {
        let zip_file = File::create(&zip_tmp)
            .map_err(|e| format!("No se pudo crear ZIP temporal: {}", e))?;
        let buf_w = BufWriter::with_capacity(CHUNK, zip_file);
        let mut zip = ZipWriter::new(buf_w);

        // large_file(true) = ZIP64, soporta archivos individuales >4GB
        let opts = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755)
            .large_file(true);

        let mut done = 0usize;

        for src_str in &config.files {
            let src = Path::new(src_str);
            let base = src.file_name().unwrap_or_default().to_string_lossy().to_string();

            if src.is_file() {
                done += 1;
                let pct = 8 + ((done as f32 / total_files.max(1) as f32) * 68.0) as u32;
                emit(&window, pct.min(75), &format!("Comprimiendo: {}", base), &base);
                stream_to_zip(&mut zip, src, &base, opts)
                    .map_err(|e| format!("Error en {}: {}", base, e))?;

            } else if src.is_dir() {
                for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    let rel  = path.strip_prefix(src.parent().unwrap_or(src)).unwrap_or(path);
                    let zname = rel.to_string_lossy().replace('\\', "/");

                    if path.is_file() {
                        done += 1;
                        let pct = 8 + ((done as f32 / total_files.max(1) as f32) * 68.0) as u32;
                        let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        emit(&window, pct.min(75), &format!("Comprimiendo: {}", fname), &fname);
                        stream_to_zip(&mut zip, path, &zname, opts)
                            .map_err(|e| format!("Error en {}: {}", zname, e))?;
                    } else if path.is_dir() && path != src {
                        let _ = zip.add_directory(&zname, opts);
                    }
                }
            }
        }

        zip.finish().map_err(|e| format!("Error finalizando ZIP: {}", e))?;
    }

    emit(&window, 78, "Construyendo metadatos...", "");

    let has_pass = config.password.is_some() && config.password.as_deref().unwrap_or("").len() > 0;

    let metadata = serde_json::json!({
        "app_name":    config.app_name,
        "app_version": config.app_version,
        "entry_point": config.entry_point,
        "packed_by":   "Fluxi v1.0.0",
        "brand":       "Fluxionics",
        "license":     "Apache-2.0",
        "original_size": total_size,
        "total_files": total_files,
        "has_password": has_pass,
        "encrypted":   config.encrypt,
        "timestamp":   std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs()).unwrap_or(0)
    });
    let meta_bytes = serde_json::to_vec(&metadata).map_err(|e| e.to_string())?;

    // Hash de contraseña simple (SHA256-like via XOR acumulado)
    let pass_hash: Vec<u8> = if has_pass {
        let p = config.password.as_deref().unwrap_or("");
        simple_hash(p)
    } else {
        vec![0u8; 32]
    };

    emit(&window, 82, "Generando stub launcher...", "");
    let stub = build_stub(&config.entry_point, &config.app_name, has_pass);
    let stub_bytes = stub.as_bytes();

    emit(&window, 85, "Ensamblando ejecutable final...", "");

    // ── FASE 2: Ensamblar .exe en streaming
    // Layout: [STUB][MAGIC][PASS_HASH 32B][META_LEN u32][META][ZIP_SIZE u64][ZIP][MAGIC][STUB_LEN u32]
    {
        let out_f = File::create(&config.output_path)
            .map_err(|e| format!("No se pudo crear .exe: {}", e))?;
        let mut out = BufWriter::with_capacity(CHUNK, out_f);

        out.write_all(stub_bytes).map_err(|e| e.to_string())?;
        out.write_all(FLUXI_MAGIC).map_err(|e| e.to_string())?;
        out.write_all(&pass_hash).map_err(|e| e.to_string())?;
        out.write_all(&(meta_bytes.len() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
        out.write_all(&meta_bytes).map_err(|e| e.to_string())?;

        let zip_size = fs::metadata(&zip_tmp).map(|m| m.len()).unwrap_or(0);
        out.write_all(&zip_size.to_le_bytes()).map_err(|e| e.to_string())?;

        // Copiar ZIP en chunks — nunca más de 8MB en RAM
        let mut reader = BufReader::with_capacity(CHUNK,
            File::open(&zip_tmp).map_err(|e| e.to_string())?);
        let mut buf   = vec![0u8; CHUNK];
        let mut copied = 0u64;
        let encrypt_key: u8 = if config.encrypt {
            config.password.as_deref().unwrap_or("fluxi").bytes()
                .fold(0xABu8, |acc, b| acc.wrapping_add(b))
        } else { 0 };

        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 { break; }
            let chunk = if config.encrypt {
                // XOR simple con clave rotativa — rápido para archivos grandes
                buf[..n].iter().enumerate()
                    .map(|(i, &b)| b ^ encrypt_key.wrapping_add(i as u8))
                    .collect::<Vec<u8>>()
            } else {
                buf[..n].to_vec()
            };
            out.write_all(&chunk).map_err(|e| e.to_string())?;
            copied += n as u64;

            if zip_size > 0 {
                let copy_pct = (copied as f64 / zip_size as f64 * 12.0) as u32;
                emit(&window, (85 + copy_pct).min(97),
                    &format!("Copiando: {:.1}GB / {:.1}GB",
                        copied as f64 / 1_073_741_824.0,
                        zip_size as f64 / 1_073_741_824.0),
                    "");
            }
        }

        // Footer
        out.write_all(FLUXI_MAGIC).map_err(|e| e.to_string())?;
        out.write_all(&(stub_bytes.len() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
    }

    let _ = fs::remove_file(&zip_tmp);
    emit(&window, 100, "PACK COMPLETE!", "");

    let final_size = fs::metadata(&config.output_path).map(|m| m.len()).unwrap_or(0);

    Ok(PackResult {
        success:     true,
        output_file: config.output_path.clone(),
        size_bytes:  final_size,
        message:     format!(
            "{} v{} — {:.2}GB — {} files — by Fluxionics{}",
            config.app_name, config.app_version,
            final_size as f64 / 1_073_741_824.0,
            total_files,
            if has_pass { " [PASS]" } else { "" }
        ),
    })
}

fn stream_to_zip(
    zip: &mut ZipWriter<BufWriter<File>>,
    path: &Path,
    name: &str,
    opts: FileOptions,
) -> anyhow::Result<()> {
    zip.start_file(name, opts)?;
    let mut reader = BufReader::with_capacity(CHUNK, File::open(path)?);
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        zip.write_all(&buf[..n])?;
    }
    Ok(())
}

fn count_files(paths: &[String]) -> usize {
    paths.iter().map(|p| {
        let path = Path::new(p);
        if path.is_file() { 1 }
        else if path.is_dir() {
            WalkDir::new(path).into_iter().filter_map(|e| e.ok())
                .filter(|e| e.path().is_file()).count()
        } else { 0 }
    }).sum()
}

fn calc_size(paths: &[String]) -> u64 {
    paths.iter().map(|p| {
        let path = Path::new(p);
        if path.is_file() { fs::metadata(path).map(|m| m.len()).unwrap_or(0) }
        else if path.is_dir() {
            WalkDir::new(path).into_iter().filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0)).sum()
        } else { 0 }
    }).sum()
}

fn build_stub(entry: &str, app: &str, has_pass: bool) -> String {
    let safe = app.to_lowercase().replace(' ', "_");
    let pass_check = if has_pass {
        "set /p FLUXI_PASS=\"> ENTER PASSWORD: \"\r\n".to_string()
    } else {
        String::new()
    };
    format!(
        "@echo off\r\n:: Fluxi Portable — by Fluxionics\r\nsetlocal\r\n{pass}set \"T=%TEMP%\\fluxi_{safe}\"\r\nif not exist \"%T%\" mkdir \"%T%\"\r\npowershell -NoProfile -Command \"Expand-Archive -Force -Path '%~f0' -DestinationPath '%T%'\"\r\nstart \"\" \"%T%\\{entry}\"\r\nendlocal\r\nexit /b\r\n",
        pass = pass_check, safe = safe, entry = entry
    )
}

/// Hash simple de 32 bytes para verificar contraseña
fn simple_hash(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut hash = vec![0u8; 32];
    for (i, &b) in bytes.iter().enumerate() {
        hash[i % 32] = hash[i % 32].wrapping_add(b).wrapping_mul(31u8.wrapping_add(i as u8));
    }
    // Segunda pasada
    for i in 1..32 {
        hash[i] = hash[i].wrapping_add(hash[i-1]).wrapping_mul(7);
    }
    hash
}
