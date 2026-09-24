# IA'GS 0.1.0 验证记录

日期：2026-09-24。环境：Apple Silicon M4、24 GB 统一内存、macOS。下方保留各轮检查记录；本轮已经生成并校验 ad-hoc 签名的 macOS 安装包，尚未 Apple 公证或公开发布。最新结果见最后一节。

## 自动检查

| 检查 | 结果 |
| --- | --- |
| 前端 Vitest | 23 个文件，104 项通过 |
| Rust 单元测试 | 124 项通过 |
| 原生照片处理集成测试 | 1 项通过，需显式启用 |
| 重建缓存集成测试 | 1 项通过，需完整重建样本并显式启用 |
| TypeScript / Vite 生产构建 | 通过；保留既有 3D 查看器较大分块警告 |
| Rust Clippy（所有 targets，警告视为错误） | 通过 |
| Rust 格式、Git diff 空白检查 | 通过 |
| 引擎完整性、许可证、归属和无遥测检查 | 通过 |

原生集成测试验证透明孔洞、分离部件与遮罩笔刷，确认原始副本字节不变、修改遮罩不改变规范化预览的 RGB。另用真实照片验证自动前景分割；无效背景点选返回错误且不覆盖已保存结果。这里的 RGB 保留指方向、色彩和尺寸规范化后的 RGB，不表示缩放后的像素与相机原始文件逐像素相同。

## 实际 Plus 重建

使用一件杯形文物的 12 张抽样照片，通过真实原生助手、COLMAP 4.0.4 和 Brush 0.3.0 Metal 完成 CLI 全流程。未调用云服务。

- 12 张照片分成 7 个相机组；实际 COLMAP 数据库中为 7 个相机、12 张图像。
- 7/12 张完成注册，注册率 58.3%，2,218 个稀疏点；程序显示质量警告。
- Plus 实际使用 8,000 步、最长边 1,200 px。
- 生成 22,909 个高斯点，PLY 为 5,408,074 字节（约 5.2 MiB）。
- 总耗时约 11 分 56 秒，其中训练约 11 分钟；此时间仅代表该样本，不是其他机器或素材的承诺。
- `final.ply` 与 `results/final.ply` 均生成。

这证明处理链路能够产出模型；低注册率意味着样本的角度覆盖和模型完整性仍有限，不能把此测试当作文物精度认证。

## Pro 与缓存

以实际完成的 Plus 项目创建新的 Pro 项目，确认照片指纹一致时复用特征、匹配与相机重建，并重置 Brush 训练状态。随后在临时副本中修改透明遮罩，再生成项目，确认指纹变化、不复用旧重建。原已完成项目保持不变。

Pro 的 15,000 步参数和缓存路径已检查，**没有额外完成一次完整的 Pro 15,000 步训练**。

## 界面检查

使用 Playwright 在 1440 × 1000 和 1100 × 760 两种桌面尺寸检查开发界面：导入、抠图检查、7 点笔刷、保存、前后切换、接受照片、进入待训练状态。仅显示 Plus / Pro 两档；无浏览器错误。

该浏览器检查使用真实的处理后图片，但模拟 Tauri IPC。原生后端通过上述真实 CLI 测试单独验证。原生开发二进制已启动，尚未完成原生窗口中从文件选择到训练的自动化联调；用户实际试用仍需覆盖这一入口。当时尚未生成应用安装包，后续打包结果见最后一节。

## 重跑

```sh
npm test
npm run build
npm run verify:licenses
npm run verify:engines:macos
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run test:native
# 指向一个已经完成相机重建的 IA'GS 照片项目：
IAGS_TEST_PREPARATION='/path/to/completed-project' cargo test --manifest-path src-tauri/Cargo.toml --test preparation_cache -- --ignored
```

原生图像测试需要正常 macOS 图形服务。某些受限执行沙箱会令系统图像解码返回空像素；助手会明确报错，应在普通本机终端运行。真实照片、PLY、日志和本地测试目录均不纳入源码提交。


## 星空界面增量检查（2026-09-24）

