# Fluxi v1.0.0

> **by Fluxionics** — Empaquetador portable de aplicaciones  
> Convierte cualquier programa en un único `.exe` sin instalación.

![Version](https://img.shields.io/badge/version-1.0.0-brightgreen)
![License](https://img.shields.io/badge/license-Apache%202.0-blue)
![Stack](https://img.shields.io/badge/stack-Rust%20%2B%20Tauri%20%2B%20HTML-orange)

---

## ⚙️ Requisitos antes de correrlo

Necesitas instalar estas herramientas **una sola vez**:

### 1. Rust
```bash
# Windows — abre PowerShell y ejecuta:
winget install Rustlang.Rustup

# O descarga directo:
# https://rustup.rs/
```

Verifica con:
```bash
rustc --version
cargo --version
```

---

### 2. Node.js (v18 o superior)
Descarga desde: https://nodejs.org/

Verifica con:
```bash
node --version
npm --version
```

---

### 3. Dependencias de sistema para Tauri (Windows)

Instala las **WebView2 Runtime** (necesario para Tauri en Windows):
- Descarga: https://developer.microsoft.com/en-us/microsoft-edge/webview2/

También necesitas **Microsoft C++ Build Tools**:
- Descarga: https://visualstudio.microsoft.com/visual-cpp-build-tools/
- Selecciona: `Desktop development with C++`

---

## 🚀 Cómo correr Fluxi

### Paso 1 — Clona el repositorio
```bash
git clone https://github.com/fluxionics/fluxi.git
cd fluxi
```

### Paso 2 — Instala dependencias de Node
```bash
npm install
```

### Paso 3 — Modo desarrollo (para probar)
```bash
npm run dev
```
Esto abre la ventana de Fluxi en modo desarrollo con recarga automática.

### Paso 4 — Compilar versión final
```bash
npm run build
```
El instalador `.msi` y el `.exe` compilado estarán en:
```
src-tauri/target/release/bundle/
```

---

## 📁 Estructura del proyecto

```
fluxi/
├── src/
│   └── index.html          ← Interfaz (HTML + CSS + JS)
├── src-tauri/
│   ├── src/
│   │   ├── main.rs         ← Entrada Rust + Tauri
│   │   └── packer.rs       ← Motor de empaquetado real
│   ├── build.rs
│   ├── Cargo.toml          ← Dependencias Rust
│   └── tauri.conf.json     ← Configuración Tauri
├── package.json
├── LICENSE                 ← Apache 2.0
└── README.md
```

---

## 🎯 Cómo usar Fluxi

1. Abre Fluxi
2. Arrastra tus archivos o usa los botones para seleccionarlos
3. Selecciona el **archivo principal** (el que se ejecuta al abrir)
4. Llena el nombre de la app y versión
5. Presiona **⚡ Empaquetar**
6. Obtén tu `.exe` portable listo para compartir

---

## 🗺️ Roadmap

- [x] v0.0.1 — Estructura base
- [x] v1.0.0 — Motor de empaquetado funcional + UI completa
- [ ] v1.1.0 — Ícono personalizado para el .exe generado
- [ ] v1.2.0 — Firma digital con certificado
- [ ] v1.3.0 — Soporte para Mac y Linux

---

## 🐛 Problemas comunes

**Error: `tauri` command not found**
```bash
npm install  # Asegúrate de haber corrido esto primero
```

**Error de compilación en Windows con C++**
- Instala Microsoft C++ Build Tools (ver requisitos arriba)

**La ventana no abre**
- Verifica que tienes WebView2 Runtime instalado

---

## 📄 Licencia

Apache 2.0 © 2025 **Fluxionics**

> "Fluxi" y "Fluxionics" son marcas de Fluxionics.  
> No uses estos nombres en productos derivados sin permiso.
