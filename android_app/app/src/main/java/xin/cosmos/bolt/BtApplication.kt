package xin.cosmos.bolt

import android.app.Application
import xin.cosmos.bolt.engine.BtEngine

/**
 * 进程级入口：在 Application 生命周期初始化 Rust 核心，
 * 使引擎/NSD/任务状态独立于 Activity 生命周期（锁屏、旋转、
 * 后台均不中断传输，见 [BtEngine] 说明）。
 */
class BtApplication : Application() {

    override fun onCreate() {
        super.onCreate()
        BtEngine.init(this)
        PersistentNotification.show(this)
    }
}
