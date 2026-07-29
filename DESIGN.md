# KeyTrace — Windows 按键统计工具

## 架构

```
keytrace.exe (单文件，纯后台)
     │
     ├─ WH_KEYBOARD_LL 钩子 → 按键统计
     ├─ WH_MOUSE_LL 钩子 → 鼠标移动 + 点击
     ├─ SQLite 持久化 (data/keytrace.db)
     ├─ HTTP API (localhost:50555)
     └─ 系统托盘图标 (退出/暂停)
```

exe 只负责**采集与存储**，前端由其他程序独立绘制，通过 HTTP API 拉取数据。

## 存储：SQLite

### 表结构

```sql
-- 按键统计（按小时聚合）
CREATE TABLE keystats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,           -- '2026-07-17'
    hour INTEGER NOT NULL,        -- 0-23
    key_code INTEGER NOT NULL,    -- Windows 虚拟键码
    count INTEGER NOT NULL DEFAULT 1,
    UNIQUE(date, hour, key_code)
);

-- 鼠标位置采样（仅位置变化时记录）
CREATE TABLE mouse_moves (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,   -- unix 毫秒
    x INTEGER NOT NULL,
    y INTEGER NOT NULL,
    display_width INTEGER NOT NULL,
    display_height INTEGER NOT NULL
);

-- 鼠标点击记录
CREATE TABLE mouse_clicks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    x INTEGER NOT NULL,
    y INTEGER NOT NULL,
    button TEXT NOT NULL,          -- 'left' / 'right' / 'middle'
    display_width INTEGER NOT NULL,
    display_height INTEGER NOT NULL
);

-- 活跃会话（用于手速计算）
CREATE TABLE active_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,
    start_ms INTEGER NOT NULL,    -- unix 毫秒，活跃起始键时间
    end_ms INTEGER NOT NULL,      -- unix 毫秒，最后一个键时间
    key_count INTEGER NOT NULL    -- 该会话的按键数
);
```

### 数据清理
每 7 天窗口删除：删除 7 天前的原始数据（mouse_moves / mouse_clicks / active_sessions / keystats），保留聚合数据。

## 采集逻辑

### 键盘
- 低级键盘钩子 WH_KEYBOARD_LL（不需要管理员权限）
- 每次 KeyDown 计一次
- 按键按虚拟键码分类，按小时聚合写入 keystats

### 活跃会话判定（手速）
```
规则：
1. 上一个会话结束后 → 第一个按下的键 → 标记为"活跃起始键"
2. 之后每按一个键 → 刷新"最后按键时间"
3. 如果 10 秒内无新按键 → 当前会话结束
4. 活跃时长 = 最后按键时间 - 活跃起始时间
5. 会话结束后继续等待下一个"活跃起始键"

手速(键/分) = 会话内总按键数 / (活跃时长(ms) / 60000)
```

### 鼠标
- 低级鼠标钩子 WH_MOUSE_LL
- 移动：仅位置`变化时`记录到 mouse_moves
- 点击：记录到 mouse_clicks（含按钮类型）
- 同时记录当时屏幕分辨率，方便前端还原

## HTTP API（localhost:50555）

| 端点 | 说明 |
|------|------|
| GET /api/info | 屏幕信息 / 版本 / 运行时长 |
| GET /api/stats/today | 今日按键总数 / 按键分布 / 手速 / 活跃时长 |
| GET /api/stats/range?from=&to= | 指定日期范围统计 |
| GET /api/stats/keys?date= | 指定日期的各键码分布 |
| GET /api/mouse/moves?from=&to=&limit= | 鼠标轨迹（分页） |
| GET /api/mouse/clicks?from=&to=&limit= | 点击记录（分页） |
| GET /api/sessions?date= | 指定日期的活跃会话列表 |

## 配置

配置文件 `config.json` 位于 exe 同级目录：

```json
{
    "port": 50555,
    "db_path": "data/keytrace.db",
    "idle_timeout_ms": 10000,
    "retention_days": 7
}
```

## 目录结构

```
E:\keytrack\
├── src/
│   ├── main.rs           # 入口：初始化钩子 + HTTP + 托盘
│   ├── hooks.rs          # 键盘/鼠标钩子
│   ├── db.rs             # SQLite 操作
│   ├── api.rs            # HTTP API 路由
│   └── session.rs        # 活跃会话管理
├── static/               # （预留，未来前端可放这里）
├── Cargo.toml
└── DESIGN.md
```
