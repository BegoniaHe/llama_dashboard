# Llama Dashboard — 产品需求文档 (PRD)

> **版本**: v0.1.0-draft
> **日期**: 2026-02-27
> **作者**: BegoniaHe
> **状态**: 草案

---

## 1. 概述

### 1.1 产品定位

Llama Dashboard 是一款基于 Rust 构建的**本地大语言模型管理平台**，通过 FFI 直接链接 llama.cpp 引擎，在单一进程内提供模型管理、多模型并发推理、API 服务、性能测试、模型下载和 Web 管理界面等完整功能。最终产出为**单个可执行文件**（内嵌前端资源），无需任何外部运行时依赖。

### 1.2 目标用户

- 本地部署 GGUF 大语言模型的个人开发者和 AI 爱好者
- 需要同时管理/切换多个模型的用户
- 希望通过 Web UI 或 CLI 快速加载、测试和对话的用户
- 需要兼容 OpenAI / Anthropic / Ollama 等主流 API 协议接入现有应用的用户

### 1.3 核心价值主张

| 维度 | 描述 |
|------|------|
| **单进程多模型** | 通过 FFI 直调 llama.cpp C API，在同一进程内管理多个模型实例，共享 GPU/CPU 资源 |
| **零依赖部署** | 单个静态链接二进制文件，内嵌 Web UI，下载即用 |
| **CLI + Web 双模式** | 命令行快速对话/管理，Web 仪表板长期运行 |
| **多协议兼容** | 同时提供 OpenAI / Anthropic / Ollama / LM Studio 兼容 API |
| **高性能** | Rust 异步运行时 + C 级推理性能，内存占用极低 |

### 1.4 参考项目

| 项目 | 角色 | 参考内容 |
|------|------|---------|
| **llama.cpp** (`reference/llama.cpp`) | 主要参考 | C API 接口设计、推理参数体系、GGUF 格式规范、server 架构（slot 机制、任务队列、流式响应、多模型路由）、API 端点设计 |
| **LlamacppServer** (`reference/LlamacppServer`) | 次要参考 | 模型管理 UX 设计、HuggingFace 搜索下载、Benchmark 系统、Ollama/LMStudio 兼容层、MCP 集成、配置持久化、Web UI 布局 |

---

## 2. 技术架构

### 2.1 技术栈

| 层面 | 选型 | 说明 |
|------|------|------|
| 语言 | Rust (edition 2024) | 主语言 |
| 异步运行时 | Tokio | HTTP/WS 服务、并发任务调度 |
| Web 框架 | Axum + Tower | HTTP/WebSocket/SSE 服务 |
| FFI 绑定 | bindgen + cmake crate | llama.cpp C API 绑定 |
| 前端 | Vue 3 + Vite + TypeScript | Web 管理界面 |
| 前端嵌入 | rust-embed | 将前端构建产物编译进二进制 |
| GGUF 解析 | 自研（纯 Rust） | 模型元数据快速读取 |
| HTTP 客户端 | reqwest | 模型下载、HuggingFace API |
| 存储 | SQLite (rusqlite) | 配置、任务、历史记录持久化 |
| CLI | clap | 命令行参数解析 |
| 日志 | tracing + tracing-subscriber | 结构化日志 + WebSocket 广播 |
| 序列化 | serde + serde_json | JSON 序列化/反序列化 |
| 错误处理 | thiserror + anyhow | 类型安全的错误链 |

### 2.2 系统架构

```
┌──────────────────────────────────────────────────────────┐
│                    CLI 入口 (clap)                        │
│  serve │ run │ models │ download │ bench │ config        │
└────────┬─────────────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────┐
│              Axum HTTP / WebSocket Server                 │
│                                                          │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────────────┐  │
│  │ OpenAI 路由  │ │ Anthropic 路由│ │ 管理 API 路由     │  │
│  └──────┬──────┘ └──────┬───────┘ └────────┬─────────┘  │
│         │               │                  │             │
│  ┌──────┴───────────────┴──────────────────┴─────────┐  │
│  │              业务逻辑层 (Services)                   │  │
│  │  ModelManager · DownloadService · BenchmarkService  │  │
│  │  ConfigService · McpClient · CompatLayer            │  │
│  └──────────────────────┬────────────────────────────┘  │
│                         │                                │
│  ┌──────────────────────┴────────────────────────────┐  │
│  │            llama-core (Safe Rust Wrapper)           │  │
│  │  Model · Context · Sampler · Batch · SlotManager    │  │
│  └──────────────────────┬────────────────────────────┘  │
│                         │ FFI                            │
│  ┌──────────────────────┴────────────────────────────┐  │
│  │          llama-sys (bindgen C bindings)             │  │
│  │  libllama.a + libggml.a (静态链接)                   │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │       嵌入式前端 (Vue 3 SPA — rust-embed)           │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌────────────┐  ┌──────────┐  ┌───────────────────┐   │
│  │ SQLite DB   │  │ 文件系统  │  │ WebSocket 广播    │   │
│  └────────────┘  └──────────┘  └───────────────────┘   │
└──────────────────────────────────────────────────────────┘
```

