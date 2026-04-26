# Total TODO（按优先级重排）

> 重排原则：先解决“可持续迭代能力（编程循环+自动化测试）”与“高频阻塞/数据风险”问题，再做核心体验，再做高级能力，最后做增强与美化。

## P0（必须先做：建立可持续开发闭环）

- [x] 当前项目急需解决自动化测试流程。现在每次都需要人工手测，效率很低。
  - 思路：新增一个 agent 用的 CLI（例如 `debug_canvas_cli`），既能读取调试信息，也能执行所有人类可做操作（并保持可扩展，尽量底层能力化）。
- [x] 跑通“编程循环”主流程（这是当前最重要事项）
  1. 用户输入需求
  2. 用户与 AI 'CEO' 讨论需求详情，判断需求是否合理（一个 Terminal+Pi 可实现），确认后拆解任务到 `artifacts/{feature}_Tasks.md`
  3. 'Planner' 从 `artifacts/Tasks.md` 选择最高优先级任务，产出 `artifacts/{feature}_TODO.md`（技术方案）与 `artifacts/{feature}_Tests.md`（AI 可验收方案）；若任务全完成，执行 git add/commit 并打 `feature/fixed` 等标签
  4. 'Executer' 按 `artifacts/{feature}_TODO.md` 编码并保证编译通过
  5. 'Tester' 按 `artifacts/{feature}_Tests.md` 测试；失败则反馈 'Executer' 修复；成功则回写 `artifacts/{feature}_Tasks.md` 并返回 'Planner' 分配下一任务
- [x] 人工校验分流方案已定稿（MVP）
  - 方案文档：`docs/decision-node-mvp.md`
  - 范围：新增决策节点（按钮配置 + 事件分流 + 消息展示），先不做权限/超时等高级能力

- git worktree 工作流
    - 收到 feature/bug（明确验收标准）
    - 创建 worktree（新分支 + 新目录）
    - 在该 worktree（对应 cwd）启动 Agent 开发
          - 简单：Worker ->（迭代 commit）-> Tester -> Human Approval
                  - 通过 / bug修复回 Worker / 需重计划则升级 Planner / 失败丢弃
          - 复杂：Planner -> Worker ->（迭代 commit）-> Tester -> Human Approval
                  - 通过 / bug修复回 Worker / 重新计划回 Planner / 失败丢弃
    - 集成校验（同步主分支 + 关键测试）
    - 回主仓库走 PR 合并
    - 删除 worktree + 本地/远端分支清理
 ### 已 支 持 （ 可 以 直 接 画 出 来 并 跑 ）                                                                                                                                                                                 - Terminal / Decision 节 点 与 连 线 （ src/model.rs）                                                         - 连 线  route_key 分 流 （ fix/next/approve/reject）                                                         - 终 端 通 过  canvas done --route ... 触 发 路 由 （ src/bin/canvas.rs）                                         - 你 仓 库 里 已 有 类 似 样 例 ： flow/program_graph.json（ Planner/Executer/Tester/Decision + route）                                                                                                                      ### 不 足 （ 你 描 述 流 程 中 的 关 键 缺 口 ）                                                                                                                                                                               1. worktree 自 动 创 建 /回 收 （ 新 分 支 +新 目 录 、 结 束 后 清 理 ）                                                       - 目 前 无 专 门 命 令 /节 点 能 力 ， 一 般 要 在 终 端 里 手 写  git worktree ...                                      2. 每 个 节 点 独 立  cwd/worktree 绑 定                                                                            - 终 端 工 作 目 录 当 前 取 应 用 进 程  current_dir（ src/app/terminal.rs） ， 不 是 节 点 级 可 配 置                    3. PR 合 并 链 路 自 动 化                                                                                         - 没 有 内 建  “提  PR / 合 并  / 删 远 端 分 支 ” 的 平 台 集 成 功 能                                                4. 集 成 校 验 阶 段 模 板 化                                                                                        - 可 在  startup_script 里 写 ， 但 不 是 内 建  stage