- 更新后前端 23 个测试文件、103 项测试通过；移除过时的视频 Alpha 界面预期，保留预览资源释放、取消、失败恢复等检查，增加侧栏搜索／折叠检查。
- Rust 123 项单元测试通过；新增中文任务名称持久化、文件夹安全名称、空名与超长名拒绝检查。Clippy 和前端生产构建通过。
- Playwright Chromium 检查 1440 × 1000、1100 × 760 及 390 × 844。没有 Browser 插件，使用常规 Playwright。IPC 模拟与真实原生测试的边界同上。
- 实际读取本地测试 PLY 并渲染；检查从模型侧栏导航时的保存询问、取消和继续；不会绕过原有编辑保护。
- 检查默认最近 5 条、更多任务折叠、搜索较早任务、历史路径隐藏及访达命令参数；照片导入、遮罩笔刷、确认照片仍可用。
- 切换配色、语言、上传本地背景后重新加载页面，检查偏好恢复；恢复星空后自定义背景清除。修复旧样式造成的窄屏宽度溢出和模型工具栏颜色对比问题。
- 界面截图与生成的视觉参考检查过布局、标题字体、暗色背景、边框／间距、按钮及路径文案。新增照片复核按钮与设置入口属于实际流程所需，未照搬参考展览的图案或名称。
- 本轮没有再跑完整训练；原生 macOS 窗口中的全流程和 Pro 完整训练仍需实际试用。该轮尚未打包；后续结果见最后一节。


## 最终界面、真实训练进度及安装包（2026-09-24）

- Plus / Pro 按钮移除步数提示；任务名称输入框没有 placeholder；IA’GS 字重改为 700。训练参数保持原值。
- 通过 Brush 私有 stderr 伪终端获取真实迭代数，处理 ANSI 和回车刷新。新增测试覆盖实时接收、子进程退出码、普通及 PTY 模式取消后代进程、心跳／日志保留进度及异常计数过滤。停止使用旧的估算 95% 测试。
- 前端 104 项、Rust 单元 124 项通过；Clippy 所有 targets、格式、许可证、生产构建通过。
- 使用已有本机 COLMAP 样本执行 300 步、256 px 的真实 Brush Metal 短测，收到 48 次计数更新，最终 300/300 并成功导出 PLY。原成果未修改。它验证进度接入，不代表额外完成了完整 Pro 训练。
- Playwright 在 1500×850 检查空白命名框、仅 Plus/Pro 标签、标题计算字重 700，以及训练百分比显示；没有页面异常。这里使用模拟 IPC，真实引擎由上一项单独验证。
- `npm run package:macos` 成功生成 `.app` 和 `.dmg`，最终安装文件为 `releases/IA-GS-0.1.0-macOS-AppleSilicon.dmg`，164,028,114 字节。
- `hdiutil verify` 校验通过；只读挂载检查应用和 Applications 拖放入口，使用包内 CLI 与包内 arm64 引擎目录执行 health，COLMAP、Brush、原生助手均就绪。用两张合成透明 PNG 验证包内照片助手生成两张输出及清单。
- 主应用 `codesign --verify --deep --strict` 通过；107 个内置引擎二进制和动态库分别验证签名通过；上游引擎 SHA256SUMS 一致。LICENSE、NOTICE、第三方声明及引擎清单完整。
- 已停止网页开发服务，通过 LaunchServices 启动最终 `.app`；原生窗口从 `tauri://localhost` 正常加载，检查新建任务、空白命名框和 Plus/Pro，应用保持打开供试用。
- 使用 ad-hoc 签名，没有 Developer ID 证书和 Apple 公证；不声称已具备免 Gatekeeper 提示的公共发行资格。本轮没有在安装包 GUI 中重新跑完整重建；完整 Plus 与用户原生试用记录见前文。

安装包 SHA-256：`9e97dfbd48b391132189014d43da5b24a93fc7b8ec3e4d1d7ccbcdabd962930a`。

真实 Brush 进度重跑：

```sh
IAGS_TEST_BRUSH_DATASET='/path/to/project/work/brush/dataset' cargo test --manifest-path src-tauri/Cargo.toml --test brush_progress -- --ignored --nocapture
```


## GitHub 公开发布检查（2026-09-24）

- 公共源码树经过凭据、个人路径和生成模型检查；不包含照片、模型、日志、原生引擎或安装包。提交作者使用 GitHub noreply 邮箱，保留 33 条上游提交历史。
- 首次 GitHub Actions 在干净的 macOS arm64 环境完成依赖安装、原生助手构建、许可证检查、前端测试／构建、Rust 测试与 Clippy，全部通过。对应运行：https://github.com/by-lastime/IA-GS/actions/runs/35990153682 。
- 核实旧独立引擎地址返回 404 后，源码安装流程改为下载官方 OOOSplat 0.4.1 macOS DMG，核对固定 SHA-256，只读挂载并提取引擎，不安装或运行上游应用。已在独立的干净源码目录执行下载与提取，并通过内置引擎校验。
- 公开发行附件仅为 IA'GS DMG 和 SHA256SUMS。独立引擎压缩包保留本地，不上传 GitHub。