### 2.3 Workspace 结构

```
llama_dashboard/
├── Cargo.toml                      # [workspace]
├── crates/
│   ├── llama-sys/                  # C FFI bindings
│   │   ├── build.rs               # cmake 编译 llama.cpp + bindgen
│   │   ├── wrapper.h
│   │   └── src/lib.rs
│   ├── llama-core/                 # Safe Rust wrapper
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── backend.rs          # Backend 初始化/清理
│   │       ├── model.rs            # Model 加载/卸载/信息
│   │       ├── context.rs          # 推理上下文
│   │       ├── sampler.rs          # 采样器链
│   │       ├── batch.rs            # Batch 处理
│   │       ├── chat.rs             # 聊天模板
│   │       └── slot.rs             # Slot 管理
│   ├── gguf-parser/                # 纯 Rust GGUF 解析
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── reader.rs           # 快速元数据读取
│   │       ├── full_reader.rs      # 完整 KV 解析
│   │       └── types.rs            # GGUF 类型定义
│   ├── downloader/                 # 模型下载引擎
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── manager.rs          # 下载任务管理器
│   │       ├── task.rs             # 下载任务
│   │       ├── huggingface.rs      # HuggingFace API
│   │       └── resume.rs           # 断点续传
│   └── server/                     # 主程序
│       └── src/
│           ├── main.rs             # CLI 入口
│           ├── cli/                # CLI 子命令
│           ├── routes/             # HTTP 路由
│           ├── services/           # 业务逻辑
│           ├── ws/                 # WebSocket
│           ├── compat/             # 兼容层 (Ollama/LMStudio)
│           ├── state.rs            # 共享状态
│           └── config.rs           # 配置管理
├── frontend/                       # Vue 3 + Vite
├── docs/                           # 文档
└── reference/                      # 参考项目
    ├── LlamacppServer/
    └── llama.cpp/
```

---

## 3. 功能需求

### 3.1 功能优先级定义

- **P0**: 核心功能，MVP 必须实现
- **P1**: 重要功能，首个正式版本实现
- **P2**: 增强功能，后续迭代实现

---

### 3.2 M1 — llama.cpp FFI 引擎层

> **优先级**: P0

#### 3.2.1 Backend 管理

| ID | 需求 | 说明 |
|----|------|------|
| M1-01 | Backend 初始化与清理 | 封装 `llama_backend_init()` / `llama_backend_free()`，进程生命周期内仅调用一次 |
| M1-02 | NUMA 初始化 | 封装 `llama_numa_init()`，支持配置 NUMA 策略 |
| M1-03 | 设备枚举 | 通过 `ggml_backend_dev_count()` / `ggml_backend_dev_get()` 枚举可用 GPU/CPU 设备 |

#### 3.2.2 Model 管理

| ID | 需求 | 说明 |
|----|------|------|
| M1-04 | 模型加载 | 封装 `llama_model_load_from_file()`，支持完整 `llama_model_params` |
| M1-05 | 分卷模型加载 | 封装 `llama_model_load_from_splits()`，支持自定义命名的分卷文件 |
| M1-06 | 模型卸载 | 封装 `llama_model_free()`，安全释放资源 |
| M1-07 | 模型信息查询 | 封装词表信息、模型架构、参数量、训练上下文大小、嵌入维度等查询 API |
| M1-08 | 加载进度回调 | 通过 `progress_callback` 向上层汇报加载百分比 |
| M1-09 | 显存自适应 | 封装 `llama_params_fit()`，根据可用显存自动调整 GPU 层数和上下文大小 |

#### 3.2.3 Context 管理

| ID | 需求 | 说明 |
|----|------|------|
| M1-10 | 上下文创建 | 封装 `llama_init_from_model()`，支持完整 `llama_context_params` |
| M1-11 | 上下文销毁 | 安全释放上下文，保证 model 存活 |
| M1-12 | KV Cache 管理 | KV 缓存清除、序列复制/移除/移位操作 |
| M1-13 | KV Cache 持久化 | Slot 级别的 KV cache 保存/恢复/擦除 |

#### 3.2.4 推理流水线

