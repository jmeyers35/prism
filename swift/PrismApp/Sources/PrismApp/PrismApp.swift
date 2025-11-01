import SwiftUI
import PrismFFI

@main
struct PrismApp: App {
  @StateObject private var sessionStore = SessionStore()

  var body: some Scene {
    WindowGroup {
      ContentView()
        .environmentObject(sessionStore)
    }
    .commands {
      CommandGroup(after: .newItem) {
        Button("Open Repository…", action: sessionStore.presentRepositoryPicker)
          .keyboardShortcut("o", modifiers: [.command])
      }
    }
  }
}
