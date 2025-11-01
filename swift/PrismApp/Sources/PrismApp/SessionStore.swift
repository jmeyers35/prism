import AppKit
import Combine
import Foundation
import PrismFFI

@MainActor
final class SessionStore: ObservableObject {
  @Published private(set) var phase: SessionPhase = .idle
  @Published private(set) var diffPhase: DiffPhase = .idle
  @Published var selectedDiffFileID: DiffFileViewModel.ID?
  @Published private(set) var storedSessions: [StoredSession] = []

  private let client: any PrismSessionClient
  private let storage: any SessionPersisting
  private var activeSession: (any PrismSession)?
  private var loadIdentifier: UUID?
  private var diffLoadIdentifier: UUID?

  init(
    client: any PrismSessionClient = PrismCoreClientAdapter(),
    storage: (any SessionPersisting)? = nil
  ) {
    self.client = client
    let storageInstance = storage ?? SessionStorage()
    self.storage = storageInstance
    self.storedSessions = (try? storageInstance.fetchSessions()) ?? []
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
    let previousDiffPhase = diffPhase
    let previousSelection = selectedDiffFileID
    let ticket = UUID()
    loadIdentifier = ticket
    diffLoadIdentifier = nil
    phase = .loading(url)
    diffPhase = .idle
    selectedDiffFileID = nil

    do {
      let session = try await client.openSession(at: url.path)
      let info = try await session.repositoryInfo()
      let status = try await session.workspaceStatus()

      guard loadIdentifier == ticket else { return }

      activeSession = session
      let viewModel = makeViewModel(info: info, status: status)
      phase = .ready(viewModel)
      persistSession(viewModel)
      let sessionID = ObjectIdentifier(session as AnyObject)
      await loadDiff(for: session, sessionID: sessionID, preferredSelection: nil)
      guard loadIdentifier == ticket else { return }
      loadIdentifier = nil
    } catch {
      guard loadIdentifier == ticket else { return }
      loadIdentifier = nil

      if let previousSession {
        activeSession = previousSession
        phase = previousPhase
        diffPhase = previousDiffPhase
        selectedDiffFileID = previousSelection
      } else {
        activeSession = nil
        phase = .failed(Self.describe(error))
        diffPhase = .idle
        selectedDiffFileID = nil
      }
    }
  }

  func closeSession() {
    loadIdentifier = nil
    activeSession = nil
    phase = .idle
    diffPhase = .idle
    selectedDiffFileID = nil
    diffLoadIdentifier = nil
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

      let viewModel = makeViewModel(info: info, status: status)
      phase = .ready(viewModel)
      persistSession(viewModel)
      await loadDiff(for: session, sessionID: expectedSessionID, preferredSelection: selectedDiffFileID)
    } catch {
      guard loadIdentifier == expectedLoadIdentifier else { return }
      guard let activeSession = activeSession else { return }
      guard ObjectIdentifier(activeSession as AnyObject) == expectedSessionID else { return }
      // Preserve the current UI state; transient refresh failures should not eject the user.
      return
    }
  }

  func reloadDiff() async {
    guard let session = activeSession else { return }
    let sessionID = ObjectIdentifier(session as AnyObject)
    await loadDiff(for: session, sessionID: sessionID, preferredSelection: selectedDiffFileID)
  }

  func hasActiveSession() -> Bool {
    activeSession != nil
  }

  func removeStoredSession(id: UUID) {
    do {
      try storage.deleteSession(id: id)
      storedSessions = try storage.fetchSessions()
    } catch {
      NSLog("Failed to delete stored session: \(error)")
      assertionFailure("Failed to delete stored session: \(error)")
    }
  }

  private func loadDiff(
    for session: any PrismSession,
    sessionID: ObjectIdentifier,
    preferredSelection: DiffFileViewModel.ID?
  ) async {
    let ticket = UUID()
    diffLoadIdentifier = ticket
    diffPhase = .loading

    do {
      let diff: Diff
      do {
        diff = try await session.diffWorkspace()
      } catch {
        let workspaceError = error
        do {
          diff = try await session.diffHead()
        } catch {
          throw workspaceError
        }
      }
      guard diffLoadIdentifier == ticket else { return }
      guard let currentSession = activeSession else { return }
      guard ObjectIdentifier(currentSession as AnyObject) == sessionID else { return }

      let viewModel = DiffBrowserViewModel(diff: diff)
      diffPhase = .loaded(viewModel)

      if let preferredSelection,
         viewModel.files.contains(where: { $0.id == preferredSelection }) {
        selectedDiffFileID = preferredSelection
      } else {
        selectedDiffFileID = viewModel.files.first?.id
      }

      diffLoadIdentifier = nil
    } catch {
      guard diffLoadIdentifier == ticket else { return }
      guard let currentSession = activeSession else { return }
      guard ObjectIdentifier(currentSession as AnyObject) == sessionID else { return }

      diffPhase = .failed(Self.describe(error))
      selectedDiffFileID = nil
      diffLoadIdentifier = nil
    }
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
        let .internalError(message),
        let .pluginNotRegistered(message),
        let .plugin(message):
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

  private func persistSession(_ viewModel: SessionViewModel, openedAt date: Date = Date()) {
    do {
      _ = try storage.upsertSession(from: viewModel, openedAt: date)
      storedSessions = try storage.fetchSessions()
    } catch {
      NSLog("Failed to persist session: \(error)")
      assertionFailure("Failed to persist session: \(error)")
    }
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
  func diffWorkspace() async throws -> Diff
  func diffHead() async throws -> Diff
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
