# 设计决策

## 1. 单文件 exe：前端嵌入，零依赖分发

`rust-embed` 在编译时将 `frontend/dist/` 的 HTML/CSS/JS 全部嵌入二进制。用户拿到一个 `keytrace.exe` 就能用，不需要安装运行时、不需要附带 `frontend/` 目录。

开发时用 Vite dev server（HMR 秒级热更新），发布时 `npm run build && cargo build --release`。

## 2. 写不阻塞读：WAL + 两层存储

**WAL 模式**：SQLite 默认 DELETE 模式下写持有排他锁，读全部卡住。`PRAGMA journal_mode=WAL` 后一写多读并发——Processor 每 60 秒 flush 几千条鼠标移动时，HTTP API 照常响应。

**两层存储**：趋势图 7 天只需要 168 个点，不该扫百万行原始表。

```
鼠标/按键事件
  ├─→ 原始层 (keystats / mouse_moves / mouse_clicks)
  │     热力图查询：WHERE date + limit
  │
  └─→ 归档层 (hourly_keys / hourly_mouse_moves / hourly_mouse_clicks)
        趋势查询：扫 168 行，O(1)
```

Processor 每次 flush 时同步 upsert 两层，不用异步物化视图、不依赖定时任务。

## 3. 内存缓冲 + 批量写入

非逐事件写库——那样每秒几十次 SQLite 事务会拖垮性能。

```
钩子线程 → mpsc channel → 处理线程
                              ├─ HashMap 合并同键计数
                              ├─ Vec 攒鼠标坐标
                              └─ 每 60 秒 flush 一次
                                    ├─ 批量 insert (1000+ 行/事务)
                                    └─ 批量 upsert 归档
```

`unchecked_transaction()` 关闭事务同步，以 crash 安全换吞吐——单次 flush 可写数千行。

## 4. 端口自动容错

默认 50555。如果被占用（上次未正常退出、端口 TIME_WAIT），循环尝试 50556–50565 直到成功。托盘「打开 Dashboard」用 `get_actual_port()` 动态获取实际端口，不硬编码。

## 5. 退出流程：正确清理低层钩子

Windows `WH_KEYBOARD_LL` / `WH_MOUSE_LL` 钩子在线程 `GetMessageW` 循环中运行。退出时不能直接杀线程——`UnhookWindowsHookEx` 必须在钩子线程内调用。

```
用户点退出
  → RUNNING.store(false)     // 通知钩子线程退出循环
  → PostThreadMessageW(WM_NULL)  // 唤醒 GetMessageW
  → 钩子线程：while 结束 → UnhookWindowsHookEx → 线程自然结束
  → tray: PostQuitMessage → 消息循环退出
```

## 6. 单实例：PID 文件锁 + 僵尸检测

`%TEMP%/keytrace_instance.lock` 写入当前 PID。再次启动时读 PID → `OpenProcess` 检测进程是否存活 → 僵尸锁自动覆盖。`taskkill /F` 强杀也能正确恢复。

## 7. 两个热力图：统一着色算法

键盘热力图和屏幕热力图共享同一套渲染管线：

```
黑/蓝底 Canvas → 描点/描键帽(rgba 白) → blur(半径) → getImageData
→ R 通道归一化 → Jet colormap 映射 → 最终 Canvas
```

键盘热力图是蓝底（`rgb(162,191,244)`）+ 键帽矩形白覆盖；屏幕热力图是黑底 + 散点圆白覆盖。模糊半径和 Jet 色表完全一致，视觉风格统一。

## 8. 鼠标采样：比单纯限频更省资源

不是「每 50ms 记一次」，而是「50ms 检查一次，坐标未变就不写」。配合处理器端 `Vec` 去重——同一秒内同坐标只存一次，实测减少 60%+ 写入量，对数据库大小和 API 响应都有显著改善。
