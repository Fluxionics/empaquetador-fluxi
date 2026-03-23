# Changelog — Fluxi by Fluxionics

## [1.0.0] — 2025

### ✨ Nuevo
- Motor de empaquetado completo en Rust con streaming
- ZIP64 activado — soporta archivos individuales y payloads de 32GB+
- Chunks de 8MB — nunca carga más de eso en RAM
- Progreso en tiempo real archivo por archivo desde Rust
- Contraseña de protección al ejecutar el portable
- Cifrado XOR del payload
- Extractor integrado — desempaqueta cualquier .fluxi.exe
- UI pixel art completa con fuente Press Start 2P
- Splash screen animada con logo Fluxionics
- Tema claro / oscuro toggle
- Ícono personalizado para el .exe generado
- Stats en GB: origen, estimado, final
- Consola en tiempo real estilo terminal

### 🏗️ Stack
- Frontend: HTML + CSS + JS (pixel art)
- Backend: Rust + Tauri v1
- Compresión: ZIP64/Deflate (zip crate)

### ⚠️ Notas
- El .exe generado es un batch script con ZIP embebido
- El extractor integrado soporta contraseña y cifrado
- Requiere Windows para ejecutar los portables generados

---
© 2025 Fluxionics — Apache 2.0
