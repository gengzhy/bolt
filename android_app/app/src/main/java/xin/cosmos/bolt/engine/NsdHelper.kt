package xin.cosmos.bolt.engine

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.util.Log
import xin.cosmos.bolt.ffi.Native
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.net.Inet4Address

/**
 * NSD（NsdManager）发现桥（进程生命周期）。
 *
 * Android 上 Rust 侧不跑 mDNS（use_mdns=false）：
 * - 发现：NsdManager 浏览 `_bolt._udp.` 服务，解析后经
 *   `bt_nsd_inject_device` 注入 Rust 设备表（与 Windows 侧 mdns-sd 广播兼容）；
 * - 广播：取 `bt_get_local_info` 注册同名同属性的本机服务，
 *   使 PC 端 mDNS 浏览能发现本机；
 * - 多播锁：进程级持有，锁屏后仍可收组播。
 */
object NsdHelper {

    private const val TAG = "NsdHelper"

    // 注册/浏览用短名（NsdManager 自动补 .local. 域）；
    // 回调里的 serviceType 各版本形态不一，统一用前缀匹配
    private const val BROWSE_TYPE = "_bolt._udp."
    private const val REGISTER_TYPE = "_bolt._udp."
    private const val SERVICE_TYPE_PREFIX = "_bolt._udp"
    private const val INSTANCE_PREFIX = "bolt-"

    /** 重注入间隔：必须小于核心设备表 TTL（10s）。 */
    private const val REINJECT_INTERVAL_MS = 4000L

    private var nsdManager: NsdManager? = null
    private var discoveryListener: NsdManager.DiscoveryListener? = null
    private var registrationListener: NsdManager.RegistrationListener? = null
    private var multicastLock: WifiManager.MulticastLock? = null
    private var registered = false
    private var discoveryStarted = false

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    /** 已解析服务：uuid → 注入 JSON。周期重注入刷新 last_seen（核心设备表 10s 过期清扫）。 */
    private val resolvedServices = HashMap<String, String>()

