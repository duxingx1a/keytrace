# KeyTrace 数据库表结构说明

数据库文件：`E:\keytrack\data\keytrace.db`（SQLite）

---

## 1. keystats — 按键统计

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键，自增 |
| date | TEXT | 日期，格式 `2026-07-17` |
| hour | INTEGER | 小时，0-23 |
| key_code | INTEGER | Windows 虚拟键码（如 65=A, 13=Enter） |
| count | INTEGER | 该键在该小时内的按键次数 |

**UNIQUE(date, hour, key_code)** — 同一天同一小时同一键合并

**用途：**
- 查询今日/某天总按键数：`SELECT SUM(count) FROM keystats WHERE date='2026-07-17'`
- 按小时分布：`SELECT hour, SUM(count) FROM keystats WHERE date='2026-07-17' GROUP BY hour`
- 哪些键按得最多：`SELECT key_code, SUM(count) FROM keystats GROUP BY key_code ORDER BY SUM(count) DESC`

---

## 2. mouse_moves — 鼠标移动轨迹

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键，自增 |
| timestamp | INTEGER | Unix 毫秒时间戳 |
| x | INTEGER | 鼠标 X 坐标（屏幕像素） |
| y | INTEGER | 鼠标 Y 坐标（屏幕像素） |
| display_width | INTEGER | 录制时的屏幕宽度（像素） |
| display_height | INTEGER | 录制时的屏幕高度（像素） |

**用途：**
- 绘制鼠标热力图：`SELECT x, y, COUNT(*) FROM mouse_moves GROUP BY x/50, y/50`
- 每 50ms 采样一次（钩子端）→ 处理端再降为 100ms 写入间隔
- 记录屏幕分辨率是为了前端渲染时坐标缩放适配

---

## 3. mouse_clicks — 鼠标点击

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键，自增 |
| timestamp | INTEGER | Unix 毫秒时间戳 |
| x | INTEGER | 点击 X 坐标 |
| y | INTEGER | 点击 Y 坐标 |
| button | TEXT | 按键：`left` / `right` / `middle` |
| display_width | INTEGER | 录制时的屏幕宽度 |
| display_height | INTEGER | 录制时的屏幕高度 |

**用途：**
- 统计左右键点击比例
- 点击位置热力图
- 每小时点击频次分析

---

## 4. active_sessions — 活跃会话（手速计算用）

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER | 主键，自增 |
| date | TEXT | 日期 |
| start_ms | INTEGER | 该会话第一个按键的时间戳（Unix 毫秒） |
| end_ms | INTEGER | 该会话最后一个按键的时间戳（Unix 毫秒） |
| key_count | INTEGER | 该会话内的总按键数 |

**活跃会话判定规则：**
```
1. 上一个会话结束后 → 第一个按下的键 → 标记为"活跃起始键"
2. 之后每按一个键 → 刷新"最后按键时间"
3. 如果 10 秒内无新按键 → 当前会话结束
4. 活跃时长 = 最后按键时间 - 活跃起始时间
5. 手速(键/分) = 会话内总按键数 / (活跃时长(ms) / 60000)
```

**用途：**
- 计算真实手速（排除挂机/看文档时间）
- 每日活跃总时长：`SELECT SUM(end_ms - start_ms) FROM active_sessions WHERE date='2026-07-17'`

---

## 数据清理策略

每 7 天窗口删除：删除 7 天前的原始数据（所有表），由 config.json 的 `retention_days` 控制。
