# GSD-2 核心洞察整理（面向学习）

> 目标：仅整理 `gsd-build/gsd-2` 的核心技术思想与实现方案，便于系统学习。
> 范围：不讨论“我们应该怎么做”，只还原目标项目的方法论与工程结构。
> 
> 主要依据：
> - `docs/user-docs/auto-mode.md`
> - `docs/dev/architecture.md`
> - `docs/dev/ADR-003-pipeline-simplification.md`
> - `docs/dev/ADR-009-orchestration-kernel-refactor.md`
> - `docs/dev/ADR-013-memory-store-consolidation.md`
> - `docs/user-docs/token-optimization.md`
> - `docs/user-docs/configuration.md`
> - `docs/user-docs/skills.md`

---

## 1. 一句话本质

GSD-2 的本质不是“更强提示词”，而是：

- **状态机控制自动化流程**（不是 LLM 自行记忆流程）
- **磁盘状态作为唯一真相**（`.gsd/`）
- **每个工作单元 fresh session**（避免上下文衰减）
- **上下文按层/按预算注入**（不是全量硬塞）
- **执行后机械验证与可恢复闭环**（不是“模型说完成”）

---

## 2. 自动化控制面的核心设计

### 2.1 Auto Mode = 文件驱动状态机

在官方描述中，auto mode 是“**state machine driven by files on disk**”：

1. 读取 `.gsd/STATE.md` 与相关工件
2. 判定下一工作单元（unit）
3. 构建聚焦提示并新建会话执行
4. 执行后落盘，再循环

核心价值：
- 进程崩溃后可恢复
- 允许多终端/远程控制
- 流程推进不依赖单次会话记忆

### 2.2 单元化执行（Unit-based orchestration）

GSD 将工作拆成 unit（如 `plan-*`, `execute-task`, `complete-*` 等），每个 unit 都是可追踪、可重试、可审计的执行点。

在 `architecture.md` 里还明确了调度链：
- 复杂度分类
- 预算压力调整
- 模型动态路由
- prompt 构建与压缩
- 执行后验证与状态持久化

即：**控制平面**（调度/策略/守护）与 **执行平面**（LLM 干活）是分离的。

---

## 3. 上下文工程（Context Engineering）核心方法

### 3.1 Fresh Session Per Unit

GSD 明确坚持“每个 unit 新会话”。
这样避免：
- 长会话垃圾累积
- 任务间污染
- 上下文窗口尾部质量下滑

### 3.2 预注入 + 分层注入

`auto-mode.md` 提到 dispatch prompt 预注入：
- task/slice plan
- prior summaries
- dependency summaries
- roadmap excerpt
- decisions register

`token-optimization.md` 则进一步把注入分为：
- `minimal`
- `standard`
- `full`

不同 profile（budget/balanced/quality）控制注入粒度，核心思想是：
**上下文不是越多越好，而是与任务复杂度和预算动态匹配。**

### 3.3 观测内容裁剪（Observation Masking）

在 v2.59 引入：
- 把较早轮次的 tool result 用占位符替换
- 对超长工具输出截断

目的：
- 减少无效上下文占用
- 不增加额外 LLM 摘要开销（零额外推理成本）

### 3.4 阶段锚点（Phase Handoff Anchors）

在 phase 切换时写入结构化 anchor JSON，后续 phase 注入这些 anchor，减少“跨阶段意图漂移”。

---

## 4. 知识与记忆架构（ADR-013）

### 4.1 问题定义

ADR-013 指出此前存在多知识面并行：
- `decisions` 表
- `KNOWLEDGE.md`
- `memories` 表

它们在 schema、注入路径、工具调用能力上不一致，导致知识碎片化。

### 4.2 决策

ADR-013 的核心决策：
- **`memories` 表作为跨会话持久知识唯一权威源**
- `DECISIONS.md` / `KNOWLEDGE.md` 转为投影（projection）
- 通过分阶段迁移（dual-write -> backfill -> cutover）降低风险

### 4.3 迁移方法学亮点

ADR-013 的工程价值不止“换表”，更重要是迁移策略：
- 明确 cutover 条件
- 双写观察期
- 可回滚路径
- idempotent backfill
- 外部接口（MCP）先对齐再切换

这是典型“高风险系统改造”范式。

---

## 5. 可靠性与可恢复性机制

`auto-mode.md` / `architecture.md` 体现的可靠性策略包括：