| ID | 需求 | 说明 |
|----|------|------|
| M1-14 | Tokenization | 封装分词/反分词（线程安全 API） |
| M1-15 | Batch 处理 | 构建和提交 batch 进行 decode |
| M1-16 | 采样器链 | 封装采样器创建和链式组合（temperature, top-k, top-p, min-p, mirostat, penalties, DRY 等） |
| M1-17 | 流式生成 | 逐 token 生成，通过 channel 推送到异步层 |
| M1-18 | 停止条件 | 支持 EOS token、停止词、长度限制三种停止条件 |
| M1-19 | 聊天模板 | 封装 `llama_chat_apply_template()` 和 Jinja 模板渲染 |

#### 3.2.5 安全封装

| ID | 需求 | 说明 |
|----|------|------|
| M1-20 | RAII 封装 | Model、Context、Sampler 的 Rust RAII wrapper（Drop trait 自动释放） |
| M1-21 | Send + Sync | 确保封装类型可安全跨线程使用（通过内部 Mutex 保护非线程安全操作） |
| M1-22 | panic 隔离 | 所有 FFI 调用点使用 `catch_unwind` 防止 panic 穿越 FFI 边界 |
| M1-23 | 全局 Logger | 进程启动时设置一次 `llama_log_set()`，桥接到 Rust tracing 系统 |

---

### 3.3 M2 — 多模型实例管理

> **优先级**: P0

#### 3.3.1 ModelManager

| ID | 需求 | 说明 |
|----|------|------|
| M2-01 | 多模型并存 | 在同一进程内同时加载和管理多个 Model + Context 实例 |
| M2-02 | 模型最大数量限制 | 可配置最大同时加载模型数（默认 4），参考 llama.cpp `--models-max` |
| M2-03 | LRU 自动卸载 | 达到上限时，自动卸载最近最少使用的模型 |
| M2-04 | 模型生命周期状态机 | `Unloaded → Loading → Loaded`，失败回退 `Unloaded`（含 exit_code） |
| M2-05 | 线程安全并发访问 | `Arc<RwLock<HashMap<ModelId, ModelSlot>>>` 管理所有模型实例 |
| M2-06 | 按需加载 | API 请求中指定模型名时，若未加载则自动触发加载流程 |
| M2-07 | 空闲睡眠 | 参考 llama.cpp `--sleep-idle-seconds`，模型闲置指定时间后自动卸载释放资源 |

#### 3.3.2 Slot 系统

| ID | 需求 | 说明 |
|----|------|------|
| M2-08 | 并行 slot | 每个模型支持多个并行推理 slot（共享 KV 缓存），参考 llama.cpp `--parallel` |
| M2-09 | Slot 分配 | 请求到达时分配空闲 slot，无空闲 slot 时进入等待队列 |
| M2-10 | Prompt 缓存复用 | 参考 llama.cpp `--slot-prompt-similarity`，相似 prompt 匹配到已有 slot 复用 KV cache |
| M2-11 | 连续批处理 | 支持 continuous batching，多个 slot 的 token 合并成一个 batch |

#### 3.3.3 任务队列

| ID | 需求 | 说明 |
|----|------|------|
| M2-12 | 请求队列 | 参考 llama.cpp `server_queue`，双队列（主队列 + 延迟队列） |
| M2-13 | 延迟执行 | slot 不可用时将任务放入延迟队列，slot 释放后自动取出执行 |
| M2-14 | 任务取消 | 支持客户端断开时取消进行中的任务 |
| M2-15 | 优先级 | 支持将任务插入队列前端（高优先级） |

---

### 3.4 M3 — GGUF 解析器

> **优先级**: P0

| ID | 需求 | 说明 |
|----|------|------|
| M3-01 | 快速扫描模式 | 仅读取前 128KB，提取 `general.architecture`、`general.name`、`general.file_type`、`{arch}.context_length`、文件大小，用于模型列表展示。响应时间 < 10ms/文件 |
| M3-02 | 完整解析模式 | 读取所有 KV 对（跳过 `tokenizer.ggml.tokens` 等大型数组仅记录长度），用于模型详情页 |
| M3-03 | GGUF v3 兼容 | 支持 GGUF 文件格式 v3（Magic: "GGUF"），完整支持所有 13 种 `gguf_type` |
| M3-04 | 分卷检测 | 自动识别 `*-00001-of-*.gguf` 命名模式，聚合为单一模型条目 |
| M3-05 | 多模态检测 | 自动检测同目录下的 `-mmproj-*.gguf` 文件并关联到主模型 |
| M3-06 | 量化类型映射 | 将 `general.file_type` 整数值映射为人类可读的量化名称（Q4_0, Q5_K_M, IQ4_XS 等） |
| M3-07 | 聊天模板提取 | 提取 `tokenizer.chat_template` 字段内容 |
| M3-08 | 推荐参数提取 | 提取 `general.sampling.*` 系列键值（top_k, top_p, temp 等推荐采样参数） |

---

### 3.5 M4 — API 服务

> **优先级**: P0

