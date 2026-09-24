# 使用与 CLI

## 命令行

```sh
cargo build --manifest-path src-tauri/Cargo.toml --bin iags-cli
./src-tauri/target/debug/iags-cli health
./src-tauri/target/debug/iags-cli prepare '/path/to/photos' --output '/path/to/new-object-project'
# 检查 output 中的遮罩后，接受这批照片：
./src-tauri/target/debug/iags-cli finalize '/path/to/new-object-project' --approve-all
./src-tauri/target/debug/iags-cli generate '/path/to/new-object-project' --quality plus
# 再用 Pro 训练会保留之前的模型，输出到独立项目。
./src-tauri/target/debug/iags-cli generate '/path/to/new-object-project' --quality pro
```

`prepare` 不覆盖已有项目目录。`generate` 只接受已检查的 IA'GS 照片项目，不接受视频。`--engine-dir` 指向直接含 `bin/` 和 `lib/` 的运行时目录，例如 `engines/macos/arm64`。集成测试可使用 `--diagnostics`，避免写入个人项目索引。

## 项目文件

```text
object-project/
├── originals/           原始照片副本
├── output/              处理后的透明 PNG
├── masks/               当前灰度遮罩
├── previews/            方向、色彩、尺寸规范化后的原图
├── base-masks/          初始自动遮罩
├── processing.json     照片对应、相机分组、检查与排除状态
├── .training/           确认后的训练快照
├── source/              本次重建的独立输入副本
├── work/                重建、训练及恢复检查点
├── logs/                本地日志
├── results/final.ply    初始高斯模型
└── final.ply            同一初始模型，供内置查看器使用
```

不要手工改动运行中项目的 `work/` 或训练快照。编辑器的裁切、方向及缩放调整通过导出功能保存；`results/final.ply` 保留最初的训练成果。

## 模式参数

| 模式 | Brush 步数 | 训练最长边 |
| --- | ---: | ---: |
| Plus | 8,000 | 1,200 px |
| Pro | 15,000 | 1,600 px |

两者使用同一套照片处理流程，准备阶段最长边为 2,400 px。只等比缩小、不逐张裁切或缩放前景。处理后 RGB 指规范化预览中的像素，不表示与相机原图逐像素相同；笔刷只更改 alpha。

## 外观与任务

左侧显示最近五条任务，较早任务可展开，名称搜索检索全部历史。设置支持蓝、紫、青、金配色、中英文和界面缩放。自定义背景支持 JPG、PNG、WebP，最大 20 MB、6,000 万像素，保存在本机 WebView 的 IndexedDB。

## 问题反馈

提交 Issue 时请附 macOS 与芯片型号、内存、应用版本、输入格式与数量、失败阶段和脱敏后的关键日志。不要直接上传未获授权的私人照片或包含个人路径的整份日志。请先阅读 [测试记录](VALIDATION.md) 与 [贡献指南](../CONTRIBUTING.md)。
