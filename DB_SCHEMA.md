# 数据库表结构

数据库文件：`keytrace_data/keytrace.db`（SQLite，WAL 模式，读写不互斥）

## keystats — 按键统计

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键 |
| date | TEXT | 日期 `2026-07-29` |
| hour | INTEGER | 小时 0–23 |
| key_code | INTEGER | Windows 虚拟键码 |
| count | INTEGER | 按键次数 |

UNIQUE(date, hour, key_code)，批量 upsert。

## mouse_moves — 鼠标移动

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键 |
| timestamp | INTEGER | Unix 毫秒 |
| x, y | INTEGER | 屏幕坐标 |
| display_width, display_height | INTEGER | 屏幕分辨率 |
| screen_index | INTEGER | 屏幕序号 |

50ms 采样间隔，位置变化时才记录。

## mouse_clicks — 鼠标点击

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键 |
| timestamp | INTEGER | Unix 毫秒 |
| x, y | INTEGER | 屏幕坐标 |
| button | TEXT | left / right / middle / x1 / x2 |
| display_width, display_height | INTEGER | 屏幕分辨率 |
| screen_index | INTEGER | 屏幕序号 |

## active_sessions — 活跃会话

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键 |
| date | TEXT | 日期 |
| start_time | INTEGER | Unix 毫秒 |
| end_time | INTEGER | Unix 毫秒 |
| key_count | INTEGER | 会话内按键数 |

## hourly_* — 小时级聚合

`hourly_keys`、`hourly_mouse_moves`、`hourly_mouse_clicks`，按日期+小时聚合，趋势图快速查询。