#### 3.5.1 OpenAI 兼容 API

| ID | 端点 | 方法 | 说明 |
|----|------|------|------|
| M4-01 | `/v1/models` | GET | 模型列表（含状态：loaded/unloaded） |
| M4-02 | `/v1/completions` | POST | 文本补全（支持流式） |
| M4-03 | `/v1/chat/completions` | POST | 聊天补全（支持流式、工具调用、JSON mode） |
| M4-04 | `/v1/embeddings` | POST | 嵌入向量生成（支持 none/mean/cls/last 池化） |
| M4-05 | `/v1/responses` | POST | OpenAI Responses API 兼容 |

#### 3.5.2 Anthropic 兼容 API

| ID | 端点 | 方法 | 说明 |
|----|------|------|------|
| M4-06 | `/v1/messages` | POST | Anthropic Messages API（支持流式、助手预填充） |
| M4-07 | `/v1/messages/count_tokens` | POST | Token 计数（不执行推理） |

#### 3.5.3 原生 API

| ID | 端点 | 方法 | 说明 |
|----|------|------|------|
| M4-08 | `/completion` | POST | llama.cpp 原生补全格式 |
| M4-09 | `/tokenize` | POST | 文本分词 |
| M4-10 | `/detokenize` | POST | Token 转文本 |
| M4-11 | `/apply-template` | POST | 应用聊天模板（不执行推理） |
| M4-12 | `/embedding` | POST | 原生嵌入向量（支持 `pooling: none` 返回每 token 向量） |
| M4-13 | `/reranking` | POST | 文档重排序 |
| M4-14 | `/infill` | POST | 代码填充 (FIM) |

#### 3.5.4 管理 API

| ID | 端点 | 方法 | 说明 |
|----|------|------|------|
| M4-15 | `/health` | GET | 健康检查（公开，无需认证） |
| M4-16 | `/props` | GET/POST | 服务器属性查询/修改 |
| M4-17 | `/metrics` | GET | Prometheus 兼容指标端点 |
| M4-18 | `/slots` | GET | 当前 slot 状态 |
| M4-19 | `/slots/{id}?action=save\|restore\|erase` | POST | Slot KV cache 持久化操作 |
| M4-20 | `/lora-adapters` | GET/POST | LoRA 适配器管理 |

#### 3.5.5 模型管理 API

| ID | 端点 | 方法 | 说明 |
|----|------|------|------|
| M4-21 | `/api/models` | GET | 所有已发现模型列表（含元数据、状态、收藏标记） |
| M4-22 | `/api/models/{id}/details` | GET | 模型完整 GGUF 元数据 |
| M4-23 | `/api/models/{id}/load` | POST | 加载模型（含启动参数） |
| M4-24 | `/api/models/{id}/unload` | POST | 卸载模型 |
| M4-25 | `/api/models/{id}/config` | GET/PUT | 模型启动参数配置持久化 |
| M4-26 | `/api/models/{id}/alias` | PUT | 设置模型别名 |
| M4-27 | `/api/models/{id}/favorite` | PUT | 切换收藏状态 |
| M4-28 | `/api/models/{id}/template` | GET/PUT/DELETE | 自定义聊天模板管理 |
| M4-29 | `/api/models/{id}/capabilities` | GET/PUT | 模型能力标记（vision, embedding, etc.） |
| M4-30 | `/api/models/scan` | POST | 触发模型目录重新扫描 |
| M4-31 | `/api/models/vram-estimate` | POST | 显存需求估算 |

#### 3.5.6 系统管理 API

| ID | 端点 | 方法 | 说明 |
|----|------|------|------|
| M4-32 | `/api/config` | GET/PUT | 全局配置（模型目录、端口等） |
| M4-33 | `/api/config/model-paths` | GET/POST/DELETE | 模型目录路径管理 |
| M4-34 | `/api/system/devices` | GET | GPU/CPU 设备列表 |
| M4-35 | `/api/system/logs` | GET | 获取最近日志 |
| M4-36 | `/api/system/shutdown` | POST | 优雅关停 |
| M4-37 | `/api/system/fs/browse` | GET | 文件系统目录浏览（安全限制） |

#### 3.5.7 通用要求

| ID | 需求 | 说明 |
|----|------|------|
| M4-38 | 流式响应 (SSE) | 所有推理端点支持 `stream: true`，使用 Server-Sent Events 格式 |
| M4-39 | 请求路由 | 推理请求中的 `model` 字段自动路由到对应模型实例 |
| M4-40 | API Key 认证 | 支持 `--api-key` 配置 Bearer token 认证（管理 API 和推理 API 可分别配置） |
| M4-41 | CORS | 支持跨域配置 |
| M4-42 | 请求超时 | 可配置的读写超时（默认 600s），参考 llama.cpp `--timeout` |

