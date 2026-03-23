// extractor.rs — Desempaquetador de .fluxi.exe
// Fluxi v1.0.0 — Fluxionics — Apache 2.0

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use tauri::Window;

const FLUXI_MAGIC: &[u8] = b"FLUXI_FLUXIONICS_V100\x00";
const CHUNK: usize = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnpackConfig {
    pub exe_path:    String,
    pub output_dir:  String,
    pub password:    Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnpackResult {
    pub success:     bool,
    pub output_dir:  String,
    pub files_count: usize,
    pub app_name:    String,
    pub message:     String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProgressEvent {
    pub percent:      u32,
    pub message:      String,
    pub current_file: String,
}

fn emit(window: &Window, pct: u32, msg: &str, file: &str) {
    let _ = window.emit("pack_progress", ProgressEvent {
        percent: pct.min(100),
        message: msg.to_string(),
        current_file: file.to_string(),
    });
}

/// Desempaqueta un .fluxi.exe generado por Fluxi
#[tauri::command]
pub async fn unpack_exe(window: Window, config: UnpackConfig) -> Result<UnpackResult, String> {
    emit(&window, 5, "Abriendo archivo...", "");

    let exe_path = Path::new(&config.exe_path);
    if !exe_path.exists() {
        return Err(format!("Archivo no encontrado: {}", config.exe_path));
    }

    let exe_size = fs::metadata(exe_path).map(|m| m.len()).unwrap_or(0);
    let mut file = BufReader::new(
        File::open(exe_path).map_err(|e| e.to_string())?
    );

    emit(&window, 10, "Buscando firma Fluxionics...", "");

    // Buscar MAGIC desde el final del archivo
    // Footer: [MAGIC 22B][STUB_LEN u32 4B] = 26 bytes
    if exe_size < 26 {
        return Err("Archivo demasiado pequeño — no es un .fluxi.exe válido".to_string());
    }

    file.seek(SeekFrom::End(-26)).map_err(|e| e.to_string())?;
    let mut footer_magic = vec![0u8; 22];
    let mut stub_len_bytes = [0u8; 4];
    file.read_exact(&mut footer_magic).map_err(|e| e.to_string())?;
    file.read_exact(&mut stub_len_bytes).map_err(|e| e.to_string())?;

    if footer_magic != FLUXI_MAGIC {
        return Err("No es un archivo Fluxi válido — firma no encontrada".to_string());
    }

    let stub_len = u32::from_le_bytes(stub_len_bytes) as u64;

    // Leer desde después del stub
    // Layout: [STUB][MAGIC][PASS_HASH 32B][META_LEN u32][META][ZIP_SIZE u64][ZIP][MAGIC][STUB_LEN u32]
    file.seek(SeekFrom::Start(stub_len)).map_err(|e| e.to_string())?;

    let mut magic_check = vec![0u8; 22];
    file.read_exact(&mut magic_check).map_err(|e| e.to_string())?;
    if magic_check != FLUXI_MAGIC {
        return Err("Estructura interna inválida".to_string());
    }

    // Leer hash de contraseña
    let mut pass_hash = vec![0u8; 32];
    file.read_exact(&mut pass_hash).map_err(|e| e.to_string())?;

    let has_pass = pass_hash.iter().any(|&b| b != 0);
    if has_pass {
        let provided = config.password.as_deref().unwrap_or("");
        if provided.is_empty() {
            return Err("Este archivo está protegido con contraseña".to_string());
        }
        let provided_hash = simple_hash(provided);
        if provided_hash != pass_hash {
            return Err("Contraseña incorrecta".to_string());
        }
    }

    emit(&window, 20, "Leyendo metadatos...", "");

    // Metadata
    let mut meta_len_b = [0u8; 4];
    file.read_exact(&mut meta_len_b).map_err(|e| e.to_string())?;
    let meta_len = u32::from_le_bytes(meta_len_b) as usize;

    let mut meta_bytes = vec![0u8; meta_len];
    file.read_exact(&mut meta_bytes).map_err(|e| e.to_string())?;

    let metadata: serde_json::Value = serde_json::from_slice(&meta_bytes)
        .unwrap_or(serde_json::json!({}));

    let app_name = metadata["app_name"].as_str().unwrap_or("Unknown").to_string();
    let encrypted = metadata["encrypted"].as_bool().unwrap_or(false);
    let entry_point = metadata["entry_point"].as_str().unwrap_or("").to_string();

    emit(&window, 28, &format!("Extrayendo: {}", app_name), "");

    // ZIP size
    let mut zip_size_b = [0u8; 8];
    file.read_exact(&mut zip_size_b).map_err(|e| e.to_string())?;
    let zip_size = u64::from_le_bytes(zip_size_b);

    // Extraer ZIP a archivo temporal
    let zip_tmp = PathBuf::from(&config.output_dir).join("fluxi_extract_tmp.zip");
    fs::create_dir_all(&config.output_dir).map_err(|e| e.to_string())?;

    {
        let out_f = File::create(&zip_tmp).map_err(|e| e.to_string())?;
        let mut out = BufWriter::with_capacity(CHUNK, out_f);
        let mut buf = vec![0u8; CHUNK];
        let mut copied = 0u64;

        let encrypt_key: u8 = if encrypted {
            config.password.as_deref().unwrap_or("fluxi").bytes()
                .fold(0xABu8, |acc, b| acc.wrapping_add(b))
        } else { 0 };

        loop {
            let to_read = CHUNK.min((zip_size - copied) as usize);
            if to_read == 0 { break; }
            let n = file.read(&mut buf[..to_read]).map_err(|e| e.to_string())?;
            if n == 0 { break; }

            let chunk: Vec<u8> = if encrypted {
                buf[..n].iter().enumerate()
                    .map(|(i, &b)| b ^ encrypt_key.wrapping_add(i as u8))
                    .collect()
            } else {
                buf[..n].to_vec()
            };

            out.write_all(&chunk).map_err(|e| e.to_string())?;
            copied += n as u64;

            if zip_size > 0 {
                let pct = 28 + (copied as f64 / zip_size as f64 * 50.0) as u32;
                emit(&window, pct.min(78),
                    &format!("Extrayendo: {:.1}GB / {:.1}GB",
                        copied as f64 / 1_073_741_824.0,
                        zip_size as f64 / 1_073_741_824.0), "");
            }
        }
        out.flush().map_err(|e| e.to_string())?;
    }

    emit(&window, 80, "Descomprimiendo archivos...", "");

    // Descomprimir ZIP
    let zip_file = File::open(&zip_tmp).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(zip_file).map_err(|e| e.to_string())?;
    let total = archive.len();

    for i in 0..total {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let out_path = PathBuf::from(&config.output_dir).join(entry.name());
        let pct = 80 + ((i as f32 / total.max(1) as f32) * 18.0) as u32;
        emit(&window, pct.min(98), &format!("Extrayendo: {}", entry.name()), entry.name());

        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out_f = BufWriter::new(File::create(&out_path).map_err(|e| e.to_string())?);
            let mut buf = vec![0u8; CHUNK];
            loop {
                let n = entry.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 { break; }
                out_f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            }
        }
    }

    let _ = fs::remove_file(&zip_tmp);
    emit(&window, 100, "EXTRACCION COMPLETA!", "");

    Ok(UnpackResult {
        success:     true,
        output_dir:  config.output_dir.clone(),
        files_count: total,
        app_name:    app_name.clone(),
        message:     format!("{} extraído — {} archivos — by Fluxionics", app_name, total),
    })
}

fn simple_hash(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut hash = vec![0u8; 32];
    for (i, &b) in bytes.iter().enumerate() {
        hash[i % 32] = hash[i % 32].wrapping_add(b).wrapping_mul(31u8.wrapping_add(i as u8));
    }
    for i in 1..32 { hash[i] = hash[i].wrapping_add(hash[i-1]).wrapping_mul(7); }
    hash
}
