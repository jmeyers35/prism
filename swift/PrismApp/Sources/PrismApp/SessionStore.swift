import AppKit
import Combine
import Foundation
import PrismFFI

@MainActor
final class SessionStore: ObservableObject {
  @Published private(set) var phase: SessionPhase = .idle

  private let client: any PrismSessionClient
  private var activeSession: (any PrismSession)?
  private var loadIdentifier: UUID?

  init(client: any PrismSessionClient = PrismCoreClientAdapter()) {
    self.client = client
  }

  func presentRepositoryPicker() {
    let panel = NSOpenPanel()
    panel.prompt = "Open"
    panel.canChooseFiles = false
    panel.canChooseDirectories = true
    panel.allowsMultipleSelection = false
    panel.canCreateDirectories = false

    panel.begin { [weak self] result in
      guard result == .OK, let url = panel.url else { return }
      Task { [weak self] in
        await self?.openSession(at: url)
      }
    }
  }

  func openSession(at url: URL) async {
    let previousSession = activeSession
    let previousPhase = phase
    let ticket = UUID()
    loadIdentifier = ticket
    phase = .loading(url)

    do {
      let session = try await client.openSession(at: url.path)
      let info = try await session.repositoryInfo()
      let status = try await session.workspaceStatus()

      guard loadIdentifier == ticket else { return }

      activeSession = session
      phase = .ready(makeViewModel(info: info, status: status))
      loadIdentifier = nil
    } catch {
      guard loadIdentifier == ticket else { return }
      loadIdentifier = nil

      if let previousSession {
        activeSession = previousSession
        phase = previousPhase
      } else {
        activeSession = nil
        phase = .failed(Self.describe(error))
      }
    }
  }

  func closeSession() {
    loadIdentifier = nil
    activeSession = nil
    phase = .idle
  }

  func refreshActiveSession() async {
    guard let session = activeSession else { return }
    let expectedSessionID = ObjectIdentifier(session as AnyObject)
    let expectedLoadIdentifier = loadIdentifier

    do {
      let info = try await session.repositoryInfo()
      let status = try await session.workspaceStatus()
      guard loadIdentifier == expectedLoadIdentifier else { return }
      guard let currentSession = activeSession else { return }
      guard ObjectIdentifier(currentSession as AnyObject) == expectedSessionID else { return }

      phase = .ready(makeViewModel(info: info, status: status))
    } catch {
      guard loadIdentifier == expectedLoadIdentifier else { return }
      guard let activeSession = activeSession else { return }
      guard ObjectIdentifier(activeSession as AnyObject) == expectedSessionID else { return }
      // Preserve the current UI state; transient refresh failures should not eject the user.
      return
    }
  }

  func hasActiveSession() -> Bool {
    activeSession != nil
  }

  private static func describe(_ error: Error) -> String {
    if let coreError = error as? PrismCoreError {
      switch coreError {
      case let .notARepository(message),
        let .bareRepository(message),
        let .missingHeadRevision(message),
        let .git(message),
        let .io(message),
        let .unimplemented(message),
        let .internalError(message):
        return message
      }
    }
    return error.localizedDescription
  }

  private func makeViewModel(info: RepositoryInfo, status: WorkspaceStatus) -> SessionViewModel {
    SessionViewModel(
      repositoryName: URL(fileURLWithPath: info.root).lastPathComponent,
      repositoryPath: info.root,
      defaultBranch: info.defaultBranch,
      currentBranch: status.currentBranch,
      hasUncommittedChanges: status.dirty
    )
  }
}

enum SessionPhase: Equatable {
  case idle
  case loading(URL)
  case ready(SessionViewModel)
  case failed(String)
}

struct SessionViewModel: Equatable {
  var repositoryName: String
  var repositoryPath: String
  var defaultBranch: String?
  var currentBranch: String?
  var hasUncommittedChanges: Bool
}

protocol PrismSessionClient {
  func openSession(at path: String) async throws -> any PrismSession
}

protocol PrismSession: AnyObject {
  func repositoryInfo() async throws -> RepositoryInfo
  func workspaceStatus() async throws -> WorkspaceStatus
}

struct PrismCoreClientAdapter: PrismSessionClient {
  private let client: PrismCoreClient

  init(client: PrismCoreClient = PrismCoreClient()) {
    self.client = client
  }

  func openSession(at path: String) async throws -> any PrismSession {
    try await client.openSession(at: path)
  }
}

extension PrismCoreSession: PrismSession {}

#if DEBUG
  extension SessionStore {
    func injectPreviewState(phase: SessionPhase) {
      self.phase = phase
    }
  }
#endif
