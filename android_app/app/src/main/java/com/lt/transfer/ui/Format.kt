package com.lt.transfer.ui

/** 展示格式化工具（字节数、速率、剩余时间）。 */
object Format {

    /** 1024 进制字节数：`3.2 GB` / `512 KB`。 */
    fun bytes(n: Long): String {
        if (n < 1024) return "$n B"
        val units = listOf("KB", "MB", "GB", "TB")
        var v = n.toDouble()
        var u = -1
        do {
            v /= 1024.0
            u++
        } while (v >= 1024.0 && u < units.size - 1)
        val text = if (v >= 100) v.toLong().toString() else "%.1f".format(v)
        return "$text ${units[u]}"
    }

    /** 速率 → 字节单位（格式：`xx MB/s`）。 */
    fun rate(bps: Long): String {
        if (bps <= 0) return "0 B/s"
        val units = listOf("B", "KB", "MB", "GB", "TB")
        var v = bps.toDouble()
        var u = 0
        while (v >= 1024.0 && u < units.size - 1) {
            v /= 1024.0
            u++
        }
        val text = if (v >= 100) v.toLong().toString() else "%.1f".format(v)
        return "$text ${units[u]}/s"
    }

    /** 剩余时间 → `1 分 23 秒` 形式。 */
    fun eta(secs: Long): String = when {
        secs <= 0 -> "—"
        secs < 60 -> "${secs} 秒"
        secs < 3600 -> "${secs / 60} 分 ${secs % 60} 秒"
        else -> "${secs / 3600} 时 ${(secs % 3600) / 60} 分"
    }

    /** 任务状态 → 中文。 */
    fun state(state: String): String = when (state) {
        "waiting_accept" -> "等待对方接受"
        "transferring" -> "传输中"
        "paused" -> "已暂停"
        "done" -> "已完成"
        "cancelled" -> "已取消"
        "rejected" -> "对方已拒绝"
        "error" -> "出错"
        else -> state
    }

    /** 设备类型码 → 「类型（操作系统）」中文标签（1=Windows/PC，2=Android，见 protocol_spec）。 */
    fun deviceType(type: Int): String = when (type) {
        1 -> "电脑（Windows）"
        2 -> "手机（Android）"
        3 -> "手机（iOS）"
        else -> "设备"
    }
}
