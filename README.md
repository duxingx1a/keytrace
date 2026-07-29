# KeyTrace

记录你的键盘按键和鼠标活动，生成热力图和趋势报告。Windows 后台运行，双击即用。

<p align="center">
  <img src="screenshot.png" width="640" alt="KeyTrace 截图">
</p>

## 是什么

KeyTrace 是一个 Windows 键盘/鼠标统计工具。通过低级钩子采集输入数据，存到本地 SQLite，浏览器展示热力图和 KPI 趋势。全部数据存 `%LOCALAPPDATA%`，纯本地，不上传。

## 功能

- 键盘热力图 — 每个键的频率可视化，hover 弹起 + 标签
- 屏幕热力图 — 鼠标移动轨迹和点击分布
- KPI 面板 — 累计按键、手速 (WPM)、鼠标数据
- 趋势图 — 四标签切换，小时/日/月粒度自适应
- 系统托盘 — 右键菜单、开机自启动、退出清理钩子
- 单实例 — 不重复启动，端口自动容错 50555→50565

## 使用

1. 下载 [Release](https://github.com/duxingx1a/keytrace/releases) 的 zip，解压得 `keytrace.exe`
2. 双击运行，右下角弹出启动通知
3. 右键托盘 → 打开 Dashboard
4. KPI 每 3 秒刷新，趋势每 15 秒

## 开发

```bash
cd frontend && npm install && npm run dev   # 前端热更新
cargo build                                  # debug
cd frontend && npm run build && cd .. && cargo build --release  # 发布
```

## 技术

| 层 | 依赖 |
|----|------|
| 后端 | Rust, tiny_http, rusqlite, rust-embed |
| 前端 | React, Vite, Tailwind CSS, Motion |
| 钩子 | WH_KEYBOARD_LL / WH_MOUSE_LL |
| 图表 | Canvas Jet colormap 自绘 |
