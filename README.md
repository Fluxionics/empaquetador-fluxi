# Fluxi v1.0.0

> **by Fluxionics** — Empaquetador portable de aplicaciones  
> Convierte cualquier programa en un único `.exe` sin instalación.

![Version](https://img.shields.io/badge/version-1.0.0-brightgreen)
![License](https://img.shields.io/badge/license-Apache%202.0-blue)
![Stack](https://img.shields.io/badge/stack-Rust%20%2B%20Tauri%20%2B%20HTML-orange)
![Platform](https://img.shields.io/badge/platform-Windows-informational)
![ZIP64](https://img.shields.io/badge/ZIP64-32GB%2B-success)

---

## ¿Qué es Fluxi?

**Fluxi** es una herramienta open source creada por **Fluxionics** que convierte cualquier programa, instalador o conjunto de archivos en **un único archivo `.exe` portable** — sin carpetas ocultas, sin instalación, sin rastros.

---

## ✨ Características

- 📦 Empaqueta cualquier archivo o carpeta en un único `.exe`
- 🚀 Motor streaming — soporta **32GB+** sin congelarse ni usar RAM
- 🔐 Contraseña de protección al ejecutar
- 🔒 Cifrado del payload
- 📂 Extractor integrado — desempaqueta cualquier `.fluxi.exe`
- 🎨 UI pixel art con tema claro/oscuro
- 🖼️ Ícono personalizado para el `.exe` generado
- 📊 Progreso en tiempo real archivo por archivo
- 🆓 100% Open Source — Apache 2.0

---

## 🛠️ Stack tecnológico

| Capa | Tecnología |
|------|-----------|
| Interfaz | HTML + CSS + JS (pixel art) |
| Backend | Rust |
| Bridge | Tauri v1 |
| Compresión | ZIP64 / Deflate |

---

## 📥 Instalación para desarrollo

### Requisitos

**1. Rust**
```bash
# Descarga desde https://rustup.rs/
rustc --version  # verificar
```

**2. Node.js v18+**
```bash
# Descarga desde https://nodejs.org/
node --version   # verificar
```

**3. WebView2 Runtime (Windows)**
- Descarga el `Evergreen Bootstrapper` desde:
- https://developer.microsoft.com/en-us/microsoft-edge/webview2/

**4. Microsoft C++ Build Tools**
- https://visualstudio.microsoft.com/visual-cpp-build-tools/
- Selecciona: `Desktop development with C++`

---

## 🚀 Correr en desarrollo

```bash
git clone https://github.com/fluxionics/empaquetador-fluxi.git
cd empaquetador-fluxi
npm install
npm run dev
```

---

## 📦 Compilar release

```bash
# Genera instalador .msi y .exe en Windows
npm run build
```

El resultado estará en:
```
src-tauri/target/release/bundle/
├── msi/    ← Fluxi_1.0.0_x64_en-US.msi
└── nsis/   ← Fluxi_1.0.0_x64-setup.exe
```

> **Nota:** El `.dmg` para Mac solo se puede compilar desde una Mac.

---

## 🎯 Cómo usar Fluxi

1. Abre Fluxi
2. Arrastra tus archivos o usa los botones para seleccionarlos
3. Selecciona el **archivo principal** (entry point)
4. Opcionalmente: agrega ícono, contraseña o cifrado
5. Presiona **[ EMPAQUETAR ]**
6. Elige dónde guardar el `.exe` portable
7. ¡Listo! Un único `.exe` para compartir

### Extraer un .fluxi.exe
1. Ve a la pestaña **[ EXTRAER ]**
2. Selecciona el `.exe` empaquetado
3. Elige carpeta de destino
4. Ingresa contraseña si aplica
5. Presiona **[ EXTRAER ARCHIVOS ]**

---

## 📁 Estructura del proyecto

```
fluxi/
├── src/
│   └── index.html              ← UI pixel art completa
├── src-tauri/
│   ├── src/
│   │   ├── main.rs             ← Entrada Rust + Tauri
│   │   ├── packer.rs           ← Motor de empaquetado ZIP64
│   │   └── extractor.rs        ← Desempaquetador
│   ├── icons/
│   │   ├── icon.ico
│   │   ├── icon.png
│   │   └── logotipo.png        ← Logo Fluxionics
│   ├── build.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE                     ← Apache 2.0
└── README.md
```

---

## 🗺️ Roadmap

- [x] v0.0.1 — Estructura base
- [x] v1.0.0 — Motor streaming ZIP64, UI pixel art, extractor, contraseña
- [ ] v1.1.0 — Firma digital del `.exe` generado
- [ ] v1.2.0 — Soporte Mac/Linux
- [ ] v1.3.0 — CLI (línea de comandos)

---

## 🐛 Problemas comunes

**`tauri` command not found**
```bash
npm install
```

**Error de compilación C++**
Instala Microsoft C++ Build Tools (ver requisitos)

**La ventana no abre**
Verifica WebView2 Runtime instalado

**Archivo muy grande (>4GB individual)**
ZIP64 está activado por defecto — debería funcionar automáticamente

---

## 🤝 Contribuir

Lee [CONTRIBUTING.md](CONTRIBUTING.md) para guía de contribución.

---

## 📄 Licencia

Apache 2.0 © 2025 **Fluxionics**

> "Fluxi" y "Fluxionics" son marcas de Fluxionics.  
> No uses estos nombres en productos derivados sin permiso.
