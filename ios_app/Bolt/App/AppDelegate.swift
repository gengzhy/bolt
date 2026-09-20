//
//  AppDelegate.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import UIKit

public final class AppDelegate: NSObject, UIApplicationDelegate {

    private var backgroundTaskId: UIBackgroundTaskIdentifier = .invalid

    public func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        return true
    }

    public func applicationDidEnterBackground(_ application: UIApplication) {
        // 当应用切入后台且存在进行中传输时，向 iOS 系统申请后台宽限运行时间
        let hasActiveTasks = BoltEngine.shared.uiState.tasks.values.contains { TaskStates.isActive($0.state) }
        guard hasActiveTasks else { return }

        backgroundTaskId = application.beginBackgroundTask(withName: "BoltActiveTransfer") { [weak self] in
            guard let self = self else { return }
            application.endBackgroundTask(self.backgroundTaskId)
            self.backgroundTaskId = .invalid
        }
    }

    public func applicationWillEnterForeground(_ application: UIApplication) {
        if backgroundTaskId != .invalid {
            application.endBackgroundTask(backgroundTaskId)
            backgroundTaskId = .invalid
        }
        // 回到前台后立即刷新一轮设备发现
        BoltEngine.shared.probeNetwork()
    }

    public func applicationWillTerminate(_ application: UIApplication) {
        BoltEngine.shared.shutdown()
    }
}
