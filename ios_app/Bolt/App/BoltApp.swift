//
//  BoltApp.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

@main
struct BoltApp: App {

    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @StateObject private var engine = BoltEngine.shared

    var body: some Scene {
        WindowGroup {
            MainView()
                .preferredColorScheme(.dark)
                .onAppear {
                    engine.start()
                }
        }
    }
}
