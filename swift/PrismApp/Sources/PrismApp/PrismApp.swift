import SwiftUI
import PrismFFI

@main
struct PrismApp: App {
  private let persistenceController = PersistenceController.shared
  @StateObject private var sessionStore = SessionStore()

  var body: some Scene {
    WindowGroup {
      ContentView()
        .environment(\.managedObjectContext, persistenceController.container.viewContext)
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