---

### 3.6 M5 — 兼容层

> **优先级**: P1

#### 3.6.1 Ollama 兼容

| ID | 端点 | 说明 |
|----|------|------|
| M5-01 | `/api/tags` | 模型列表 |
| M5-02 | `/api/show` | 模型信息 |
| M5-03 | `/api/chat` | 聊天（流式） |
| M5-04 | `/api/embed` | 嵌入向量 |
| M5-05 | `/api/ps` | 运行中模型状态 |

- 运行在可配置的独立端口（默认 11434）
- 可通过 API 动态启停

#### 3.6.2 LM Studio 兼容

| ID | 端点 | 说明 |
|----|------|------|
| M5-06 | `/api/v0/models` | 模型列表 |
| M5-07 | `/api/v0/chat/completions` | 聊天补全 |
| M5-08 | `/api/v0/completions` | 文本补全 |
| M5-09 | `/api/v0/embeddings` | 嵌入向量 |

- 运行在可配置的独立端口
- 可通过 API 动态启停

---

### 3.7 M6 — 模型下载

> **优先级**: P1

| ID | 需求 | 说明 |
|----|------|------|
| M6-01 | HuggingFace 搜索 | 通过 HF API 搜索 GGUF 模型，支持分页和过滤 |
| M6-02 | hf-mirror 支持 | 自动检测和使用 hf-mirror.com 镜像站 |
| M6-03 | Repo 文件浏览 | 列出指定 Repo 中的所有 GGUF 文件（大小、量化类型） |
| M6-04 | 下载任务管理 | 创建、暂停、恢复、删除下载任务 |
| M6-05 | 并发下载 | 最大 4 个并发下载任务，超额进入等待队列 |
| M6-06 | 断点续传 | 支持 HTTP Range 请求，保存已下载字节数和 ETag |
| M6-07 | 实时进度推送 | 通过 WebSocket 推送下载进度（速度、百分比、剩余时间） |
| M6-08 | 任务持久化 | 下载任务状态存储到 SQLite，服务重启后自动恢复 |
| M6-09 | HF_TOKEN 支持 | 从环境变量或配置读取 HuggingFace 认证 token |
| M6-10 | 下载完成后自动发现 | 下载目标目录在模型路径中时，完成后自动更新模型列表 |

**API 端点**：

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/download/search` | GET | HuggingFace 模型搜索 |
| `/api/download/files` | GET | 指定 Repo 的 GGUF 文件列表 |
| `/api/download/start` | POST | 开始下载任务 |
| `/api/download/pause` | POST | 暂停任务 |
| `/api/download/resume` | POST | 恢复任务 |
| `/api/download/delete` | DELETE | 删除任务 |
| `/api/download/tasks` | GET | 所有任务列表及状态 |

---

### 3.8 M7 — Benchmark 系统

> **优先级**: P1

| ID | 需求 | 说明 |
|----|------|------|
| M7-01 | 推理性能测试 | 测量 prompt 处理速度 (pp) 和文本生成速度 (tg)，单位 tokens/s |
| M7-02 | 多参数配置 | 可配置重复次数、prompt 长度、生成长度、batch size、上下文大小 |
| M7-03 | 结果存储 | 测试结果存储到 SQLite 并关联模型 ID 和参数 |
| M7-04 | 结果对比 | 支持同一模型不同参数或不同模型之间的性能对比 |
| M7-05 | 实时进度 | 通过 WebSocket 推送测试进度 |
| M7-06 | CLI Benchmark | `llama-dashboard bench` 命令直接运行测试并输出结果 |

**API 端点**：

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/bench/run` | POST | 开始 benchmark |
| `/api/bench/results` | GET | 查询历史结果 |
| `/api/bench/results/{id}` | DELETE | 删除结果 |

---

### 3.9 M8 — CLI 工具

> **优先级**: P0

| ID | 命令 | 说明 |
|----|------|------|
| M8-01 | `llama-dashboard serve` | 启动 Web 服务器（默认行为） |
| M8-02 | `llama-dashboard run <model>` | 加载模型并进入交互式对话（无需启动 Web） |
| M8-03 | `llama-dashboard models list` | 列出已发现的所有模型 |
| M8-04 | `llama-dashboard models scan <path>` | 扫描指定目录 |
| M8-05 | `llama-dashboard models info <model>` | 打印 GGUF 完整元数据 |
| M8-06 | `llama-dashboard download search <query>` | 搜索 HuggingFace |
| M8-07 | `llama-dashboard download pull <url>` | 下载模型文件 |
| M8-08 | `llama-dashboard bench <model>` | 运行性能测试 |
| M8-09 | `llama-dashboard config show` | 显示当前配置 |
| M8-10 | `llama-dashboard config set <key> <value>` | 修改配置项 |

