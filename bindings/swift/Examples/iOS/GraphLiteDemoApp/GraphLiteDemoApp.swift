import SwiftUI
import GraphLite

@main
struct GraphLiteDemoApp: App {
    @StateObject private var databaseManager = DatabaseManager()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(databaseManager)
        }
    }
}
