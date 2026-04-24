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

## 3) 打包与使用（统一流程）

统一使用打包产物，避免版本混淆。

### 一键打包全部

```cmd
scripts\package-all.cmd
```

输出：

```text
dist/CanvasTerminal.exe
dist/canvas.exe
dist/SKILL.md
```

### 运行约定

- App 使用：`dist\CanvasTerminal.exe`
- CLI 一律使用：`dist\canvas.exe`
- 不建议直接使用裸命令 `canvas`（可能命中 PATH 里的旧版本）

---

## 4) 自动化调试接口

- 协议文档：`docs/automation-protocol.md`
- Debug API：`POST /automation`
- CLI 示例：

```cmd
dist\canvas.exe debug graph get --pretty
dist\canvas.exe debug node create --kind text --x 120 --y 100 --text "hello"
dist\canvas.exe debug inject terminal --node-id 2 --command "echo smoke" --wait --timeout 5000
```

E2E smoke（可选，要求 app 正在运行）：

```cmd
scripts\e2e-smoke.cmd
```
