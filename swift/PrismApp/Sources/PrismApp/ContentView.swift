import SwiftUI

struct ContentView: View {
  @EnvironmentObject private var sessionStore: SessionStore

  var body: some View {
    switch sessionStore.phase {
    case .idle:
      WelcomeView(openRepository: sessionStore.presentRepositoryPicker)
    case .loading(let url):
      LoadingView(repositoryURL: url)
    case .ready(let viewModel):
      SessionShellView(viewModel: viewModel)
        .environmentObject(sessionStore)
    case .failed(let message):
      ErrorView(message: message, retry: sessionStore.presentRepositoryPicker)
    }
  }
}

private struct WelcomeView: View {
  var openRepository: () -> Void

  var body: some View {
    VStack(spacing: 12) {
      Image(systemName: "app.dashed")
        .resizable()
        .scaledToFit()
        .frame(width: 64, height: 64)
        .foregroundStyle(.secondary)

      Text("Welcome to Prism")
        .font(.title2.weight(.semibold))

      Text("Open a repository to start reviewing changes.")
        .font(.body)
        .foregroundStyle(.secondary)

      Button(action: openRepository) {
        Text("Open Repository…")
          .padding(.horizontal, 20)
          .padding(.vertical, 8)
      }
      .buttonStyle(.borderedProminent)
    }
    .padding(40)
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(Color(nsColor: .windowBackgroundColor))
  }
}

private struct LoadingView: View {
  var repositoryURL: URL

  var body: some View {
    VStack(spacing: 12) {
      ProgressView()
        .progressViewStyle(.circular)

      Text("Loading \(repositoryURL.lastPathComponent)…")
        .font(.body)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(Color(nsColor: .windowBackgroundColor))
  }
}

private struct ErrorView: View {
  var message: String
  var retry: () -> Void

  var body: some View {
    VStack(spacing: 12) {
      Image(systemName: "exclamationmark.triangle")
        .resizable()
        .scaledToFit()
        .frame(width: 48, height: 48)
        .foregroundStyle(.orange)

      Text("We couldn't open that repository.")
        .font(.title3.weight(.semibold))

      Text(message)
        .font(.body)
        .multilineTextAlignment(.center)
        .foregroundStyle(.secondary)

      Button("Try Again", action: retry)
        .keyboardShortcut(.defaultAction)
    }
    .padding(40)
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(Color(nsColor: .windowBackgroundColor))
  }
}

#if DEBUG
  struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
      ContentView()
        .environmentObject(SessionStorePreviewFactory.readyStore())
    }
  }

  enum SessionStorePreviewFactory {
    @MainActor
    static func readyStore() -> SessionStore {
      let persistence = PersistenceController.preview
      let storage = SessionStorage(controller: persistence)
      let store = SessionStore(client: MockPreviewClient(), storage: storage)
      store.injectPreviewState(
        phase: .ready(
          SessionViewModel(
            repositoryName: "prism",
            repositoryPath: "/Users/example/dev/prism",
            defaultBranch: "main",
            currentBranch: "feature/prism",
            hasUncommittedChanges: true
          )
        )
      )
      return store
    }
  }

  private final class MockPreviewClient: PrismSessionClient {
    func openSession(at path: String) async throws -> any PrismSession {
      throw PrismPreviewError.unimplemented
    }
  }

  private enum PrismPreviewError: Error {
    case unimplemented
  }
#endif
