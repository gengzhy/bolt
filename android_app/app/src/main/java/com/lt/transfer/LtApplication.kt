package com.lt.transfer

import android.app.Application
import com.lt.transfer.engine.LtEngine

/**
 * 进程级入口：在 Application 生命周期初始化 Rust 核心，
 * 使引擎/NSD/任务状态独立于 Activity 生命周期（锁屏、旋转、
 * 后台均不中断传输，见 [LtEngine] 说明）。
 */
class LtApplication : Application() {

    override fun onCreate() {
        super.onCreate()
        LtEngine.init(this)
    }
}