#### CLI 通用选项

```
-p, --port <PORT>           HTTP 监听端口 (默认: 8080)
    --host <HOST>           监听地址 (默认: 127.0.0.1)
-c, --ctx-size <N>          上下文大小 (默认: 从模型读取)
-ngl, --n-gpu-layers <N>    GPU 层数 (默认: auto)
-t, --threads <N>           CPU 线程数 (默认: auto)
    --temp <FLOAT>          温度 (默认: 0.8)
    --top-k <N>             Top-K (默认: 40)
    --top-p <FLOAT>         Top-P (默认: 0.95)
    --api-key <KEY>         API 认证密钥
    --models-dir <PATH>     模型目录
    --models-max <N>        最大同时加载模型数 (默认: 4)
    --parallel <N>          每模型 slot 数 (默认: auto)
-v, --verbose               详细日志
```

---

### 3.10 M9 — Web 管理界面

> **优先级**: P1

#### 3.10.1 页面结构

| 页面 | 路由 | 功能 |
|------|------|------|
| 仪表板 | `/` | 系统概览（已加载模型状态、资源使用、快速操作） |
| 模型列表 | `/models` | 所有模型卡片/表格视图，搜索、排序、筛选、收藏、别名 |
| 模型详情 | `/models/:id` | GGUF 完整元数据、启动参数配置、聊天模板编辑 |
| 对话 | `/chat` | 与已加载模型对话（流式输出、Markdown 渲染、代码高亮） |
| 下载 | `/download` | HuggingFace 搜索、下载任务管理、实时进度 |
| Benchmark | `/bench` | 性能测试执行、历史结果对比图表 |
| 设置 | `/settings` | 模型目录、端口配置、兼容层、API Key |
| 日志 | `/logs` | 实时日志流（WebSocket） |

#### 3.10.2 通用要求

| ID | 需求 | 说明 |
|----|------|------|
| M9-01 | 响应式设计 | 桌面端和移动端自适应 |
| M9-02 | 暗/亮主题 | 支持暗色和亮色主题切换 |
| M9-03 | 国际化 | 支持中文和英文，根据浏览器语言自动切换，支持 URL 参数手动指定 |
| M9-04 | 嵌入式分发 | 编译产物通过 `rust-embed` 嵌入二进制，无需独立部署 |
| M9-05 | WebSocket | 实时事件推送（模型状态变化、下载进度、Benchmark 进度、日志流） |

---

### 3.11 M10 — WebSocket 实时通信

> **优先级**: P0

| ID | 通道 | 说明 |
|----|------|------|
| M10-01 | `/ws/events` | 全局事件流（模型状态变化、系统事件） |
| M10-02 | `/ws/logs` | 实时日志广播 |
| M10-03 | `/ws/download` | 下载进度推送 |
| M10-04 | `/ws/bench` | Benchmark 进度推送 |

事件格式统一采用 JSON：

```json
{
  "type": "model.loaded | model.unloaded | download.progress | bench.progress | log",
  "timestamp": "2026-02-27T12:00:00Z",
  "data": { ... }
}
```

---

### 3.12 M11 — 配置持久化

> **优先级**: P0

| ID | 需求 | 说明 |
|----|------|------|
| M11-01 | 全局配置 | 监听端口、模型目录列表、兼容层端口、API Key 等 → SQLite 或 JSON 文件 |
| M11-02 | 模型启动配置 | 每个模型独立的启动参数（ctx_size, n_gpu_layers, temp 等）持久化 |
| M11-03 | 模型元信息 | 收藏状态、别名、能力标记、自定义聊天模板持久化 |
| M11-04 | 下载任务 | 下载任务状态和进度持久化，服务重启后可恢复 |
| M11-05 | Benchmark 结果 | 测试结果持久化，支持查询和对比 |
| M11-06 | 配置文件位置 | 默认 `~/.config/llama-dashboard/`，可通过 `--config-dir` 覆盖 |
| M11-07 | 数据目录 | 默认 `~/.local/share/llama-dashboard/`（下载、缓存等），可通过 `--data-dir` 覆盖 |

---

### 3.13 M12 — MCP 客户端 (P2)

> **优先级**: P2

| ID | 需求 | 说明 |
|----|------|------|
| M12-01 | MCP 服务注册 | 添加/修改/删除 MCP SSE 服务器 |
| M12-02 | 工具发现 | 自动获取已注册 MCP 服务器的可用工具列表 |
| M12-03 | 工具调用 | 在对话中通过函数调用触发 MCP 工具执行 |
| M12-04 | 内置工具 | 提供Web 搜索、当前时间等内置工具 |
| M12-05 | 配置持久化 | MCP 服务器配置持久化 |

---