---

## P1（高优先级：高频 bug / 基础交互 / 稳定性）

- [x] 当前程序在用户输入中文时：输入法组合态下按退格，本应只退输入法字母，但实际同时退格了编辑文本（严重输入 bug）
- [x] 文本节点编辑时，应允许用户按住左键选择文本，而不是拖拽节点（编辑态交互冲突）
- [x] 默认空 Graph 时，`Ctrl+S` 应触发“另存为”（而不是默认保存 `./graph.json`）；`Ctrl+N` 新建空 Graph；新空 Graph 同样 `Ctrl+S` 走另存为（基础文档软件逻辑）
- [x] 性能优化：多开终端 + 每个终端打开 Pi 后，CPU 很快飙升到 100%，需分析瓶颈并提供给 AI/人工定位能力（已补齐 UI 性能浮窗 + `/automation/metrics` + `canvas debug metrics` 诊断闭环）
- [x] Graph 需要名称且可修改（基础资产管理能力）

---

## P2（核心生产力功能：节点组织、复用与可视化执行）

- [x] 节点复制功能（高频效率功能）
  - [x] 选中单个或多个节点 `Ctrl+C`
  - [x] `Ctrl+V` 在鼠标位置粘贴多节点
  - [x] 若多节点之间有连线，自动复制其连线信息
- [ ] 新增节点打组功能：用户 `Ctrl+G` 打组选中节点，组有组名显示；组位置基于组内节点位置，移动组即移动组内全部节点
  - [ ] 按住 `Alt` 点击可跳转到组内（若点击区域被组包含）；按住 `Alt` 时需要可视化提示用户“可跳转”
- [ ] 执行流程高亮：脚本执行时高亮当前节点，让用户一眼看到执行进度
- [ ] 节点间事件可视化：展示事件顺序与事件详情，便于理解当前进展
- [ ] 终端节点中，按住鼠标左键向上拖拽应可滚动终端以复制更多历史信息（当前缺失）
- [ ] 决策节点中, 需要类似文本/终端节点支持文字缩放, 目前文字大小不变. 也要支持基于距离隐藏内部信息
- [ ] 决策节点中, 节点内需要一个背景色, 当前背景穿透的, 完全可以看到后面

---

## P3（工作流能力扩展：面向复杂编排）

- [ ] 并行分流节点：单节点基于连线分发到所有下游节点（并行工作流）
- [ ] 判断分流节点：单节点基于条件/选择发送给单个下游节点（分支工作流）
- [ ] 子图功能（需专项讨论）：子图可视为另一个 `graph.json`，并支持嵌套
  - [ ] 支持无限嵌套
  - [ ] 支持逐层进入子图（跳转进入）

---

## P4（中长期探索 / 业务场景化）

- [ ] 多终端节点 Agent 合作
  - [ ] deep-interview：澄清需求、确定边界/非目标/技术约束
  - [ ] ralplan：审批计划、方案权衡、实施路径，把需求转成可执行 task 列表
    - [ ] team：任务可并行时使用，并行开发多个链路（每链路调用 ralph 串行流程）
    - [ ] ralph：任务需串行时使用，进行持久处理任务
      - [ ] 独立 worktree，可持久化，可 resume
- [ ] 小说工作流（参考 `./flow/novel_graph.json`）
  - [ ] 从“借鉴小说名称与简介”到“生成完整章节”的全流程

---

## P5（易用性与视觉优化）

- [ ] 文本节点创建后的默认高度/宽度需要调整（先给代码位置，手动调参）
- [ ] 右上角三个按钮缺少交互反馈（hover/pressed 等多态表现），需优化（如颜色变化）

---

## 已完成

- [x] 连线默认显示为贝塞尔曲线；并优化起终点视觉方向感
- [x] 终端节点支持 `Ctrl+V` 粘贴文本
- [x] `src/app/ui/canvas.rs` 已明显超过 600 行需要拆分（已处理）
