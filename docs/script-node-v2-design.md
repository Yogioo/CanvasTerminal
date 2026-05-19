# Script Node V2: Lua 可编程节点 架构设计

> **目标**: 将 Script 节点从"JSON 声明式 UI"**彻底重写**为"纯 Lua 可编程节点"。所有用户交互、状态管理、UI 渲染、消息处理全部通过 Lua 脚本完成。
>
> **V1 → V2**: JSON Widget 系统（col/row/text/button/slider/input/bar/badge/divider/spacer/image）**不再暴露给用户**。底层的渲染函数保留，但由 Lua 的 `ctx` API 内部调用，用户只写 Lua。

## 短期目标（MVP）

**以当前默认 JSON Script 节点（审批队列行为）为基准，用 Lua 完全复现其功能，且不硬编码任何 UI/逻辑。**

即：

```lua
-- 用户写这样的脚本，而非硬编码在 Rust 里
state = { queue = {} }

function on_input(name, value)
  table.insert(state.queue, value)
end

function render(ctx)
  -- 用户完全自由决定 UI 布局、按钮数量、逻辑
  if ctx:button("✓ 批准") then
    emit("approve", table.remove(state.queue, 1))
  end
end
```

**验证标准（MVP 通过条件）**:

| # | 验收项 | 说明 |
|---|--------|------|
| 1 | 消息入队 | 上游节点发来的消息进入 Lua `state.queue`，触发 `on_input` |
| 2 | 队列显示 | `render(ctx)` 显示队列数量和首条消息内容 |
| 3 | 单条审批 | 点击「批准」按钮消耗队列首条消息，通过 `emit("approve", msg)` 转发到下游 |
| 4 | 批量审批 | 点击「全部批准」遍历整个队列，逐条 `emit` 到下游 |
| 5 | 驳回 | 点击「驳回」消耗首条消息，转发到 `reject` 端口 |
| 6 | 空队列保护 | 队列为空时按钮灰化（`enabled` 条件），不报错 |
| 7 | 状态持久化 | 关闭/重新打开画布后，`state.queue` 内容正确恢复 |
| 8 | 端口映射 | `ports.inputs.input` 接收上游消息，`ports.outputs.approve/reject` 正确路由 |
| 9 | 无硬编码 | 上述所有行为均来自用户 Lua 脚本，Rust 侧不预设任何 UI/逻辑 |

---

## 目录