### 3.14 M13 — 监控与可观测性

> **优先级**: P1

| ID | 需求 | 说明 |
|----|------|------|
| M13-01 | Prometheus 指标 | 兼容 llama.cpp 指标格式：prompt tokens/s、generation tokens/s、KV cache 使用率、请求计数 |
| M13-02 | 模型级指标 | 每个模型独立的推理性能统计 |
| M13-03 | 系统级指标 | 内存使用、GPU 使用率（如可获取）、活跃连接数 |
| M13-04 | 结构化日志 | tracing 结构化日志，支持 JSON 输出 |
| M13-05 | 日志级别动态调整 | 运行时通过 API 调整日志级别 |

---

## 4. 非功能需求

### 4.1 性能

| ID | 指标 | 目标 |
|----|------|------|
| NF-01 | 首 token 延迟 | 与直接调用 llama.cpp CLI 相比，额外延迟 < 5ms |
| NF-02 | 吞吐量 | 单模型多 slot 场景下，吞吐量不低于 llama.cpp 原生 server |
| NF-03 | 内存基础开销 | Rust 服务本身（不含模型）内存占用 < 30MB |
| NF-04 | 启动时间 | 服务启动到可接受请求 < 500ms（不含模型加载） |
| NF-05 | 模型扫描速度 | 1000 个 GGUF 文件的快速扫描 < 3s |

### 4.2 可靠性

| ID | 指标 | 目标 |
|----|------|------|
| NF-06 | 推理隔离 | 单个模型推理失败不影响其他已加载模型和 API 服务 |
| NF-07 | 优雅关停 | SIGTERM/SIGINT 后优雅完成进行中的请求（超时后强制关闭） |
| NF-08 | 配置安全 | 配置文件损坏时使用默认值启动，不崩溃 |

### 4.3 安全性

| ID | 需求 | 说明 |
|----|------|------|
| NF-09 | API 认证 | 支持 Bearer token 认证 |
| NF-10 | 目录浏览限制 | 文件系统浏览 API 有路径安全检查，禁止符号链接穿越 |
| NF-11 | CORS 配置 | 可配置允许的 Origin |

### 4.4 兼容性

| ID | 需求 | 说明 |
|----|------|------|
| NF-12 | 操作系统 | Linux (x86_64, aarch64)、macOS (Apple Silicon)、Windows (x86_64) |
| NF-13 | GPU 后端 | CUDA、Vulkan、Metal、CPU-only |
| NF-14 | GGUF 版本 | 支持 GGUF v3 格式 |
| NF-15 | 量化格式 | 支持 llama.cpp 所有量化类型（F32, F16, BF16, Q8_0, Q4_0 至 IQ1_S 等 30+ 种） |

---

## 5. 里程碑计划

### Phase 1：核心引擎 (MVP)

**目标**：完成 FFI 层 + 单模型推理 + CLI 对话 + 基础 API

| 模块 | 优先级 | 范围 |
|------|--------|------|
| M1 — FFI 引擎层 | P0 | 全部 |
| M3 — GGUF 解析器 | P0 | 快速扫描模式 |
| M8 — CLI | P0 | `run` 和 `config` 命令 |
| M4 — API (部分) | P0 | OpenAI chat/completions + /health |
| M11 — 配置持久化 | P0 | 全局配置 + 模型配置 |

**交付物**：可通过 CLI 加载单个模型进行对话，提供 OpenAI 兼容 API。

### Phase 2：多模型 + Web 服务

**目标**：多模型管理 + 完整 API + Web UI 基础版

| 模块 | 优先级 | 范围 |
|------|--------|------|
| M2 — 多模型管理 | P0 | 全部 |
| M4 — API (完整) | P0 | 全部端点 |
| M3 — GGUF (完整) | P0 | 完整解析 + 分卷 + 多模态 |
| M8 — CLI (完整) | P0 | 全部命令 |
| M10 — WebSocket | P0 | 全部通道 |
| M9 — Web UI | P1 | 模型列表 + 对话 + 设置 |

**交付物**：完整的多模型管理平台，含 Web UI。

### Phase 3：生态完善

**目标**：下载、Benchmark、兼容层、MCP

| 模块 | 优先级 | 范围 |
|------|--------|------|
| M5 — 兼容层 | P1 | Ollama + LM Studio |
| M6 — 模型下载 | P1 | 全部 |
| M7 — Benchmark | P1 | 全部 |
| M13 — 监控 | P1 | Prometheus 指标 |
| M9 — Web UI (完整) | P1 | 下载 + Benchmark + 日志页面 |
| M12 — MCP 客户端 | P2 | 全部 |

**交付物**：功能完整的产品。

---

## 6. 开放问题

