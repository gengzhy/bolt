# Bolt 闪传 - 微软应用商店 (Microsoft Store) 完整发布与维护指南

本文档整理了 **Bolt 闪传** 接入微软合作伙伴中心（Microsoft Partner Center）、构建 MSIX/MSIXBundle 安装包、填写应用商店元数据、上架审核以及日常版本迭代的完整标准化流程。

---

## 目录
- [一、产品身份与官方信息清单](#一产品身份与官方信息清单)
- [二、本地 MSIX 生产环境与一键打包](#二本地-msix-生产环境与一键打包)
- [三、合作伙伴中心建档与配置全流程](#三合作伙伴中心建档与配置全流程)
  - [1. 定价与可用性 (Pricing and availability)](#1-定价与可用性-pricing-and-availability)
  - [2. 上传安装包 (Packages)](#2-上传安装包-packages)
  - [3. 产品属性 (Properties)](#3-产品属性-properties)
  - [4. 年龄分级 (Age ratings)](#4-年龄分级-age-ratings)
  - [5. 商店列表与图文物料 (Store listings)](#5-商店列表与图文物料-store-listings)
  - [6. 审核说明 (Notes for certification)](#6-审核说明-notes-for-certification)
- [四、日常新版本迭代提交流程 (2分钟搞定)](#四日常新版本迭代提交流程-2分钟搞定)
- [五、核心注意事项与避坑总结](#五核心注意事项与避坑总结)

---

## 一、产品身份与官方信息清单

在微软合作伙伴中心注册的 Bolt 闪传官方身份标识如下（用于签名校验与包清单生成，严禁擅自变更字符）：

| 字段名 | 官方配置值 | 说明 |
| :--- | :--- | :--- |
| **应用名称** | `Bolt 闪传` | 商店展示名称与桌面快捷方式名称 |
| **产品 Store ID** | `9PMRQQCLKMBR` | 微软全球唯一商品 ID |
| **商店公开直达链接** | `https://apps.microsoft.com/detail/9PMRQQCLKMBR` | 审核通过后公开可访问 |
| **软件包系列名称 (Package/Identity/Name)** | `IanGeng.Bolt` | MSIX 唯一包名 |
| **发布者标识 (Package/Identity/Publisher)** | `CN=9739D105-BE12-4B41-8961-6E5CBAC36F26` | 微软官方为开发者分配的发布者证书标识（**注意是 `BE12` 与 `6E5C`**） |
| **发布者显示名称 (PublisherDisplayName)** | `Ian Geng` | 商店展示的开发者名称 |
| **程序包系列名称 (PFN)** | `IanGeng.Bolt_anpcg3pxap9k6` | 关联系统包族名的唯一哈希 |
| **隐私政策公开 URL** | `https://gist.github.com/gengzhy/7f51fc7851a3c99f82e839e71caa8ca7` | GitHub Gist 独立公开托管链接 |

---

## 二、本地 MSIX 生产环境与一键打包

### 1. 核心配置文件说明

项目已集成专用于 Tauri 的 Windows Store 现代化打包工具链：
* **`tauri_app/src-tauri/gen/windows/bundle.config.json`**：MSIX 打包配置，严格配置了身份标识、Windows 版本依赖（10.0.17763.0 起）及局域网通信特权。
* **`tauri_app/src-tauri/gen/windows/AppxManifest.xml.template`**：清单模板，已声明 `internetClient`、`privateNetworkClientServer` 和 `runFullTrust` 桌面全信任权限。
* **`scripts/build_windows_dist.ps1`** 与 **`scripts/build_windows_dist.bat`**：全自动构建流水线，支持一键生成并自动平铺导出至 `dist/release/` 目录。

### 2. 本地一键打包命令

执行以下任一命令均可直接生成应用商店安装包：

```powershell
# 方式 A：通过 PowerShell 脚本直接构建（推荐）
powershell.exe -ExecutionPolicy Bypass -File scripts\build_windows_dist.ps1 -Target msix

# 方式 B：双击批处理脚本交互构建
运行 scripts\build_windows_dist.bat -> 输入 5（MSIX 商店包）
```

构建完成后，安装包将扁平化输出到以下路径：
* `dist/release/Bolt 闪传_0.1.0.0.msixbundle`（体积约 2.9 MB，**专门用于上传应用商店**）
* `dist/release/bolt-v0.1.0-windows-amd64.msix`（备用单架构包）

---

## 三、合作伙伴中心建档与配置全流程

登录 [Microsoft 合作伙伴中心 (Partner Center)](https://partner.microsoft.com/zh-cn/dashboard/apps-and-games/overview)，点击进入 `Bolt 闪传` 的当前提交页面，依次完成以下 6 个板块：

### 1. 定价与可用性 (Pricing and availability)

* **市场 (Markets)**：选择“所有可能的市场（选择全部）”。
* **定价 (Pricing)**：选择“免费 (Free)”。
* **免费试用 (Free trial)**：选择“无免费试用”。
* **组织许可 (Organizational licensing)**：勾选允许批量购买许可（默认勾选即可）。
* **公开发布日期 (Publish date)**：选择“认证通过后立即发布”。

### 2. 上传安装包 (Packages)

1. 点击“程序包 (Packages)”页面。
2. 将 `dist/release/Bolt 闪传_0.1.0.0.msixbundle` 拖拽到上传区域。
3. 等待验证完成。验证通过后将显示：
   * **体系结构**：x64
   * **版本**：0.1.0.0
   * **警告说明**：若出现 `runFullTrust` 权限警告（提示使用了 `runFullTrust` 功能），属于 Win32/Tauri 桌面软件的正常提示，不影响提交，在“审核说明”中阐明原因即可。
4. 点击保存。

### 3. 产品属性 (Properties)

* **类别 (Category)**：选择 `实用工具与工具 (Utilities & tools)`。
* **子类别 (Subcategory)**：选择 `文件管理器 (File managers)` 或 `通用工具`。
* **支持信息 (Support info)**：
  * 支持电子邮箱：`genggzy@gmail.com`
  * 支持网页：填写你的支持站点或 GitHub 主页
* **隐私策略 URL (Privacy policy URL)**：必须填写公开可访问链接：
  ```text
  https://gist.github.com/gengzhy/7f51fc7851a3c99f82e839e71caa8ca7
  ```

### 4. 年龄分级 (Age ratings)

进入 IARC 年龄分级问卷系统，所有涉及暴力、成人、血腥、博彩、不良语言的选项均选择 **“否 (No)”**：
* 是否包含暴力内容：否
* 是否允许用户公开不受审查地与其他用户聊天分享敏感内容：否（Bolt 为局域网点对点文件传输工具）
* 提交问卷后，系统将自动生成评级为“适合全年龄（Everyone / 3+）”。

### 5. 商店列表与图文物料 (Store listings)

在 `中文 (简体)` 列表页面填写以下标准化物料：

#### (1) 应用描述 (Description)
```text
Bolt 闪传是一款专为极速互联打造的现代局域网文件传输工具。底层基于高性能 Rust 语言构建传输引擎，深度优化 QUIC 与多通道 TCP 协议栈，致力于解决跨设备、跨平台之间大文件传输繁琐、限速、泄露隐私等痛点。

【核心亮点】
1. 极速局域网传输：充分压榨局域网全双工与 Wi-Fi 6 吞吐带宽，多并发流加持，轻松跑满网络瓶颈。
2. 跨平台互联互通：支持与 Android 手机、macOS、Linux 等多端设备无缝自动发现与互传。
3. 零云端依赖·纯本地安全：所有文件仅在局域网内设备点对点直接传输，绝无云端中转与数据留存，支持设备指纹防伪校验。
4. 批量拖拽与目录传输：支持单文件、多文件以及成千上万个文件的深层文件夹极速递归打包传输。
5. 现代化交互体验：极简现代化界面，清晰的传输进度、实时速度与剩余时间监控。
```

#### (2) 产品功能特性 (Features)
* 纯局域网点对点传输，零云端中转，隐私安全有保障
* 基于 Rust 高性能传输核心，轻松跑满局域网与千兆 Wi-Fi 带宽
* 支持跨平台多端设备自动发现与局域网 IP 直连
* 支持单个大文件、多文件并发传输与整个文件夹递归打包
* 实时传输速率、进度与传输历史日志追踪
* 支持 BLAKE3 算法完整性校验，杜绝传输损坏与篡改

#### (3) 搜索关键字 (Search terms)
* `局域网传输`
* `文件快传`
* `P2P传文件`
* `跨平台文件传输`
* `隔空投送`
* `LocalSend`
* `文件共享`

#### (4) 版权与商标信息 (Copyright and trademark info)
* 版权信息：`© 2026 Ian Geng. All Rights Reserved.`

#### (5) 屏幕截图 (Screenshots)
商店要求桌面端（Desktop）至少上传 1 张、推荐 4 张标准 16:9（1920×1080）截图。

项目中已内置全套生成好的高清截图，位于：
* `docs/screenshots/`
* `dist/store_screenshots/`

**上传顺序与对应说明 (Caption)**：
1. **`screenshot_1_discovery.png`**：局域网设备多端自动发现，零配置一键互联
2. **`screenshot_2_transfer.png`**：千兆网络满速传输，实时多任务性能与速率监控
3. **`screenshot_3_batch_send.png`**：文件与整个文件夹批量拖拽打包极速发送
4. **`screenshot_4_settings.png`**：专业传输网络参数优化与设备防伪安全指纹

> 💡 **重新生成截图方法**：若后续 UI 有微调，在项目根目录运行 `node scripts/generate_store_screenshots.js` 即可全自动渲染更新所有截图。

### 6. 审核说明 (Notes for certification)

在提交前，请在“提交选项 / 审核说明 (Notes for certification)”文本框中填入以下说明，以向微软审核团队解释 `runFullTrust` 特权用途，避免人工审核被驳回：

```text
Bolt 闪传 is an open-source, local-area-network (LAN) peer-to-peer file transfer utility.
It requires the 'runFullTrust' capability because it is built upon native Win32/Rust high-performance network sockets to achieve fast local file I/O and peer-to-peer data streaming (TCP/QUIC) between devices on the same local network.
No cloud services or external servers are involved. All operations are strictly local and initiated by the user.
```

---

## 四、日常新版本迭代提交流程 (2分钟搞定)

初次上架成功后，未来发布新版本（如 v0.1.1、v0.2.0）的流程极其简捷，所有基础信息、截图、定价等**全部自动继承**，仅需执行以下四步：

1. **更新本地版本号并打包**：
   * 修改 `tauri_app/package.json` 中的 `version`（如 `0.2.0`）。
   * 运行打包脚本：
     ```powershell
     powershell.exe -ExecutionPolicy Bypass -File scripts\build_windows_dist.ps1 -Target msix
     ```
2. **在合作伙伴中心开启新提交**：
   * 打开 `Bolt 闪传` 管理页面，点击右上角的 **“更新 (Update)”**，系统会自动复制上一个版本的全部配置为新草稿。
3. **替换新安装包**：
   * 进入“程序包 (Packages)”页面，删除旧的安装包，直接拖入新生成的 `dist/release/Bolt 闪传_0.2.0.0.msixbundle`。
4. **（可选）更新“本次更新说明”并提交**：
   * 在商店列表中简短写一两句新版本特性（如：*“优化局域网传输速率，提升连接稳定性”*）。
   * 点击 **“提交到应用商店 (Submit to Store)”**。完成！

---

## 五、核心注意事项与避坑总结

1. **发布者 CN 务必完全一致**：
   `bundle.config.json` 中的 Publisher 必须是 `CN=9739D105-BE12-4B41-8961-6E5CBAC36F26`，字符严禁错漏（注意不是 BF12 或 6F5C），否则会报错“无效的发布者名称”。
2. **隐私策略链接必须完全公开**：
   禁止使用私有 GitHub 仓库链接（外部访问会返回 404，导致直接被拒）。请始终使用公开的 GitHub Gist 链接：
   `https://gist.github.com/gengzhy/7f51fc7851a3c99f82e839e71caa8ca7`
3. **截图尺寸严格锁定 1920×1080**：
   微软应用商店对屏幕截图的分辨率比例要求极为严苛，任意非标准比例（如手机截图或裁切窗口）都会被校验拦截，使用脚本生成的图片能确保 100% 校验通过。
4. **随时可以取消认证修改**：
   在审核完成前，若发现信息填写有误，可直接在控制台点击“取消认证”，退回草稿修改后重新提交，不会扣除信誉分或产生任何惩罚。
