# Canvas Automation Protocol v1 (draft)

本文档定义 Canvas App Debug API 与 `canvas debug` CLI 的统一协议。

## 1. 目标

统一以下调用面：
- CLI (`canvas debug ...`)
- App 内部调试 HTTP API (`POST /automation`)
- E2E/CI 自动化脚本
- Agent 回放日志

## 2. 传输层

- Base URL: `http://127.0.0.1:4545`
- Endpoint: `POST /automation`
- Content-Type: `application/json`
- 健康检查：`GET /ping`

## 3. 请求模型

```json
{
  "action": "node.create",
  "payload": {"kind":"text", "x":120, "y":80},
  "request_id": "req-001",
  "timestamp_ms": 1761000000000
}
```

字段说明：
- `action`：动作名（见动作列表）
- `payload`：动作参数
- `request_id`：幂等键（可选）。同一 `request_id` 重复提交返回首次响应
- `timestamp_ms`：客户端发送时间（可选）

## 4. 响应模型

```json
{
  "request_id": "req-001",
  "ok": true,
  "data": {},
  "error": null,
  "diagnostics": {
    "action": "node.create",
    "queue_ms": 1,
    "exec_ms": 2,
    "total_ms": 3,
    "state_version": 12,
    "state_timestamp_ms": 1761000000100,
    "affected_ids": [5]
  }
}
```

### 错误模型

`ok=false` 时，`error` 字段返回：
- `code`: 机器可读错误码
- `message`: 人类可读信息
- `details`: 可选扩展上下文

常见错误码：
- `BAD_REQUEST`
- `BAD_PAYLOAD`
- `UNKNOWN_ACTION`
- `NOT_FOUND`
- `BAD_TARGET`
- `TIMEOUT`
- `INTERNAL_CHANNEL_CLOSED`
- `TERMINAL_EXEC_FAILED`

## 5. Graph Snapshot Schema（最小字段）

`graph.get` 返回：
- `version`
- `timestamp_ms`
- `snapshot`
  - `nodes[]`：`id/uid/kind/data/pos/size`
  - `edges[]`：`[from, to]`
  - `edge_routes[]`：`{from,to,route_key}`（可选，存在 route_key 时返回）
  - `viewport`：`pan/zoom`
  - `selection`：`selected/selected_nodes`

稳定性策略：
- 节点按 `id` 排序
- 连线按 `(from,to)` 排序
- 支持 `since_version` 增量读取（相同版本返回空 `changes`）

## 6. 核心动作（v1）

1. `graph.get`
2. `node.create`
3. `node.move`
4. `node.update`
5. `node.delete`
6. `edge.create`
7. `edge.reconnect`
8. `edge.delete`
9. `inject.text`
10. `inject.terminal`
11. `terminal.restart`

### `terminal.restart` 语义

请求 payload：
```json
{"node_id": 2}
```

成功响应 `data`：
```json
{"node_id": 2, "restarted": true, "version": 51}
```

错误示例：
- `NOT_FOUND`：节点不存在
- `BAD_TARGET`：目标节点不是 terminal

## 7. 安全边界

- 调试 API 仅用于本地开发/测试环境
- 终端注入建议在隔离容器或 CI sandbox 中执行
- 生产环境默认不暴露该端口

## 8. CLI 映射

- `canvas debug graph get`
- `canvas debug node create|update|move|delete`
- `canvas debug edge create|reconnect|delete`
- `canvas debug inject text`
- `canvas debug inject terminal`
- `canvas debug terminal restart --node-id <id>`

公共参数：
- `--pretty`：格式化输出
- `--jsonpath <path>`：输出字段过滤
- `--request-id <id>`：幂等请求键
- `--timeout <ms>`：终端注入等待超时
- `--wait`：终端注入等待执行完成

## 9. 可观测性与回放

每个动作输出：
- timing：`queue_ms/exec_ms/total_ms`
- `affected_ids`
- `state_version`

动作日志（jsonl）：
- `artifacts/automation/actions.jsonl`
- 每行包含 `request + response + timestamp_ms`
- 可作为 Agent 回放输入

## 10. 样例（6+）

### A. 读取 graph
```bash
canvas debug graph get --pretty
```

### B. 创建文本节点
```bash
canvas debug node create --kind text --x 120 --y 90 --text "hello"
```

### C. 更新节点文本
```bash
canvas debug node update --id 3 --text "new body"
```

### D. 创建连线
```bash
canvas debug edge create --from 3 --to 4
canvas debug edge create --from 3 --to 4 --route fix
```

### E. 文本注入（append）
```bash
canvas debug inject text --node-id 3 --mode append --text "\nmore"
```

### F. 终端注入并等待
```bash
canvas debug inject terminal --node-id 2 --command "echo smoke" --wait --timeout 5000
```

### G. 重启终端节点（应用 startup_script）
```bash
canvas debug terminal restart --node-id 2
```

### H. 删除节点
```bash
canvas debug node delete --id 3
```
