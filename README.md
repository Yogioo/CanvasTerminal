# 安装与运行

## 1) 安装 Rust（含 cargo）

### Windows
在 PowerShell 执行：

```powershell
winget install -e --id Rustlang.Rustup
```

安装后重开终端，验证：

```powershell
rustc --version
cargo --version
```

### macOS / Linux

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

安装后重开终端，验证：

```bash
rustc --version
cargo --version
```

---

## 2) 运行项目

```bash
git clone https://github.com/Yogioo/CanvasTerminal
cd CanvasTerminal
cargo run
```

---

## 3) 打包

### 仅打包主程序

```cmd
scripts\package-canvas-app.cmd
```

输出：

```text
dist/app/CanvasTerminal.exe
```

### 仅打包 agent skill + CLI

```cmd
scripts\package-canvas-agent-kit.cmd
```

输出：

```text
dist/canvas_skills/canvas-agent-events/
```

其中包含：

```text
SKILL.md
bin/canvas.exe
scripts/canvas.cmd
```

### 一键打包全部

```cmd
scripts\package-all.cmd
```

输出：

```text
dist/app/CanvasTerminal.exe
dist/canvas_skills/canvas-agent-events/
```

### 仅重新编译 CLI（开发用）

Debug:

```cmd
scripts\build-canvas-cli.cmd
```

Release:

```cmd
scripts\build-canvas-cli-release.cmd
```
