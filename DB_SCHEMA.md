# 数据库设计

> SQLite · WAL 模式 · 7 天自动清理 · 默认路径 `%LOCALAPPDATA%/KeyTrace/keytrace.db`

## 设计理念

两层存储架构：

| 层 | 表 | 用途 | 粒度 |
|----|----|------|------|
| **原始层** | `keystats` `mouse_moves` `mouse_clicks` `active_sessions` | 热力图（需要坐标点）、按键明细 | 事件级 |
| **归档层** | `hourly_keys` `hourly_mouse_moves` `hourly_mouse_clicks` | 趋势图快速查询 | 小时级聚合 |

趋势图展示 7 天只需要 168 个数据点（24h×7），没必要扫百万条原始记录。Processor 每 60 秒 flush 时同步 upsert 归档表，API 直接读归档表 O(1)。原始层只在热力图请求时访问（有 limit 控制）。

## WAL 模式

`PRAGMA journal_mode=WAL`：一写多读并发。Processor flush 写数据时 API 查询不阻塞。默认 DELETE 模式写操作持有排他锁，所有读卡住。

## 数据清理 & 大小估算

默认保留 7 天。清理线程每小时运行，按日期批量 DELETE。之后 `PRAGMA wal_checkpoint(TRUNCATE)` 回收 WAL 文件，`VACUUM` 回收磁盘。

**8h/天活跃使用、7 天保留的估算**：

| 表 | 日增量 | 周总量 | 大小 |
|----|--------|--------|------|
| `keystats` | ~1,200 行 | ~8,400 行 | ~1 MB |
| `mouse_moves` | ~150K 行 | ~1,000K 行 | **~60 MB** |
| `mouse_clicks` | ~1,000 行 | ~7,000 行 | ~1 MB |
| `active_sessions` | ~20 行 | ~140 行 | <0.1 MB |
| `hourly_*` ×3 | 72 行 | 504 行 | <0.1 MB |
| **索引** | — | — | ~15 MB |
| **合计** | — | — | **~80 MB** |

`mouse_moves` 是绝对大户（50ms 采样 × 位置变化才记录）。极端情况（16h/天）可能到 ~150 MB。7 天清理保证不无限增长。

## 表详解

### keystats — 按键原始统计

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键 |
| date | TEXT | `"2026-07-29"` |
| hour | INTEGER | 0–23 |
| key_code | INTEGER | Windows 虚拟键码 |
| count | INTEGER | 该小时内次数 |

`UNIQUE(date, hour, key_code)`。同键同时段合并，count 累加。批量 upsert：`ON CONFLICT DO UPDATE SET count = count + ?`。

### mouse_moves — 鼠标移动

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键 |
| timestamp | INTEGER | Unix 毫秒 |
| x, y | INTEGER | 屏幕坐标 |
| display_width, display_height | INTEGER | 屏幕分辨率（前端缩放） |
| screen_index | INTEGER | 多屏序号 |

50ms 采样 + 坐标防抖（未变不记录）。API `limit 8000` 控制热力图点数。

### mouse_clicks — 鼠标点击

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键 |
| timestamp | INTEGER | Unix 毫秒 |
| x, y | INTEGER | 坐标 |
| button | TEXT | left/right/middle/x1/x2 |
| display_width, display_height | INTEGER | 屏幕分辨率 |
| screen_index | INTEGER | 多屏序号 |

### active_sessions — 活跃会话

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键 |
| date | TEXT | 日期 |
| start_ms | INTEGER | 开始 Unix 毫秒 |
| end_ms | INTEGER | 结束 Unix 毫秒 |
| key_count | INTEGER | 会话按键数 |

10 秒无操作视为结束。WPM = session_keys / (active_ms / 60000)。

### hourly_* — 小时级归档（3 张表）

`hourly_keys` `hourly_mouse_moves` `hourly_mouse_clicks`，结构相同：

| 字段 | 类型 | 说明 |
|------|------|------|
| date | TEXT | 日期 |
| hour | INTEGER | 0–23 |
| count | INTEGER | 累计数 |

`UNIQUE(date, hour)`。Processor flush 时 upsert。趋势 API 直接扫这 168 行，不碰百万级原始表。

## 索引

| 索引 | 覆盖列 | 用途 |
|------|--------|------|
| `idx_keystats_date` | date | 日期范围查询 |
| `idx_mouse_moves_ts` | timestamp | 时间戳范围查询 |
| `idx_mouse_clicks_ts` | timestamp | 时间戳范围查询 |
| `idx_sessions_date` | date | 会话日期查询 |
