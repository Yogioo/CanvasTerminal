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
