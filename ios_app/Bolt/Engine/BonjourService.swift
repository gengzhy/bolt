//
//  BonjourService.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import Foundation
import Network

/// iOS 原生 Bonjour (mDNS / DNS-SD) 发布与发现服务。
///
/// 遵循 Apple 规范，通过系统 mDNS 守护进程交互，免申请原始 UDP 多播套接字权限，
/// 发现结果解析后经 `bt_nsd_inject_device` 注入 Rust 核心设备表。
public final class BonjourService: NSObject {

    public static let shared = BonjourService()

    private let serviceType = "_bolt._udp."
    private let domain = "local."

    private var netService: NetService?
    private var serviceBrowser: NetServiceBrowser?
    private var resolvingServices: Set<NetService> = []

    /// 记录已解析的服务信息：UUID -> 注入的 JSON 字符串
    private var resolvedDevices: [String: String] = [:]
    private let lock = NSLock()

    private var reinjectTimer: Timer?
    private var isStealth: Bool = false

    private override init() {
        super.init()
    }

    // =========================================================================
    // 生命周期控制
    // =========================================================================

    /// 启动 Bonjour 广播与网络扫描
    public func start() {
        stop()

        startPublishing()
        startBrowsing()

        // 核心设备表具有 10s TTL 清扫机制，启动 4s 周期重注入刷新 last_seen
        reinjectTimer = Timer.scheduledTimer(withTimeInterval: 4.0, repeats: true) { [weak self] _ in
            self?.reinjectAll()
        }
    }

    /// 停止全部广播与发现
    public func stop() {
        reinjectTimer?.invalidate()
        reinjectTimer = nil

        stopPublishing()
        stopBrowsing()

        lock.lock()
        resolvedDevices.removeAll()
        resolvingServices.removeAll()
        lock.unlock()
    }

    /// 根据当前隐身模式动态切换广播
    public func updateStealthMode(isStealth: Bool) {
        self.isStealth = isStealth
        if isStealth {
            stopPublishing()
        } else {
            startPublishing()
        }
    }

    // =========================================================================
    // 本机服务广播 (Advertiser)
    // =========================================================================

    private func startPublishing() {
        guard !isStealth else { return }

        // 从底层 Rust 获取本机信息
        let localInfoJson = BoltNative.shared.btGetLocalInfo()
        guard let data = localInfoJson.data(using: .utf8),
              let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let uuid = dict["uuid"] as? String,
              let name = dict["name"] as? String,
              let qport = dict["qport"] as? Int,
              let tport = dict["tport"] as? Int else {
            return
        }

        let instanceName = "bolt-\(uuid)"
        let port = Int32(tport > 0 ? tport : qport)

        netService = NetService(domain: domain, type: serviceType, name: instanceName, port: port)
        guard let service = netService else { return }

        service.delegate = self

        // 构造 TXT 字典（必须与 crates/discovery/src/mdns.rs 严格一致）
        let txtDict: [String: String] = [
            "uuid": uuid,
            "dt": "3", // DeviceType::Ios = 3
            "name": name,
            "qport": "\(qport)",
            "tport": "\(tport)",
            "ver": "2",
            "stealth": isStealth ? "1" : "0",
            "ptcp": "0"
        ]

        var txtDataDict: [String: Data] = [:]
        for (k, v) in txtDict {
            txtDataDict[k] = v.data(using: .utf8)
        }
        service.setTXTRecord(NetService.data(fromTXTRecord: txtDataDict))
        service.publish()
    }

    private func stopPublishing() {
        netService?.stop()
        netService = nil
    }

    // =========================================================================
    // 局域网服务发现 (Browser)
    // =========================================================================

    private func startBrowsing() {
        serviceBrowser = NetServiceBrowser()
        serviceBrowser?.delegate = self
        serviceBrowser?.searchForServices(ofType: serviceType, inDomain: domain)
    }

    private func stopBrowsing() {
        serviceBrowser?.stop()
        serviceBrowser = nil
    }

    private func reinjectAll() {
        lock.lock()
        let jsons = Array(resolvedDevices.values)
        lock.unlock()

        for json in jsons {
            BoltNative.shared.btInjectDevice(json: json)
        }
    }
}

// MARK: - NetServiceBrowserDelegate
extension BonjourService: NetServiceBrowserDelegate {

    public func netServiceBrowser(_ browser: NetServiceBrowser, didFind service: NetService, moreComing: Bool) {
        service.delegate = self
        lock.lock()
        resolvingServices.insert(service)
        lock.unlock()

        // 异步解析主机与地址信息（超时 5 秒）
        service.resolve(withTimeout: 5.0)
    }