| # | 问题 | 状态 |
|---|------|------|
| 1 | 多 GPU 异构场景（如 CUDA + Vulkan 混合）是否需要支持？ | 待定 |
| 2 | 是否需要支持 LoRA 热加载功能？ | 倾向支持（llama.cpp 已有 API） |
| 3 | 是否需要支持推测解码 (Speculative Decoding)？如支持，需要多模型协同 | 待定 |
| 4 | Web UI 是否需要支持对话分支编辑（树状对话历史）？ | 待定 |
| 5 | 是否需要 TLS/HTTPS 支持？还是推荐用户通过反向代理实现？ | 倾向反向代理 |
| 6 | 模型下是否需要支持 ModelScope（国内镜像源）？ | 待定 |
| 7 | 是否需要 Docker 镜像分发？ | 后期考虑 |

---

## 附录 A：llama.cpp C API 关键函数清单

> 以下为 Rust FFI 层需要封装的核心函数。

```
// Backend
llama_backend_init() / llama_backend_free()
llama_numa_init()

// Model
llama_model_load_from_file() / llama_model_load_from_splits()
llama_model_free()
llama_model_n_params() / llama_model_size()
llama_model_desc() / llama_model_meta_*()
llama_params_fit()

// Context
llama_init_from_model() / llama_free()
llama_n_ctx() / llama_n_batch()
llama_attach_threadpool() / llama_detach_threadpool()

// KV Cache
llama_kv_cache_clear() / llama_kv_cache_seq_rm()
llama_kv_cache_seq_cp() / llama_kv_cache_seq_add()

// Decode
llama_batch_init() / llama_batch_free()
llama_decode()
llama_get_logits() / llama_get_logits_ith()
llama_get_embeddings()

// Sampling
llama_sampler_init() / llama_sampler_free()
llama_sampler_chain_add()
llama_sampler_sample()
// (temperature, top_k, top_p, min_p, mirostat, penalties, DRY, XTC 等)

// Tokenization
llama_tokenize() / llama_token_to_piece()
llama_vocab_*()

// Chat
llama_chat_apply_template()

// Misc
llama_log_set()
llama_print_system_info()
llama_perf_context() / llama_perf_context_reset()
```

## 附录 B：Prometheus 指标格式

```
# HELP llamacpp_prompt_tokens_total Number of prompt tokens processed.
# TYPE llamacpp_prompt_tokens_total counter
llamacpp_prompt_tokens_total 0

# HELP llamacpp_prompt_tokens_seconds Average prompt throughput in tokens/s.
# TYPE llamacpp_prompt_tokens_seconds gauge
llamacpp_prompt_tokens_seconds 0.00

# HELP llamacpp_tokens_predicted_total Number of generation tokens processed.
# TYPE llamacpp_tokens_predicted_total counter
llamacpp_tokens_predicted_total 0

# HELP llamacpp_predicted_tokens_seconds Average generation throughput in tokens/s.
# TYPE llamacpp_predicted_tokens_seconds gauge
llamacpp_predicted_tokens_seconds 0.00

# HELP llamacpp_kv_cache_usage_ratio KV-cache usage (1 = 100%).
# TYPE llamacpp_kv_cache_usage_ratio gauge
llamacpp_kv_cache_usage_ratio 0.00

# HELP llamacpp_kv_cache_tokens Total KV-cache token count.
# TYPE llamacpp_kv_cache_tokens gauge
llamacpp_kv_cache_tokens 0

# HELP llamacpp_requests_processing Number of requests processing.
# TYPE llamacpp_requests_processing gauge
llamacpp_requests_processing 0

# HELP llamacpp_requests_deferred Number of requests deferred.
# TYPE llamacpp_requests_deferred gauge
llamacpp_requests_deferred 0
```

## 附录 C：GGUF 文件格式结构

```
+-------------------+
| Magic: "GGUF"     | 4 bytes
+-------------------+
| Version (uint32)  | 4 bytes  (current: 3)
+-------------------+
| Tensor Count (i64)| 8 bytes
+-------------------+
| KV Count (i64)    | 8 bytes
+-------------------+
| KV Pairs...       | variable
|  ├─ Key (string)  |
|  ├─ Type (i32)    |
|  └─ Value         |
+-------------------+
| Tensor Infos...   | variable
|  ├─ Name (string) |
|  ├─ n_dims (u32)  |
|  ├─ Dims (i64[])  |
|  ├─ Type (i32)    |
|  └─ Offset (u64)  |
+-------------------+
| [Alignment Pad]   |
+-------------------+
| Tensor Data Blob  | variable (memory-mappable)
+-------------------+

String := length (u64) + chars (UTF-8, no null terminator)
Array  := elem_type (i32) + count (u64) + elements
Bool   := int8_t
Enum   := int32_t
Alignment := general.alignment key value, or default 32
```
