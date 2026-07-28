# TikClock

[![Build](https://github.com/GDWhisper/opendeck-tikclock/actions/workflows/build.yml/badge.svg)](https://github.com/GDWhisper/opendeck-tikclock/actions/workflows/build.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) | 简体中文

一个面向 [OpenDeck](https://github.com/nekename/OpenDeck) 的 [OpenAction](https://openaction.amankhanna.me/) 插件，把数字时钟铺满你的按键面板——**每个格子显示一位数字**。

将当前时间（HH:MM:SS）拆成一个个数字，随意排布：8 键完整时钟、3 键紧凑布局（两位同格模式），或任何你喜欢的摆法。

![TikClock 实拍 —— 20:41，秒针为两位同格模式](assets/preview.png)

## 功能特性

- **每格一位数字** —— HH:MM:SS 的任意一位都可以分配到任意按键
- **两位同格** —— 紧凑模式，一个格子显示完整的时/分/秒两位数字
- **冒号闪烁** —— 可选，随秒同步闪烁
- **12 / 24 小时制** —— 12 小时制自动去掉前导零，更自然
- **自定义颜色** —— 每个按键可单独设置文字色和背景色
- **按键执行命令** —— 每个数字格都能兼职启动器（Windows 走 `cmd /C`，macOS/Linux 走 `sh -c`）；留空则无动作
- **设置界面双语** —— 英文 / 简体中文，跟随宿主语言自动切换
- **轻量且防御性设计** —— 单个约 1.4 MB 的原生二进制；差分渲染只发送变化的帧（8 键完整时钟约 3 条消息/秒），并内置错峰强制刷新、失效防抖和单周期发送熔断，绝不会冲垮宿主或设备

## 布局示例

| 布局 | 按键数 | 位置分配 |
|---|---|---|
| 完整时钟 | 8 | `时` `时` `:` `分` `分` `:` `秒` `秒` |
| 紧凑 | 3 | `时时` `分分` `秒秒`（两位同格） |
| 极简 | 2 | `时时` `分分` |

## 安装

### 从 OpenAction 插件市场

在 [OpenAction Marketplace](https://marketplace.rivul.us/) 搜索 **TikClock**。

### 手动安装

1. 从[最新 Release](https://github.com/GDWhisper/opendeck-tikclock/releases/latest) 下载 `com.gdwhisper.tikclock.zip`
2. 解压后，把 `com.gdwhisper.tikclock.sdPlugin` 文件夹复制到 OpenDeck 插件目录：

| 平台 | 路径 |
|---|---|
| Windows | `%appdata%\opendeck\plugins\` |
| macOS | `~/Library/Application Support/opendeck/plugins/` |
| Linux | `~/.config/opendeck/plugins/` |
| Flatpak | `~/.var/app/me.amankhanna.opendeck/config/opendeck/plugins/` |

3. 重启 OpenDeck

## 配置

把 **Clock Digit** 动作拖到按键上，然后在属性检查器中配置：

| 设置项 | 说明 |
|---|---|
| 显示位置 | 该按键显示 HH:MM:SS 的哪一部分（单个数字、冒号或两位同格） |
| 24 小时制 | 在 24 小时制和 12 小时制之间切换 |
| 冒号闪烁 | 冒号格每秒闪烁一次 |
| 文字 / 背景颜色 | 按键级配色 |
| 按键命令 | 按下按键时执行的 shell 命令（可选） |

## 从源码构建

```sh
cargo build --release
```

二进制产物放入 `com.gdwhisper.tikclock.sdPlugin/bin/`（Windows 下 `./build.ps1` 一步完成）。开发时可将 `.sdPlugin` 文件夹符号链接到插件目录。

支持的构建目标：`x86_64-pc-windows-msvc`、`x86_64-apple-darwin`、`aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`。

## 许可证

[MIT](LICENSE)