    public func netServiceBrowser(_ browser: NetServiceBrowser, didRemove service: NetService, moreComing: Bool) {
        lock.lock()
        resolvingServices.remove(service)
        lock.unlock()

        // 服务下线时尝试根据名字推导 UUID 并下线
        let uuid = service.name.replacingOccurrences(of: "bolt-", with: "")
        if !uuid.isEmpty {
            lock.lock()
            resolvedDevices.removeValue(forKey: uuid)
            lock.unlock()
            BoltNative.shared.btRemoveDevice(uuid: uuid)
        }
    }
}

// MARK: - NetServiceDelegate
extension BonjourService: NetServiceDelegate {

    public func netServiceDidResolveAddress(_ sender: NetService) {
        defer {
            lock.lock()
            resolvingServices.remove(sender)
            lock.unlock()
        }

        // 1. 提取 IPv4 地址
        guard let addresses = sender.addresses, !addresses.isEmpty else { return }
        var resolvedIp: String?

        for addrData in addresses {
            var storage = sockaddr_storage()
            (addrData as NSData).getBytes(&storage, length: MemoryLayout<sockaddr_storage>.size)

            if storage.ss_family == sa_family_t(AF_INET) {
                var addrIn = sockaddr_in()
                (addrData as NSData).getBytes(&addrIn, length: MemoryLayout<sockaddr_in>.size)
                var ipBuffer = [CChar](repeating: 0, count: Int(INET_ADDRSTRLEN))
                if inet_ntop(AF_INET, &addrIn.sin_addr, &ipBuffer, socklen_t(INET_ADDRSTRLEN)) != nil {
                    let ipStr = String(cString: ipBuffer)
                    if ipStr != "127.0.0.1" {
                        resolvedIp = ipStr
                        break
                    }
                }
            }
        }

        guard let ip = resolvedIp else { return }

        // 2. 提取 TXT 记录
        var uuid = sender.name.replacingOccurrences(of: "bolt-", with: "")
        var name = sender.name
        var dt = 0
        var qport = sender.port > 0 ? sender.port : 8899
        var tport = sender.port > 0 ? sender.port : 8899
        var ver = 2
        var stealth = false
        var ptcp = false

        if let txtData = sender.txtRecordData() {
            let dict = NetService.dictionary(fromTXTRecord: txtData)
            if let uData = dict["uuid"], let uStr = String(data: uData, encoding: .utf8), !uStr.isEmpty {
                uuid = uStr
            }
            if let nData = dict["name"], let nStr = String(data: nData, encoding: .utf8), !nStr.isEmpty {
                name = nStr
            }
            if let dtData = dict["dt"], let dtStr = String(data: dtData, encoding: .utf8), let dtVal = Int(dtStr) {
                dt = dtVal
            }
            if let qpData = dict["qport"], let qpStr = String(data: qpData, encoding: .utf8), let qpVal = Int(qpStr) {
                qport = qpVal
            }
            if let tpData = dict["tport"], let tpStr = String(data: tpData, encoding: .utf8), let tpVal = Int(tpStr) {
                tport = tpVal
            }
            if let vData = dict["ver"], let vStr = String(data: vData, encoding: .utf8), let vVal = Int(vStr) {
                ver = vVal
            }
            if let sData = dict["stealth"], let sStr = String(data: sData, encoding: .utf8) {
                stealth = (sStr == "1" || sStr.lowercased() == "true")
            }
            if let pData = dict["ptcp"], let pStr = String(data: pData, encoding: .utf8) {
                ptcp = (pStr == "1" || pStr.lowercased() == "true")
            }
        }

        guard !uuid.isEmpty else { return }

        // 3. 构造注入 JSON 并提交至 Rust 核心
        let injectDict: [String: Any] = [
            "uuid": uuid,
            "ip": ip,
            "name": name,
            "dt": dt,
            "qport": qport,
            "tport": tport,
            "ver": ver,
            "stealth": stealth,
            "ptcp": ptcp
        ]

        if let jsonData = try? JSONSerialization.data(withJSONObject: injectDict),
           let jsonStr = String(data: jsonData, encoding: .utf8) {
            lock.lock()
            resolvedDevices[uuid] = jsonStr
            lock.unlock()

            BoltNative.shared.btInjectDevice(json: jsonStr)
        }
    }

    public func netService(_ sender: NetService, didNotResolve errorDict: [String: NSNumber]) {
        lock.lock()
        resolvingServices.remove(sender)
        lock.unlock()
    }
}
