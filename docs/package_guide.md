# Bolt 项目 GitHub Release 最终定稿打包命名规范（全平台｜GUI/CLI 分离）

## 一、全局强制规则（最终定稿）

1. 全程小写，仅使用短横线 **-** 分隔，禁止大写、下划线、点号、空格、中文。
   *(注：文件名主体与扩展名全小写；Linux 社区标准扩展名 `.AppImage` 为唯一允许包含大写字母的特例。)*

2. Git Tag 统一使用 `vX.Y.Z`，包名版本与 Tag 严格一致。模板中 `{version}` 统一定义为不带 `v` 的纯版本号（如 `0.1.0`）；脚本若从 Git Tag 提取需去除前导 `v` 后代入，避免双 `v`。

3. 图形界面客户端（GUI）为默认主产物，**不携带 gui 标识**。

4. 命令行客户端（CLI）必须显式携带 **cli** 字段区分，避免与主程序混淆。

5. macOS 系统标识固定为 **macos**，废弃 darwin / osx。

## 二、统一命名模板

**GUI 主程序模板**

```Plain Text
bolt-v{version}-{os}-{arch}[-variant].{ext}
```

**CLI 命令行模板**

```Plain Text
bolt-cli-v{version}-{os}-{arch}[-variant].{ext}
```

> **压缩包解压内物规范**：
> 所有打包为 `.zip` 或 `.tar.gz` 的压缩包，内部解压出的可执行二进制文件必须保持纯净命名，统一为 `bolt-cli`（Windows 下为 `bolt-cli.exe`，macOS 免安装解压出 `Bolt.app`），不携带任何版本号与架构后缀，方便用户解压后直接配置进 PATH 或即开即用。

> **产物平铺归档规范**：
> 因所有最终打包产物的文件名均已严格包含产品标识、版本、系统、架构与格式变体等全要素信息，具备全局唯一性与自解释性，因此**去除所有产物的平台及类型子目录**，全平台所有打包文件全部直接平铺落到 `dist/release/`（正式发布包）或 `dist/debug/`（调试测试包）目录下。

## 三、系统、架构、变体标准字典（固定不可改）

### 1. 系统 OS

- Windows → **windows**
- macOS → **macos**
- Linux → **linux**
- Android → **android**
- iOS → **ios**

### 2. 架构 ARCH

- x86_64 / Intel 64位 → **amd64**
- ARM64 / M芯片 / 移动端 → **arm64**
- x86 32位 → **386**
- macOS / Android 通用包 → **universal**

### 3. 可选变体 VARIANT

- **portable**：绿色免安装版（Windows / macOS）
- **setup**：Windows EXE 安装程序
- **musl**：Linux 静态无依赖编译

### 4. 各平台后缀对照表

|平台|GUI 后缀|CLI 后缀|
|---|---|---|
|Windows|zip、exe、msi|zip|
|macOS|dmg、zip|tar.gz|
|Linux|AppImage、deb、rpm|tar.gz|
|Android|apk|无|
|iOS|ipa|无|

## 四、全平台标准成品包名（直接用于 CI）

### Windows

**GUI**
- bolt-v0.1.0-windows-amd64-portable.zip
- bolt-v0.1.0-windows-amd64-setup.exe
- bolt-v0.1.0-windows-amd64.msi
- bolt-v0.1.0-windows-amd64.msix
- bolt-v0.1.0-windows-386-portable.zip

**CLI**
- bolt-cli-v0.1.0-windows-amd64.zip
- bolt-cli-v0.1.0-windows-386.zip

### macOS

**GUI**
- bolt-v0.1.0-macos-universal.dmg
- bolt-v0.1.0-macos-universal-portable.zip

**CLI**
- bolt-cli-v0.1.0-macos-amd64.tar.gz
- bolt-cli-v0.1.0-macos-arm64.tar.gz

### Linux

**GUI**
- bolt-v0.1.0-linux-amd64.AppImage
- bolt-v0.1.0-linux-amd64.deb
- bolt-v0.1.0-linux-amd64.rpm

**CLI**
- bolt-cli-v0.1.0-linux-amd64.tar.gz
- bolt-cli-v0.1.0-linux-amd64-musl.tar.gz
- bolt-cli-v0.1.0-linux-arm64-musl.tar.gz

### Android（仅 GUI）

- bolt-v0.1.0-android-universal.apk
- bolt-v0.1.0-android-arm64.apk

### iOS（仅 GUI）

- bolt-v0.1.0-ios-arm64.ipa

### 校验和文件（推荐）

- checksums.txt（包含全部产物的 SHA-256 哈希清单）

## 五、红线禁止规则（CI 可强制拦截）

- 禁止 GUI 包带 gui 后缀
- 禁止 CLI 包省略 cli 标识
- 禁止大小写混用、下划线、点号分隔（除 `.AppImage` 扩展名特例）
- 禁止缺失版本、架构字段
