# IA'GS 修改说明

本仓库基于 OOOSplat 0.4.1，基准提交 `037005dcd5a03b1579d0b363f8046785325f95f5`。
上游：https://github.com/ooolabdev/ooosplat

以下文件与模块已为 IA'GS 0.1.0 修改或新增（2026-09-24）。保留上游 Git 历史、Apache-2.0 LICENSE、NOTICE、商标政策与第三方声明。IA'GS 是独立衍生项目，不代表上游官方版本；COLMAP、Brush 与高斯泼溅算法仍归各自项目。

## 新增照片准备与检查

- `native/`：Swift / ImageIO 图像规范化、原始副本、Apple Vision 前景分割、遮罩点选和笔刷修改；Objective-C 提供窄接口桥接。
- `src-tauri/src/photos.rs`：照片项目验证、处理进度、取消、资产作用域、检查确认、训练快照及指纹。
- `src/components/PhotoPreparation.*`：照片导入与遮罩检查界面。
- `script/`、`scripts/sdk-overlay.mjs`：本地助手构建、开发启动和可选原生测试；只读 SDK 别名处理开发机器的头文件异常。
- `src-tauri/tests/`：原生遮罩与实际重建缓存集成测试。

## 修改上游流程

- `src-tauri/src/engines/colmap.rs`、`video/image_sequence.rs`：按 EXIF 与尺寸分组内参、允许不同尺寸、传递相机分组；取代全数据集强制共用一个相机。
- `src-tauri/src/project/manager.rs`、`pipeline/runner.rs`：已审查照片输入、独立快照、按指纹复用重建、Plus / Pro 模式、训练阶段真实步数百分比、`results/final.ply` 副本。
- `src-tauri/src/process/mod.rs`、`engines/brush.rs`：以独立伪终端读取 Brush 实时计数，保留进程组取消、退出码和原始日志；心跳保留已测进度。
- `src-tauri/src/bin/splatstudio.rs`：保留源文件位置，输出 CLI 改名 `iags-cli`；增加 prepare / finalize，限制照片与两档模式。
- `src-tauri/src/commands/`、`lib.rs`、`main.rs`、`error.rs`：接入新命令、状态与独立程序身份。
- `src/app/`、`src/i18n/`、`src/stores/`、`src/styles.css`：IA'GS 文案、照片专用入口、两档模式、处理状态及相应测试；既有查看器的必要品牌字符串同步修改。
- `src-tauri/src/project/catalog.rs`、`metadata.rs`：独立数据目录与模式命名。个别 Rust 文件包含格式整理。

## 平台、身份与分发

- package / Cargo 清单、锁文件、Tauri 配置、HTML、图标、构建脚本改为 IA'GS，版本 0.1.0，标识 `app.iags.desktop`。
- 面向 macOS 15+ Apple Silicon；删除 Windows / Linux 构建入口和相应工作流，保留源代码检查 CI。
- 遥测端点清空、HTTP 客户端依赖移除；照片处理和训练留在本机。
- 引擎下载与校验保留上游来源和许可证；照片流程不要求 FFmpeg。历史视频模块仍可能存在，但新的输入边界不允许视频。
- README、CONTRIBUTING、ROADMAP、NOTICE、许可证检查、忽略规则及本文档更新；个人照片、模型、测试数据和引擎二进制不纳入源码。

星空主题、任务侧栏、命名搜索、多配色、中英文和本地背景设置已经实现。macOS `.app` 与 `.dmg` 已生成并验证，使用 ad-hoc 签名；Developer ID 签名与 Apple 公证尚未实施；源码及测试安装包发布于 by-lastime/IA-GS。当前改变是工作流与功能扩展，不是重新发明底层重建算法。

## 开源发布整理

- README 中英文首页、真实界面预览、使用说明、贡献指南、路线图与引擎说明面向公共仓库整理。
- 移除当前树内未使用的上游推广截图与品牌图案，保留原始 Git 历史和许可证归属。
- 增加本机代理配置、凭据和高斯模型的忽略规则；不提交个人数据、测试产物或引擎二进制。
- 修复上游不存在的独立引擎下载地址：源码安装脚本直接校验官方 OOOSplat 0.4.1 DMG，仅以只读方式提取其中引擎并再次核验 SHA256SUMS；不安装或启动 OOOSplat，也不在 IA'GS 单独发布引擎包。