1. **Lock + Crash Recovery**：中断后读取幸存状态恢复
2. **Idempotency + completed-key**：避免重复执行漂移
3. **Stuck Detection**：滑动窗口识别循环（不只单点重复）
4. **Artifact Verification Retries**：工件缺失触发有限重试
5. **多级超时监督**：soft / idle / hard timeout
6. **Provider Error 分类恢复**：限流、5xx 自动恢复；鉴权类错误暂停
7. **Forensics**：失败后结构化溯源与诊断

核心思想：
**自动化不是“能跑”，而是“失败可诊断、可恢复、可收敛”。**

---

## 6. 成本-质量协同控制

### 6.1 Token Profile 统一协调

`token_profile` 不是单纯模型切换，而是联动三件事：
- 模型选择
- phase 跳过策略
- context inline 级别

实现了“一个高层开关，驱动全链路行为”。

### 6.2 复杂度路由与预算压力路由

复杂任务上高能力模型，简单任务降级；当预算逼近阈值时自动降档。

这是一个典型的 **质量-成本闭环控制器**：
- 输入：任务复杂度 + 当前预算压力 + 历史路由效果
- 输出：当前 unit 的模型 tier 决策

### 6.3 自适应学习（Routing History）

GSD 记录分配 tier 的成功/失败，后续路由会按模式统计调整（含人工 feedback `/gsd rate`）。

即：路由策略不是静态规则，而是“在线修正”。

---

## 7. 管线简化思想（ADR-003）

ADR-003 是非常关键的洞察：

- 旧管线 session 数过多，存在大量“仪式性单元”（ceremony sessions）
- 多 session handoff 容易信息损失（lossy handoff）
- 重复上下文重注入带来显著 token tax

提出方向：
- 合并 research 与 planning
- 将部分 closeout/validation 机械化
- 将 LLM 用在真正需要 LLM 的环节

这是一种“**把 LLM 从流程胶水角色中解放出来**”的系统优化思路。

---

## 8. UOK（ADR-009）框架价值

ADR-009 把 orchestration 抽象为六大控制平面：
- Plan Plane
- Execution Plane
- Model Plane
- Gate Plane
- GitOps Plane
- Audit Plane

并强调：
- typed contract
- deterministic gate
- per-turn git transaction
- causal audit trace

它的核心不是功能新增，而是把“复杂系统可演进性”作为一等公民。

---

## 9. 可扩展性：Skills / Hooks / MCP

### 9.1 Skills

`skills.md` 表明 GSD 采用开放 Agent Skills 标准，支持全局 + 项目双目录、偏好/规避/规则化路由、健康度监控。

### 9.2 Hook 化

配置层支持 `pre_dispatch_hooks` / `post_unit_hooks`，可对单元执行前后进行策略扩展。

### 9.3 MCP 外部能力接入

`configuration.md` 给出 MCP server 的 stdio/http 接入方式，使能力边界不封闭在内建工具内。

---

## 10. 面向“编程流”与“内容创作流”的通用启发（抽象层）

以下是从 GSD-2 中提炼出的**领域无关**自动化原则（不限定软件开发）：

1. 先定义可执行单元，再让模型执行单元
2. 单元必须有输入契约、输出契约、验收契约
3. 跨单元信息传递优先结构化（summary/anchor/memory），非全文转述
4. 记忆与知识必须有权威源与投影边界
5. 失败处理应是分类策略，不是统一“再试一次”
6. 成本控制必须进入调度层，而不是仅靠人工节流
7. 审计日志要有因果链（traceId/turnId/causedBy）

这些原则同样适用于：
- 代码生产流水线
- 长篇内容创作流水线（章节/卷/角色设定等）

---

## 11. 学习时优先阅读顺序

建议按下面顺序读目标仓库文档：

1. `docs/user-docs/auto-mode.md`（先理解行为）
2. `docs/dev/architecture.md`（再理解模块与控制流）
3. `docs/user-docs/token-optimization.md`（理解成本控制）
4. `docs/dev/ADR-013-memory-store-consolidation.md`（理解记忆边界）
5. `docs/dev/ADR-003-pipeline-simplification.md`（理解演进思路）
6. `docs/dev/ADR-009-orchestration-kernel-refactor.md`（理解未来形态）
7. `docs/user-docs/configuration.md` + `docs/user-docs/skills.md`（理解可配置与扩展机制）

---

## 12. 结语（学习重点）

如果只抓 3 个最核心学习点：

1. **状态机 + 持久状态** 取代 prompt 串联
2. **上下文预算化 + 分层化** 取代全量注入
3. **机械 gate + 可恢复闭环** 取代“模型自觉完成”

这三点基本定义了 GSD-2 的工程竞争力。