    fun start(context: Context) {
        if (nsdManager != null) return
        val app = context.applicationContext
        nsdManager = app.getSystemService(Context.NSD_SERVICE) as NsdManager

        val wifi = app.getSystemService(Context.WIFI_SERVICE) as WifiManager
        multicastLock = wifi.createMulticastLock("bt:nsd").apply {
            setReferenceCounted(false)
            acquire()
        }

        discoveryListener = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(serviceType: String) {
                discoveryStarted = true
                Log.i(TAG, "NSD 发现已启动：$serviceType")
            }
            override fun onDiscoveryStopped(serviceType: String) {
                discoveryStarted = false
            }
            override fun onServiceFound(info: NsdServiceInfo) {
                // 不同系统版本回调的 serviceType 形态不一（带/不带 .local. 后缀），
                // 用前缀匹配；解析结果再按属性注入
                Log.d(TAG, "onServiceFound：type=${info.serviceType} name=${info.serviceName}")
                if (!info.serviceType.startsWith(SERVICE_TYPE_PREFIX)) return
                nsdManager?.resolveService(info, object : NsdManager.ResolveListener {
                    override fun onResolveFailed(serviceInfo: NsdServiceInfo, errorCode: Int) {
                        Log.w(TAG, "resolve 失败：$errorCode（${serviceInfo.serviceName}）")
                    }
                    override fun onServiceResolved(serviceInfo: NsdServiceInfo) {
                        injectResolved(serviceInfo)
                    }
                })
            }
            override fun onServiceLost(info: NsdServiceInfo) {
                // lost 回调通常不带 TXT 属性：优先取属性，退化从服务名解析
                val uuid = attrStr(info.attributes, "uuid")
                    ?: info.serviceName?.removePrefix(INSTANCE_PREFIX)
                if (!uuid.isNullOrEmpty()) {
                    synchronized(resolvedServices) { resolvedServices.remove(uuid) }
                    Native.btNsdRemoveDevice(uuid)
                }
            }
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                discoveryStarted = false
                Log.w(TAG, "启动发现失败：$errorCode（可能在等权限，授权后 ensureDiscovery 重试）")
            }
            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
        }
        nsdManager?.discoverServices(BROWSE_TYPE, NsdManager.PROTOCOL_DNS_SD, discoveryListener)

        registrationListener = object : NsdManager.RegistrationListener {
            override fun onServiceRegistered(info: NsdServiceInfo) {
                registered = true
            }
            override fun onRegistrationFailed(info: NsdServiceInfo, errorCode: Int) {
                registered = false
                Log.w(TAG, "NSD 注册失败：$errorCode")
            }
            override fun onServiceUnregistered(info: NsdServiceInfo) {
                registered = false
            }
            override fun onUnregistrationFailed(info: NsdServiceInfo, errorCode: Int) {}
        }
        applyStealth()

        // 周期重注入已解析服务：核心设备表 10s TTL 清扫，而 NSD 只在
        // resolve 时注入一次，不重注入设备会在 ~10s 后从列表消失。
        scope.launch {
            while (true) {
                delay(REINJECT_INTERVAL_MS)
                val snapshot = synchronized(resolvedServices) { resolvedServices.values.toList() }
                for (json in snapshot) Native.btNsdInjectDevice(json)
            }
        }
    }

    /**
     * 权限到手后重试发现（API 33+ NsdManager 需要 NEARBY_WIFI_DEVICES，
     * Application 阶段启动的发现可能因权限未授而失败）。
     */
    fun ensureDiscovery() {
        if (discoveryStarted) return
        val manager = nsdManager ?: return
        val listener = discoveryListener ?: return
        manager.discoverServices(BROWSE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)
    }

    /**
     * 按当前隐身配置注册/注销本机服务。
     * （ stealth_mode=true 时不出现在对方设备列表，但仍可被直连。）
     */
    @Synchronized
    fun applyStealth() {        val manager = nsdManager ?: return
        val info = try {
            JSONObject(Native.btGetLocalInfo())
        } catch (_: Exception) {
            return
        }
        val stealth = info.optBoolean("stealth")
        if (stealth) {
            if (registered) {
                registrationListener?.let(manager::unregisterService)
                registered = false
            }
            return
        }
        if (registered) return
        val uuid = info.optString("uuid")
        if (uuid.isEmpty()) return
        val port = info.optInt("qport", 8899)
        val svc = NsdServiceInfo().apply {
            serviceName = "bolt-$uuid"
            serviceType = REGISTER_TYPE
            setPort(port)
            setAttribute("uuid", uuid)
            setAttribute("name", info.optString("name", uuid))
            setAttribute("dt", info.optInt("dt", 2).toString())
            setAttribute("qport", port.toString())
            setAttribute("tport", info.optInt("tport", port).toString())
            setAttribute("ver", info.optInt("ver", 2).toString())
            setAttribute("stealth", "0")
            setAttribute("ptcp", if (info.optBoolean("ptcp")) "1" else "0")
        }
        manager.registerService(svc, NsdManager.PROTOCOL_DNS_SD, registrationListener)
    }

    /**
     * 重新注册本机服务（监听端口变更后调用）：先注销，再按最新
     * `bt_get_local_info`（新端口）注册。注销为异步回调，短暂延时
     * 后再注册，避免与「已注册」竞态。
     */
    fun reregister() {
        val manager = nsdManager ?: return
        if (registered) {
            registrationListener?.let(manager::unregisterService)
            registered = false
        }
        scope.launch {
            delay(300)
            applyStealth()
        }
    }

    /** use_mdns 设置切换：开关 NSD 的发现与广播。 */
    fun applyEnabled(enabled: Boolean) {
        val manager = nsdManager ?: return
        if (enabled) {
            ensureDiscovery()
            applyStealth()
        } else {
            if (discoveryStarted) {
                discoveryListener?.let(manager::stopServiceDiscovery)
                discoveryStarted = false
            }
            if (registered) {
                registrationListener?.let(manager::unregisterService)
                registered = false
            }
        }
    }

    /** NSD 属性值（ByteArray）按 UTF-8 取字符串。 */
    private fun attrStr(attrs: Map<String, ByteArray>?, key: String): String? =
        attrs?.get(key)?.let { String(it, Charsets.UTF_8) }

    private fun injectResolved(info: NsdServiceInfo) {
        val attrs = info.attributes ?: return
        val uuid = attrStr(attrs, "uuid") ?: return
        val host = info.host
        val ip = when (host) {
            is Inet4Address -> host.hostAddress
            else -> host?.hostAddress
        } ?: return
        val qport = attrStr(attrs, "qport")?.toIntOrNull() ?: info.port
        val tport = attrStr(attrs, "tport")?.toIntOrNull() ?: qport
        Log.i(TAG, "NSD 解析到设备：$uuid @ $ip:$qport")
        val json = JSONObject()
            .put("uuid", uuid)
            .put("ip", ip)
            .put("name", attrStr(attrs, "name") ?: uuid)
            .put("dt", attrStr(attrs, "dt")?.toIntOrNull() ?: 2)
            .put("qport", qport)
            .put("tport", tport)
            .put("ver", attrStr(attrs, "ver")?.toIntOrNull() ?: 2)
            .put("ptcp", attrStr(attrs, "ptcp") == "1")
            .toString()
        synchronized(resolvedServices) { resolvedServices[uuid] = json }
        Native.btNsdInjectDevice(json)
    }

    /**
     * 主动刷新（对应 UI「刷新」按钮）：立即重注入全部已知服务，
     * 使设备马上回到列表（对抗 10s TTL 清扫）；新设备由常驻浏览
     * 的 onServiceFound 回调补入。
     */
    fun refreshNow() {
        val snapshot = synchronized(resolvedServices) { resolvedServices.values.toList() }
        Log.d(TAG, "refreshNow：重注入 ${snapshot.size} 个已解析服务")
        for (json in snapshot) Native.btNsdInjectDevice(json)
    }
}
