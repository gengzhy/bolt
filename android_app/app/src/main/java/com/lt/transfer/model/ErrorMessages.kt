package com.lt.transfer.model

import android.content.Context
import com.lt.transfer.R

/**
 * 统一错误码 → 中文文案 / 资源 ID（需求分析报告 §6.1/§6.2）。
 * 码值与 crates/utils/src/error.rs 的 LtError::code() 一一对应；
 * UI 层只负责按码展示，不做任何自行处理。
 */
object ErrorMessages {

    fun resId(code: Int): Int = when (code) {
        0 -> R.string.err_code_0
        -1 -> R.string.err_code_neg_1
        -2 -> R.string.err_code_neg_2
        -3 -> R.string.err_code_neg_3
        -4 -> R.string.err_code_neg_4
        -5 -> R.string.err_code_neg_5
        -6 -> R.string.err_code_neg_6
        -7 -> R.string.err_code_neg_7
        -8 -> R.string.err_code_neg_8
        -9 -> R.string.err_code_neg_9
        -10 -> R.string.err_code_neg_10
        -11 -> R.string.err_code_neg_11
        -12 -> R.string.err_code_neg_12
        -13 -> R.string.err_code_neg_13
        -14 -> R.string.err_code_neg_14
        -15 -> R.string.err_code_neg_15
        else -> R.string.err_code_unknown
    }

    fun of(context: Context, code: Int): String {
        val id = resId(code)
        return if (id == R.string.err_code_unknown) context.getString(id, code) else context.getString(id)
    }

    fun of(code: Int, fallback: String? = null): String = when (code) {
        0 -> "成功"
        -1 -> "参数非法"
        -2 -> "网络端口被占用"
        -3 -> "连接超时或远端设备离线"
        -4 -> "文件不存在或无读取权限"
        -5 -> "磁盘剩余空间不足"
        -6 -> "文件校验失败（内容损坏），请重新传输"
        -7 -> "设备发现不可用（组播可能被拦截，可尝试手动输入 IP 连接）"
        -8 -> "文件读取失败（内存映射错误）"
        -9 -> "任务已被取消"
        -10 -> "系统权限不足（文件/网络/通知）"
        -11 -> "配对失败（对方拒绝或验证码不一致）"
        -12 -> "传输被对方拒绝"
        -13 -> "协议版本不兼容，请升级两端应用"
        -14 -> "对端证书指纹变更（疑似中间人），连接已阻止"
        -15 -> "内部错误（详见本地日志）"
        else -> fallback ?: "未知错误（$code）"
    }
}
