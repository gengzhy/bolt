# Security Policy / 安全策略

Bolt is committed to ensuring the highest level of security and privacy for local file transfers. We appreciate the responsible disclosure of security vulnerabilities by the community.

Bolt 致力于为局域网文件传输提供最高标准的安全与隐私保护。我们非常重视社区的安全反馈，并感谢负责任的漏洞披露。

---

## 🛡️ Supported Versions / 支持版本

Security patches are actively maintained for the latest versions of Bolt.

| Version / 版本 | Supported / 是否支持安全维护 | Status / 状态 |
|:--------------:|:-------------------------:|:------------:|
| `0.1.x` (main) | :white_check_mark:         | 当前开发与活跃维护分支 / Active |
| `< 0.1.0`      | :x:                       | 早期开发原型，不受支持 / Unsupported |

---

## 🔒 Security Model & Architecture / 安全模型与设计

Bolt operates strictly within the local area network (LAN) and adheres to the following principles:

1. **Pure LAN & Zero Cloud (纯局域网与零云端)**:
   - No relay servers, no external accounts, and zero internet dependencies.
   - All network traffic remains strictly within the local network.
2. **Mandatory Encryption (强制加密传输)**:
   - All transport connections (QUIC and TCP fallback) require TLS 1.3 with modern cipher suites.
   - No plaintext transmission fallback is permitted under any circumstances.
3. **Cryptographic Identity (密码学设备身份)**:
   - Each device generates a persistent Ed25519 keypair and self-signed X.509 certificate.
   - Device fingerprints are derived via SHA-256 / BLAKE3.
4. **Mutual Pairing & TOFU (双向配对与信任机制)**:
   - First-time connections require explicit mutual pairing via a 6-digit dynamic code.
   - TOFU (Trust-On-First-Use) whitelist is stored locally in an isolated application directory.
5. **Data Integrity & Atomic File Operations (完整性与落盘防污染)**:
   - Parallel BLAKE3 checksum calculation ensures byte-level integrity against transmission corruption or tampering.
   - Incomplete incoming files are strictly sandboxed with `.bttmp` extensions until verified, preventing partial or corrupt file execution.
6. **Zero Telemetry & Private Logging (零数据采集与日志安全)**:
   - Bolt collects zero telemetry or analytics.
   - Debug and audit logs never record private keys, passcodes, or payload contents.

---

## 🎯 Scope of Security Vulnerabilities / 漏洞评估范围

### In-Scope (适用漏洞范围)
We actively welcome reports regarding:
- **Cryptographic flaws**: Bypass of TLS 1.3 validation, identity spoofing, or pairing verification bypass.
- **Path Traversal**: Arbitrary file write/overwrite outside the designated download directory (e.g., directory traversal via `../` or absolute paths).
- **Remote Code Execution (RCE)**: Memory safety violations, buffer overflows in the Rust core, FFI boundary vulnerabilities, or malicious payload execution.
- **Denial of Service (DoS)**: Crafted frames or protocol packets causing unhandled panics, resource exhaustion (OOM), or infinite loops.
- **Data Leakage**: Unintended transmission of unencrypted metadata or credential material across the network.

### Out-of-Scope (不属于安全漏洞范畴)
- Network-level attacks assuming physical/compromised LAN access (e.g., ARP spoofing, broadcast storm) that do not break TLS 1.3 or pairing verification.
- Attacks requiring local root / administrator / physical access to the target device.
- Social engineering attacks tricking the user into manually confirming pairing or accepting untrusted files.
- Denial of Service caused simply by flooding normal network bandwidth.

---

## 🚨 Reporting a Vulnerability / 漏洞报告途径

> **⚠️ Important**: Please **DO NOT** report security vulnerabilities via public GitHub issues, discussions, or pull requests.
> 
> **请勿** 通过公开的 GitHub Issues、Discussions 或 Pull Request 提交任何潜在的安全漏洞。

If you believe you have discovered a vulnerability in Bolt, please report it via one of the following channels:

### Option 1: GitHub Private Vulnerability Reporting (Recommended / 推荐)
You can use GitHub's built-in private security advisory tool:
1. Navigate to the repository's **Security** tab.
2. Click **Report a vulnerability** under "Advisories".
3. Provide the details and submit securely.

### Option 2: Security Email / 邮箱直报
Alternatively, email the maintainer directly:
- **Email**: `genggzy@gmail.com`
- **Subject**: `[SECURITY] Bolt Vulnerability Report - <Brief Description>`

---

## 📝 Report Content / 报告建议包含的内容

To help us triage and resolve the issue swiftly, please include:
1. **Description**: A clear description of the vulnerability and its potential impact.
2. **Steps to Reproduce / PoC**: Clear steps, script, or minimal test case to reproduce the issue.
3. **Environment**:
   - Bolt version / Git commit hash
   - Operating system and version (Windows 10/11, Android version)
   - Network environment (Wi-Fi, Ethernet, Hotspot)
4. **Suggested Fix (Optional)**: Any insights or pull request proposals for resolving the vulnerability.

---

## ⏱️ Response & Disclosure Timeline / 响应与披露流程

- **Initial Acknowledgment**: Within **48 hours** of receiving the report.
- **Assessment & Triage**: Within **7 days**, confirming the severity and providing an estimated resolution timeframe.
- **Fix & Patch Release**: Security fixes will be developed and released in a prioritized patch release.
- **Coordinated Public Disclosure**: Once the patch is available, we will publish a security advisory and credit the reporter (unless anonymity is requested).
