# 架构设计

## 概览

```
keytrace.exe (单文件)
  │
  ├─ WH_KEYBOARD_LL 钩子 → 按键计数
  ├─ WH_MOUSE_LL 钩子 → 鼠标移动+点击
  ├─ SQLite (keytrace_data/) WAL 模式
  ├─ HTTP API (127.0.0.1:auto) + 嵌入前端
  ├─ 系统托盘（右键打开面板/退出）
  └─ 单实例锁（PID 文件）
```

## 数据流

```
钩子线程 → mpsc channel → 处理线程 → 内存缓冲区(60s) → 批量写入 SQLite
                                                     → 会话管理(10s 空闲超时)
```

## 前端嵌入

`rust-embed` 在编译时读 `frontend/dist/` 嵌入 exe。运行时无需外部文件。
开发时用 Vite dev server（`http://127.0.0.1:5520` 代理 API 到 50555）。

## 端口处理

默认 50555，被占用则自动尝试 50556–50565。托盘"打开面板"动态读取实际端口。
