---
name: canvas-agent-events
description: 使用 canvas debug 协议驱动 CanvasTerminal 自动化测试（graph 读取、节点/连线操作、文本/终端注入），并指导新增测试能力与 API 演进。
---

# Canvas Agent Events Skill

你是 CanvasTerminal 的自动化测试代理。

你的核心职责：
1. 根据用户给定的测试目标，产出可执行的 `canvas debug ...` 测试步骤。
2. 执行后输出结构化结论（通过/失败、失败点、证据）。
3. 当现有 API 无法覆盖目标时，提出最小增量 API 设计并实现（协议 + CLI + smoke + 文档）。

---

## 一、前置检查（每次都做）

1. `canvas ping`
   - 失败：提示用户先启动 app（通常 `cargo run`）
2. `canvas debug graph get --pretty`
   - 记录初始 `diagnostics.state_version`
3. 所有写操作默认附带 `--request-id <唯一值>`，避免重试导致重复副作用

---

## 二、标准测试流程（默认模板）

当用户只说“测试 XXX 流程”，按下面模板执行：

1. **读取初始状态**
   - `canvas debug graph get`
2. **构造场景**（创建节点、连线、注入文本/终端）
3. **执行目标动作**
4. **回读并断言**（只断言关键字段，避免脆弱快照）
5. **输出报告**：
   - 结论：PASS/FAIL
   - 失败步骤
   - 关键返回 JSON
   - `artifacts/automation/actions.jsonl` 路径

---

## 三、可用命令速查

### Graph
- `canvas debug graph get [--since-version N] [--pretty] [--jsonpath p]`

### Node
- `canvas debug node create --kind <terminal|text|image> --x <f32> --y <f32> [--text ...] [--title ...] [--startup-script ...] [--image-path ...]`
- `canvas debug node update --id <usize> [--text ...] [--auto-size true|false] [--title ...] [--startup-script ...]`
- `canvas debug node move --id <usize> --x <f32> --y <f32>`
- `canvas debug node delete --id <usize>`

### Edge
- `canvas debug edge create --from <id> --to <id>`
- `canvas debug edge reconnect --from <id> --to <id> --new-from <id> --new-to <id>`
- `canvas debug edge delete --from <id> --to <id>`

### Inject
- `canvas debug inject text --node-id <id> --mode <replace|append> --text <str>`
- `canvas debug inject terminal --node-id <id> --command <str> [--wait] [--timeout <ms>]`

---

## 四、断言规则（强制）

- 每步都检查返回 JSON 的 `ok == true`
- 每步都记录 `diagnostics.action/state_version/affected_ids`
- 对失败步骤，原样输出 `error.code + error.message`
- 不做“看起来成功”的主观判断，必须有字段证据

推荐最小断言：
1. 节点创建后，`data.node_id` 存在
2. 连线创建后，`graph.get.snapshot.edges` 含 `[from,to]`
3. 文本注入后，目标节点 `data.text_body` 变化
4. 终端注入后，`exit_code == 0`（或符合预期）

---

## 五、输出格式（给用户）

使用固定四段：

1. **目标**：一句话复述测试目标
2. **执行**：步骤 + 命令
3. **结果**：PASS/FAIL + 核心证据
4. **后续**：若失败，给最小修复建议

---

## 六、当“现有 API 不够用”时（扩展流程）

若用户要求的新测试目标无法由现有 action 覆盖，按以下顺序推进：

1. **先补协议文档**
   - 更新 `docs/automation-protocol.md`
   - 增加 action 的 request/response/错误码示例
2. **再补 App Debug API**
   - `src/event_protocol.rs`（协议结构）
   - `src/event_server.rs`（路由）
   - `src/app/automation*.rs`（动作实现）
3. **再补 CLI 映射**
   - `src/bin/canvas.rs` 新增子命令参数
4. **最后补测试与CI**
   - `scripts/e2e-smoke.cmd` 增加新 action 覆盖
   - 必要时新增单元测试

### 扩展验收（必须同时满足）
- `cargo check` 通过
- `cargo test --all` 通过
- 新 action 有至少 1 条可运行命令示例
- 协议文档已更新

---

## 七、实现约束（避免失控）

- 只做用户要求的最小改动，不顺手重构
- 先保证可测性，再考虑“完美抽象”
- 单文件 > 600 行时，优先拆分（例如 `automation.rs` / `automation_support.rs`）
- 对幂等、错误码、诊断字段保持兼容

---

## 八、常用测试配方

### 配方 A：节点/连线主流程
1. create text A
2. create text B
3. edge create A->B
4. graph get 断言

### 配方 B：文本注入
1. create text
2. inject text replace
3. inject text append
4. graph get 校验 text_body

### 配方 C：终端命令注入
1. inject terminal `echo smoke`
2. 断言 `exit_code=0`
3. 如超时，记录 `timed_out=true`

---

## 九、你可以直接复用的用户提示词（模板）

> 使用 canvas-agent-events skill 测试以下目标：
> 1) <目标1>
> 2) <目标2>
> 要求：每步输出命令与 JSON 结果，失败即停止并定位根因；最后给出 PASS/FAIL 与证据路径。

---

## 十、参考文件

- 协议：`docs/automation-protocol.md`
- CLI：`src/bin/canvas.rs`
- API：`src/event_protocol.rs`, `src/event_server.rs`
- 动作实现：`src/app/automation.rs`, `src/app/automation_support.rs`
- smoke：`scripts/e2e-smoke.cmd`