1. [Why Lua](#1-why-lua)
2. [整体架构](#2-整体架构)
3. [Lua 沙箱设计](#3-lua-沙箱设计)
4. [用户 API 参考](#4-用户-api-参考)
5. [类型系统](#5-类型系统)
6. [时序模型](#6-时序模型)
7. [序列化与持久化](#7-序列化与持久化)
8. [实现计划](#8-实现计划)
9. [完整示例](#9-完整示例)
10. [边界情况与安全](#10-边界情况与安全)

---

## 1. Why Lua

| 考量 | 结论 |
|------|------|
| **体积** | `mlua` + Lua 5.4 静态链接约 200KB，对 CanvasTerminal 可忽略 |
| **启动速度** | Lua 解析器 ~0 开销，适合 egui 每帧调用 |
| **安全性** | `mlua` 提供沙箱能力（限制 `require`、`io`、`os.execute` 等危险函数） |
| **语法** | 用户熟悉 Lua，学习曲线最低 |
| **Rust 绑定** | `mlua` 是最成熟的 Rust Lua 绑定库（3K+ stars，450 万+ 下载） |
| **宿主集成** | Rust ↔ Lua 双向调用，值自动转换（String/Number/Table ↔ Rust 类型） |
| **V1 复用** | 底层的 Widget 渲染代码（text/button/slider 等绘制函数）完全复用，只需桥接 |

### 为什么不是 Rhai / QuickJS / WASM

| 方案 | 问题 |
|------|------|
| Rhai | 用户不熟悉，生态小，复杂表格操作不方便 |
| QuickJS | 编译慢（需 C 编译器和 bindgen），体积 ~1MB+ |
| WASM | 太重，引入 Wasmtime 增加 ~20MB + 编译时间 |
| **Lua (mlua)** ✅ | **最轻、最快、用户最熟** |

---

## 2. 整体架构

### 2.1 数据流

```
V1（已废弃）:
  JSON 字符串 → serde_json → ScriptNodeSpec → Widget tree → layout_render → 屏幕

V2（新架构）:
  Lua 源码 → mlua 沙箱 → 注册 ports/state/render/on_tick
                          ↓
              render(ctx) → ctx:button()/text() 等方法
                          ↓
                    Widget tree（内部类型，用户不可见）
                          ↓
                    layout_render（完全复用 V1 渲染代码）
                          ↓
                         屏幕
```

### 2.2 运行时层次

```
┌─────────────────────────────────────────────────────┐
│                  NodeData::Script                    │
│  { code: "Lua 源码全文", serialized_state: "JSON" } │
├─────────────────────────────────────────────────────┤
│                     LuaRuntime                       │
│  管理: Lua 状态 | 沙箱 | ctx API 注册 | 缓存         │
├─────────────────────────────────────────────────────┤
│    ports 表    state 表    render()    on_tick()    │
│       │           │           │           │         │
│       ▼           ▼           ▼           ▼         │
│  Port system  持久化      layout_render   定时器     │
│  (边缘路由)   (JSON)     (V1 代码复用)   (egui 驱动)│
└─────────────────────────────────────────────────────┘
```

### 2.3 每帧执行流程

```
egui 帧开始
  │
  ├─ LuaRuntime::before_frame(node_id)
  │    ├─ 反序列化 state (JSON → Lua table 合并)
  │    ├─ 处理 pending_messages → 调用 on_input(port, value)
  │    └─ 如果定时器到期 → 调用 on_tick(dt)
  │
  ├─ render(ctx)
  │    ├─ ctx:button()/text() 等方法 → 生成内部 Widget 列表
  │    ├─ emit() → 消息缓冲
  │    └─ 用户代码修改 state 表
  │
  ├─ Widget 列表 → layout_render::render() → 屏幕输出
  │
  ├─ LuaRuntime::after_frame(node_id)
  │    ├─ 序列化 state → JSON → NodeData.serialized_state
  │    ├─ 下发 emit 消息到下游节点
  │    └─ 请求 repaint_after(interval) 如果定时器活跃
  │
  └─ egui 帧结束
```

### 2.4 文件结构

```
src/script_node/
├── mod.rs                  # 入口，导出 LuaRuntime，废弃 V1 JSON 相关函数
├── types.rs                # 保留 Widget/Style/Theme/ColorSpec（内部类型，用户不可见）
├── layout_render.rs        # 保留（Lua ctx 底层调用这些渲染函数）
├── lua/
│   ├── mod.rs              # LuaRuntime 主结构体，生命周期管理
│   ├── sandbox.rs          # 沙箱安全（白名单/黑名单/指令限制/内存限制）
│   ├── api_ctx.rs          # ctx:text/button/slider/input 等渲染 API
│   ├── api_system.rs       # emit/set_timer/log 等系统 API
│   ├── state.rs            # state ↔ JSON 序列化/反序列化
│   └── timer.rs            # TimerManager，定时器状态跟踪
└── examples/
    ├── pomodoro.lua        # 番茄钟模板
    ├── notepad.lua         # 笔记节点模板
    └── approval.lua        # 审批队列模板
```

---

## 3. Lua 沙箱设计

### 3.1 安全目标

- 用户不能访问文件系统（`io.*`, `loadfile`, `dofile`）
- 用户不能执行系统命令（`os.execute`, `os.exit`）
- 用户不能 `require` 外部模块
- 用户不能无限循环或耗尽内存
- 每个节点独立 Lua 状态，互不干扰

### 3.2 白名单全局函数

```lua
-- 数学
math.*       -- 全部开放（sin, cos, floor, random, max, min 等）
-- 字符串
string.*     -- 全部开放（sub, format, match, gsub, reverse, rep 等）
-- 表格
table.*      -- 全部开放（insert, remove, sort, concat 等）
-- 基础
type, pairs, ipairs, next, select, tostring, tonumber
pcall, xpcall, error, assert, unpack
-- 时间
os.date, os.time, os.difftime
-- 调试辅助
print        -- 重定向到 CanvasTerminal 控制台
```

### 3.3 被禁止的全局

```lua
io.*            -- 文件系统
loadfile        -- 加载文件
dofile          -- 执行文件
require         -- 模块加载
package.*       -- 模块系统
os.execute      -- 执行命令
os.exit         -- 退出
os.rename       -- 重命名文件
os.remove       -- 删除文件
os.tmpname      -- 临时文件
debug.*         -- 调试反射
```

### 3.4 执行限制

| 限制 | 实现方式 | 默认值 |
|------|---------|--------|
| 指令计数 | `mlua::Lua::set_hook(InstructionLimit)` | 每帧 500,000 条指令 |
| 执行时间 | 帧级超时 | 每帧 < 5ms |
| 内存 | `mlua::Lua::set_memory_limit()` | 每节点 8MB |
| 递归深度 | Lua 原生保护 + hook 检测 | 200 层 |

---

## 4. 用户 API 参考

### 4.1 节点定义（全局变量约定式）

用户通过定义特定名称的全局变量和函数来声明节点行为。

#### `ports`（表，可选）

声明输入/输出端口，映射到 Canvas 的边缘路由系统。

```lua
ports = {
  inputs = {
    trigger = { type = "string", description = "启动信号" },
    config  = { type = "string" },
  },
  outputs = {
    result   = { type = "string" },
    progress = { type = "number" },
  }
}

-- 不定义 ports = 纯展示节点，无数据流参与
```

`type` 支持: `"string"` | `"number"` | `"boolean"` | `"any"`（默认）

#### `state`（表，必选）

持久化状态。**每帧渲染前恢复，渲染结束后自动序列化保存。**

```lua
state = {
  count    = 0,
  running  = false,
  buffer   = "",
  notes    = {},
  mode     = "work",
}
```

> ⚠️ `state` 每帧序列化为 JSON 存盘。不可包含 function / userdata / 循环引用。

#### `on_init()`（可选，仅调用一次）

节点首次创建或序列化重建后调用一次。适合初始化复杂状态。

```lua
function on_init()
  state.remaining = 25 * 60
  state.running   = false
end
```

#### `render(ctx)`（可选，推荐实现）

**每帧调用**，用 `ctx` 方法声明 UI。

```lua
function render(ctx)
  ctx:text("Hello, Lua!")
  if ctx:button("点击") then
    state.count = state.count + 1
    emit("result", "点击了 " .. state.count .. " 次")
  end
end
```

#### `on_tick(dt)`（可选）

定时器回调。`dt` = 距上次 tick 的秒数（float）。定义了此函数，定时器自动以 1 秒为间隔驱动。

```lua
function on_tick(dt)
  if not state.running then return end
  state.remaining = state.remaining - dt
  if state.remaining <= 0 then
    state.running = false
    emit("done", "时间到！")
  end
end
```

#### `on_input(port_name, value)`（可选）

上游节点向本节点某个 input 端口发送消息时触发。

```lua
function on_input(name, value)
  if name == "trigger" then
    state.running = true
  end
end
```

### 4.2 渲染 API (`ctx.*`)

所有方法返回 `ctx` 自身支持链式调用，但典型用法是直接调用。

#### `ctx:text(text, opts?)`

```lua
ctx:text("Hello World", {
  font_size = 18,          -- 默认: theme.font_size
  bold = true,
  color = "$accent",       -- 或 "#ff6b6b"
  align = "center",        -- "left" | "center" | "right"
  width = { type = "px", value = 50 },  -- 固定宽度（可选）
})
```

`width` 支持格式：`{ type = "px", value = number }` 或 `{ type = "fr", value = number }`。

#### `ctx:button(text, opts?) -> boolean`

返回 `true` 表示本帧被点击。

```lua
if ctx:button("提交", { bg = "$success", color = "#000000" }) then
  -- 处理点击
end

-- 带禁用条件
if ctx:button("保存", { enabled = #state.notes > 0 }) then
  -- 按钮在条件不满足时灰化
end
```

#### `ctx:input(opts?) -> string`

返回当前输入框的值。

```lua
local text = ctx:input({
  label       = "笔记",
  placeholder = "写点什么...",
  multiline   = true,    -- 多行模式
  rows        = 5,
  value       = state.buffer,  -- 绑定值
})
state.buffer = text
```

#### `ctx:slider(opts?) -> number`

```lua
local vol = ctx:slider({
  label = "音量",
  min = 0, max = 100,
  value = state.volume,
})
state.volume = vol
```

#### `ctx:progress_bar(value, opts?)`

```lua
ctx:progress_bar(state.remaining / total, {
  height = 12,        -- 进度条高度
  fill   = "$accent", -- 填充色（可选），支持主题色或 "#rrggbb"
})
```

`fill` 支持主题色变量（`$accent`、`$success`、`$danger`、`$warning` 等）或十六进制颜色值。

#### `ctx:separator(opts?)`

```lua
ctx:separator()
ctx:separator({ color = "#666" })
```

#### `ctx:badge(text, opts?)`

```lua
ctx:badge("进行中", { color = "$accent" })
```

#### `ctx:spacer(height?)`

```lua
ctx:spacer(16)
```

#### `ctx:row(opts?, fn)` / `ctx:col(opts?, fn)`

布局容器。`fn` 的参数是子 `ctx`，所有子组件在容器内渲染。

```lua
ctx:row({ gap = 8 }, function(sub)
  sub:button("确认")
  sub:button("取消")
end)

ctx:col({ gap = 4, padding = { 8, 8, 8, 8 } }, function(sub)
  sub:text("标题", { font_size = 20, bold = true })
  sub:text("正文内容")
end)
```

#### `ctx:card(text, opts?)`

卡片容器，自带背景/圆角/内边距。

```lua
ctx:card("这是一段笔记", { caption = "2026-05-19" })
```

### 4.3 系统 API

#### `emit(port_name, value)`

向某个 output 端口发送消息。可在 `render()` / `on_tick()` / `on_input()` 中调用。

```lua
emit("result",   "处理完成")
emit("progress", 0.75)
```

#### 定时器控制

```lua
set_timer(0.5)          -- 设置 tick 间隔（秒），0 = 停止
clear_timer()           -- 停止定时器
local interval = get_timer_interval()  -- 查询当前间隔，0 = 未激活
```

**注意**: 定义了 `on_tick` 函数后，定时器自动以 1 秒间隔启动，无需手动调用 `set_timer`。

#### `log(...)`

向 CanvasTerminal 控制台输出调试信息（仅开发模式显示）。

```lua
log("当前计数:", state.count)
```

---

## 5. 类型系统

### 5.1 Lua ↔ Rust 类型映射

| Lua 类型 | Rust 类型 (mlua) | JSON 序列化 |
|----------|------------------|-------------|
| `nil` | `()` | `null` |
| `boolean` | `bool` | `true` / `false` |
| `number` | `f64` | `3.14` |
| `string` | `String` | `"hello"` |
| `table` (数组) | `Vec<Value>` | `[...]` |
| `table` (字典) | `HashMap<String, Value>` | `{...}` |
| `function` | **不可序列化** | ❌ |

### 5.2 `state` 序列化约束

```lua
-- ✅ 合法
state = {
  count = 0,
  items = { "a", "b" },
  config = { enabled = true, timeout = 30 },
}

-- ❌ 非法（序列化失败）
state = {
  cb = function() end,   -- function 不可 JSON
  self_ref = state,       -- 循环引用
}
```

**实现**: `LuaRuntime` 序列化前做递归检查，遇到不可序列化的值跳过并打印告警。

### 5.3 ports 端口类型

```lua
ports = {
  inputs = {
    data  = { type = "string", description = "输入数据" },
    count = { type = "number", default = 0 },
    flag  = { type = "boolean" },
  },
  outputs = {
    result   = { type = "string" },
    progress = { type = "number" },
  }
}
```

---

## 6. 时序模型

### 6.1 定时器实现

```rust
// lua/timer.rs
pub struct TimerManager {
    intervals: HashMap<usize, f64>,  // 节点 ID → 间隔秒数，0 = 未激活
    last_tick: HashMap<usize, Instant>,
}

impl TimerManager {
    pub fn poll(&mut self, node_id: usize, now: Instant) -> Option<f64> {
        let interval = self.intervals.get(&node_id)?;
        if *interval <= 0.0 { return None; }
        let last = self.last_tick.entry(node_id).or_insert(now);
        let elapsed = (now - *last).as_secs_f64();
        if elapsed >= *interval {
            *last = now;
            Some(elapsed)
        } else { None }
    }

    pub fn set(&mut self, node_id: usize, interval: f64) { ... }
    pub fn clear(&mut self, node_id: usize) { ... }
}
```

### 6.2 egui 驱动

```rust
// 在渲染循环中
if timer_manager.has_active_timer(node_id) {
    let interval = timer_manager.get_interval(node_id).unwrap();
    ctx.request_repaint_after(Duration::from_secs_f64(interval));
}
```

---

## 7. 序列化与持久化

### 7.1 NodeData 存储格式

```rust
// model.rs
NodeData::Script {
    title: String,
    /// Lua 源码全文（用户写的内容）
    code: String,
    /// 消息队列
    pending_messages: Vec<String>,
    /// state 的快照（JSON 字符串），每帧更新
    serialized_state: Option<String>,
    /// 运行时缓存，不序列化
    #[serde(skip)]
    runtime_cache: Option<LuaRuntimeCache>,
}
```

### 7.2 初始化流程

```
1. 从 NodeData 读取 code + serialized_state
2. 创建新 LuaRuntime + 沙箱
3. 执行 Lua 源码：
   a. ports = {...}         → 注册端口
   b. state = {...}         → 初始值
   c. on_init()             → 复杂初始化
4. 如果 serialized_state 存在:
   → JSON 反序列化 → 递归合并覆盖 state 中的对应字段
5. 缓存 runtime 到 HashMap<node_id, LuaRuntime>
```

### 7.3 保存流程（每帧帧尾）

```
1. 从 Lua 中读取 state 表
2. 递归遍历，收集可 JSON 序列化的值
3. 丢弃 function/userdata/循环引用
4. 序列化为 JSON 字符串
5. 存入 NodeData.serialized_state
```

---

## 8. 实现计划

### Phase 1: 基础设施（~400 行）

| 模块 | 文件 | 内容 |
|------|------|------|
| 依赖 | `Cargo.toml` | `cargo add mlua --features lua54` |
| LuaRuntime | `lua/mod.rs` | 创建/缓存 Lua 状态，沙箱初始化，节点生命周期 |
| 沙箱 | `lua/sandbox.rs` | 白名单/黑名单，指令计数 hook，内存限制 |
| 状态 | `lua/state.rs` | state ↔ JSON 序列化/反序列化，递归合并 |
| 定时器 | `lua/timer.rs` | TimerManager，poll/set/clear |

**验证**: 执行 `state = {count=0}; function render(ctx) ... end` 后能正确序列化 state。

### Phase 2: 渲染 API（~500 行）

| API | 文件 | 底层调用 |
|-----|------|---------|
| `ctx:text` | `lua/api_ctx.rs` | `render_text()` |
| `ctx:button` | `lua/api_ctx.rs` | `render_button()` + 事件回调 |
| `ctx:input` | `lua/api_ctx.rs` | `render_input()` + 双向绑定 |
| `ctx:slider` | `lua/api_ctx.rs` | `render_slider()` |
| `ctx:progress_bar` | `lua/api_ctx.rs` | `render_bar()` |
| `ctx:row` / `ctx:col` | `lua/api_ctx.rs` | `render_row()` / `render_col()` |
| `ctx:separator` / `badge` / `spacer` / `card` | `lua/api_ctx.rs` | 各自渲染函数 |
| `emit` / `set_timer` / `log` | `lua/api_system.rs` | 系统调用 |

**验证**: 在 Lua 中写 `render(ctx)` 能看到 UI 正确渲染并交互。

### Phase 3: 生命周期集成（~200 行）

- 修改 `canvas_nodes_render.rs` 中的 `draw_script_node_body()` → 完全走 Lua 路径
- 实现 `before_frame()` / `after_frame()` hook
- 消息路由: `on_input()` 调用 + `emit()` 转发
- 定时器: 与 `ctx.request_repaint_after()` 集成

**验证**: 番茄钟能在 Canvas 中正常运行。

### Phase 4: 编辑器体验（~150 行）

- Lua 语法高亮（替换 TextEdit 或加 layouter）
- 编译错误 → 红色提示面板
- 右键菜单 → "插入代码片段"（番茄钟/笔记/审批）
- 默认模板改为 Lua 版审批队列

**验证**: 创建新 Script 节点直接显示 Lua 编辑器。

### Phase 5: 废弃 V1 代码（清理）

- 删除 `parser.rs`（JSON 解析器）
- 删除 `default_script_template()`（V1 JSON 模板）
- 清理 `types.rs` 中不再使用的公开接口
- `layout_render.rs` 的渲染函数改为 `pub(crate)` 仅被 Lua 模块调用

---

## 9. 完整示例

### 9.1 🍅 番茄钟

```lua
ports = {
  inputs = {
    start = { type = "string", description = "启动信号" },
    stop  = { type = "string", description = "停止信号" },
  },
  outputs = {
    done = { type = "string", description = "时间到" },
  }
}

state = {
  remaining = 25 * 60,
  running   = false,
  mode      = "work",   -- "work" | "break"
}

function on_tick(dt)
  if not state.running then return end
  state.remaining = state.remaining - dt
  if state.remaining <= 0 then
    state.remaining = 0
    state.running = false
    emit("done", state.mode == "work" and "工作完成" or "休息结束")
    if state.mode == "work" then
      state.mode = "break"
      state.remaining = 5 * 60
    else
      state.mode = "work"
      state.remaining = 25 * 60
    end
  end
end

function on_input(name)
  if name == "start" then state.running = true
  elseif name == "stop" then state.running = false end
end

function render(ctx)
  local mins = math.floor(state.remaining / 60)
  local secs = math.floor(state.remaining % 60)

  ctx:col({ gap = 8, padding = { 12, 12, 12, 12 } }, function(sub)
    sub:row({ gap = 8 }, function(r)
      r:text("🍅 番茄钟", { font_size = 20, bold = true, color = "$accent" })
      r:badge(state.mode == "work" and "工作中" or "休息中",
              { color = state.mode == "work" and "$accent" or "$success" })
    end)

    sub:text(string.format("%02d:%02d", mins, secs),
             { font_size = 48, bold = true, align = "center" })

    local total = state.mode == "work" and 1500 or 300
    sub:progress_bar(state.remaining / total, { height = 12 })

    sub:row({ gap = 8 }, function(r)
      if state.running then
        if r:button("⏸ 暂停", { bg = "#ff9800" }) then state.running = false end
      elseif state.remaining > 0 then
        if r:button("▶ 继续", { bg = "$success" }) then state.running = true end
      else
        if r:button("🍅 开始工作") then
          state.remaining = 25 * 60
          state.mode = "work"
          state.running = true
        end
      end
    end)
  end)
end
```

### 9.2 📝 笔记节点

```lua
ports = {
  inputs  = { import = { type = "string" } },
  outputs = { saved  = { type = "string" } },
}

state = {
  notes       = {},
  edit_buffer = "",
}

function render(ctx)
  ctx:col({ gap = 6, padding = { 8, 8, 8, 8 } }, function(sub)
    sub:text("📝 笔记", { font_size = 18, bold = true })
    sub:separator()

    local text = sub:input({
      label = "新笔记",
      placeholder = "写点什么...",
      multiline = true, rows = 4,
      value = state.edit_buffer,
    })
    state.edit_buffer = text

    if sub:button("💾 保存", { enabled = text ~= "" }) then
      table.insert(state.notes, {
        text = text,
        time = os.date("%Y-%m-%d %H:%M"),
      })
      emit("saved", text)
      state.edit_buffer = ""
    end

    sub:separator()

    if #state.notes == 0 then
      sub:text("暂无笔记", { color = "$text_secondary" })
    else
      for _, note in ipairs(state.notes) do
        sub:card(note.text, { caption = note.time })
      end
    end
  end)
end

function on_input(name, value)
  if name == "import" then
    table.insert(state.notes, { text = value, time = os.date("%Y-%m-%d %H:%M") })
  end
end
```

### 9.3 ✅ 审批队列

```lua
ports = {
  inputs  = { input = { type = "string", description = "待审批消息" } },
  outputs = {
    approve = { type = "string" },
    reject  = { type = "string" },
  }
}

state = { queue = {} }

function on_input(name, value)
  if name == "input" then
    table.insert(state.queue, value)
  end
end

function render(ctx)
  ctx:col({ gap = 8, padding = { 8, 8, 8, 8 } }, function(sub)
    sub:text(string.format("待处理: %d 条", #state.queue),
             { font_size = 18, bold = true, color = "$accent" })

    if #state.queue > 0 then
      sub:text("最新: " .. state.queue[1], { font_size = 13, color = "$text_secondary" })
      sub:separator()

      sub:row({ gap = 8 }, function(r)
        if r:button("✓ 批准", { bg = "$success" }) then
          emit("approve", table.remove(state.queue, 1))
        end
        if r:button("✓ 全部批准", { bg = "$accent" }) then
          for _, msg in ipairs(state.queue) do emit("approve", msg) end
          state.queue = {}
        end
      end)
      if sub:button("✕ 驳回", { bg = "$danger" }) then
        emit("reject", table.remove(state.queue, 1))
      end
    else
      sub:text("队列为空", { color = "$text_secondary" })
    end
  end)
end
```

### 9.4 📊 简单仪表盘

```lua
ports = {
  inputs = {
    cpu  = { type = "number" },
    mem  = { type = "number" },
    disk = { type = "number" },
  }
}

state = {
  history = { cpu = {}, mem = {} },
}

function on_input(name, value)
  local h = state.history[name]
  if h then
    table.insert(h, value)
    if #h > 60 then table.remove(h, 1) end
  end
end

function render(ctx)
  ctx:col({ gap = 6, padding = { 8, 8, 8, 8 } }, function(sub)
    sub:text("📊 系统仪表盘", { font_size = 18, bold = true })
    sub:separator()

    local cpu = tonumber(ctx.input("cpu")) or 0
    local mem = tonumber(ctx.input("mem")) or 0
    local disk = tonumber(ctx.input("disk")) or 0

    sub:text(string.format("CPU:   %.1f%%", cpu), { font_size = 14 })
    sub:progress_bar(cpu / 100, { height = 8, fill = cpu > 80 and "$danger" or "$accent" })
    sub:text(string.format("内存:  %.1f%%", mem), { font_size = 14 })
    sub:progress_bar(mem / 100, { height = 8 })
    sub:text(string.format("磁盘:  %.1f%%", disk), { font_size = 14 })
    sub:progress_bar(disk / 100, { height = 8 })
  end)
end

-- 注意: ctx.input() 是读取 inputs 原始值的特殊方法
-- 这里仅示意，实际 API 可能不同
```

---

## 10. 边界情况与安全

### 10.1 语法错误处理

```lua
function render(ctx   -- 缺少 )
```

**处理**: `LuaRuntime` 捕获 `mlua::Error::SyntaxError`，将错误信息（行号+描述）显示在节点编辑器的红色错误面板中。画布不崩溃，其他节点不受影响。

### 10.2 运行时错误处理

```lua
function render(ctx)
  some_undefined_function()  -- nil 调用
end
```

**处理**: 用 `pcall` 或 `try` 包裹 `render()` / `on_tick()` / `on_input()` 调用。运行时错误在节点 UI 中显示，不影响帧循环。

### 10.3 无限循环防护

```lua
function render(ctx)
  while true do end
end
```

**处理**: `mlua::Lua::set_hook(InstructionLimit)` 在指令数超限时抛出 `Error::HookError`，节点显示"脚本执行超时"错误。

### 10.4 内存泄漏防护

```lua
function render(ctx)
  state.data = state.data or {}
  table.insert(state.data, string.rep("x", 1000000))
end
```

**处理**: 每节点 8MB 内存限制，超限时 Lua 抛出 `Error::MemoryError`，节点显示"内存超限"。

### 10.5 大 state 性能

- **优化**: 只有 state **被修改过** 才序列化（用 `__newindex` 元方法标记 dirty）
- **限制**: 序列化后 JSON 大小超过 1MB 给出性能告警
- **建议**: 海量数据应使用外部存储

### 10.6 多节点隔离

每个节点拥有独立的 `mlua::Lua` 实例，全局表不共享。

---

## 附录 A: mlua Rust API 速查

```rust
use mlua::{Lua, Result, Value, Table, Function};

// 创建 Lua 状态
let lua = Lua::new();

// 执行 Lua 代码
let result: String = lua.load("return 'hello'").eval()?;

// 设置/获取全局变量
let globals = lua.globals();
globals.set("my_var", 42)?;
let val: i32 = globals.get("my_var")?;

// 调用 Lua 函数
let func: Function = globals.get("render")?;
func.call::<_, ()>(ctx)?;

// 注册 Rust 函数到 Lua
let add = lua.create_function(|_, (a, b): (i32, i32)| Ok(a + b))?;
globals.set("add", add)?;

// 创建和操作表
let t = lua.create_table()?;
t.set("key", "value")?;
let v: String = t.get("key")?;

// 从 Lua 表读取
let state: Table = globals.get("state")?;
let count: i32 = state.get("count")?;

// 指令计数 hook
lua.set_hook(
    mlua::HookTriggers::new().every_n_instructions(1000),
    |_lua, _debug| { /* 检查超限 */ }
)?;

// 内存限制
lua.set_memory_limit(8 * 1024 * 1024)?;

// 安全执行
let result = lua.load(user_script).exec();
match result {
    Ok(_) => {},
    Err(mlua::Error::SyntaxError { msg, .. }) => {
        // 显示语法错误
    }
    Err(mlua::Error::HookError(..)) => {
        // 指令超限
    }
    Err(mlua::Error::MemoryError(..)) => {
        // 内存超限
    }
    Err(e) => {
        // 其他运行时错误
    }
}
```

---

## 附录 B: 内部 Widget 系统（开发者参考，用户不可见）

Lua 的 `ctx:button()` / `ctx:text()` 等底层调用 V1 的渲染函数。

```
ctx:button("Click")  → 内部创建 Widget::Button  →  render_button(...)
ctx:text("Hi")       → 内部创建 Widget::Text    →  render_text(...)
```

**Widget enum 定义保留在 `types.rs` 中，但纯属内部实现细节。** 用户永远看不到它。

---

> **本文档版本**: v0.2（纯 Lua 方案，废弃 JSON）
> **状态**: 设计完成，待实现
