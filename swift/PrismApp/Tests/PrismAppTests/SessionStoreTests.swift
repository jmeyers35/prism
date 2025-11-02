import XCTest
@testable import PrismApp
import PrismFFI

@MainActor
final class SessionStoreTests: XCTestCase {
  private func makeStore(client: MockSessionClient) -> SessionStore {
    let storage = InMemorySessionPersistence()
    return SessionStore(client: client, storage: storage)
  }

  func testOpenSessionSuccess() async throws {
    let session = MockSession(
      info: RepositoryInfo(root: "/tmp/prism", defaultBranch: "main"),
      status: WorkspaceStatus(currentBranch: "feature/prism", dirty: true)
    )

    let client = MockSessionClient(responses: [.success(session)])
    let store = makeStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/prism"))

    guard case let .ready(viewModel) = store.phase else {
      return XCTFail("Expected ready state")
    }

    XCTAssertEqual(viewModel.repositoryName, "prism")
    XCTAssertEqual(viewModel.defaultBranch, "main")
    XCTAssertEqual(viewModel.currentBranch, "feature/prism")
    XCTAssertTrue(viewModel.hasUncommittedChanges)
    XCTAssertTrue(store.hasActiveSession())

    guard case let .loaded(diffViewModel) = store.diffPhase else {
      return XCTFail("Expected loaded diff state after refresh")
    }

    XCTAssertTrue(diffViewModel.files.isEmpty)
    XCTAssertNil(store.selectedDiffFileID)

    XCTAssertEqual(store.attachModel?.selectedPluginID, "amp")
    XCTAssertNil(store.attachedPluginSession)

    await store.attachSelectedPlugin()
    XCTAssertNotNil(store.attachedPluginSession)
    XCTAssertTrue(store.isPluginAttached)
  }

  func testOpenSessionFailure() async {
    let client = MockSessionClient(responses: [.failure(MockError.notRepository)])
    let store = makeStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/invalid"))

    guard case let .failed(message) = store.phase else {
      return XCTFail("Expected failed state")
    }

    XCTAssertFalse(message.isEmpty)
    XCTAssertFalse(store.hasActiveSession())
    XCTAssertEqual(store.diffPhase, .idle)
  }

  func testRefreshActiveSessionUpdatesState() async throws {
    let session = MockSession(
      info: RepositoryInfo(root: "/tmp/prism", defaultBranch: "main"),
      status: WorkspaceStatus(currentBranch: "feature/initial", dirty: true)
    )

    let client = MockSessionClient(responses: [.success(session)])
    let store = makeStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/prism"))

    session.status = WorkspaceStatus(currentBranch: "feature/updated", dirty: false)
    let updatedDiff = MockSession.makeDiff(files: [MockSession.makeFile(path: "README.md", additions: UInt32(3), deletions: UInt32(1))])
    session.workspaceDiff = updatedDiff
    session.headDiff = updatedDiff

    await store.refreshActiveSession()

    guard case let .ready(viewModel) = store.phase else {
      return XCTFail("Expected ready state after refresh")
    }

    XCTAssertEqual(viewModel.currentBranch, "feature/updated")
    XCTAssertFalse(viewModel.hasUncommittedChanges)

    guard case let .loaded(diffViewModel) = store.diffPhase else {
      return XCTFail("Expected loaded diff state after refresh")
    }

    let paths = diffViewModel.files.map(\.path)
    XCTAssertEqual(paths, ["README.md"])
  }

  func testRefreshActiveSessionKeepsStateOnError() async throws {
    let session = MockSession(
      info: RepositoryInfo(root: "/tmp/prism", defaultBranch: "main"),
      status: WorkspaceStatus(currentBranch: "feature/initial", dirty: true)
    )

    var didReturnInitialStatus = false
    session.workspaceStatusHandler = {
      if didReturnInitialStatus {
        throw MockError.transient
      }
      didReturnInitialStatus = true
      return session.status
    }

    let client = MockSessionClient(responses: [.success(session)])
    let store = makeStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/prism"))

    guard case let .ready(originalViewModel) = store.phase else {
      return XCTFail("Expected ready state before refresh")
    }

    await store.refreshActiveSession()

    guard case let .ready(currentViewModel) = store.phase else {
      return XCTFail("Expected ready state after refresh failure")
    }

    XCTAssertEqual(currentViewModel, originalViewModel)
    XCTAssertTrue(store.hasActiveSession())
  }

  func testRefreshActiveSessionIgnoresStaleSessions() async throws {
    let sessionA = MockSession(
      info: RepositoryInfo(root: "/tmp/repoA", defaultBranch: "main"),
      status: WorkspaceStatus(currentBranch: "feature/a", dirty: false)
    )
    sessionA.repositoryInfoHandler = {
      try await Task.sleep(nanoseconds: 50_000_000)
      return sessionA.info
    }
    sessionA.workspaceStatusHandler = {
      try await Task.sleep(nanoseconds: 50_000_000)
      return sessionA.status
    }

    let sessionB = MockSession(
      info: RepositoryInfo(root: "/tmp/repoB", defaultBranch: "develop"),
      status: WorkspaceStatus(currentBranch: "feature/b", dirty: true)
    )

    let client = MockSessionClient(responses: [.success(sessionA), .success(sessionB)])
    let store = makeStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/repoA"))

    let refreshTask = Task {
      await store.refreshActiveSession()
    }

    try await Task.sleep(nanoseconds: 10_000_000)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/repoB"))

    await refreshTask.value

    guard case let .ready(viewModel) = store.phase else {
      return XCTFail("Expected ready state for repo B")
    }

    XCTAssertEqual(viewModel.repositoryName, "repoB")
    XCTAssertEqual(viewModel.repositoryPath, "/tmp/repoB")
    XCTAssertEqual(viewModel.currentBranch, "feature/b")
  }

  func testOpenSessionFailurePreservesExistingSession() async throws {
    let initialSession = MockSession(
      info: RepositoryInfo(root: "/tmp/prism", defaultBranch: "main"),
      status: WorkspaceStatus(currentBranch: "feature/initial", dirty: false)
    )

    let client = MockSessionClient(responses: [.success(initialSession)])
    let store = makeStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/prism"))

    guard case let .ready(originalViewModel) = store.phase else {
      return XCTFail("Expected ready state")
    }

    client.responses.append(.failure(MockError.notRepository))

    await store.openSession(at: URL(fileURLWithPath: "/tmp/other"))

    guard case let .ready(currentViewModel) = store.phase else {
      return XCTFail("Expected ready state to be preserved")
    }

    XCTAssertEqual(currentViewModel, originalViewModel)
    XCTAssertTrue(store.hasActiveSession())
  }

  func testReloadDiffHandlesErrors() async throws {
    let session = MockSession(
      info: RepositoryInfo(root: "/tmp/prism", defaultBranch: "main"),
      status: WorkspaceStatus(currentBranch: "feature/prism", dirty: false)
    )

    session.workspaceDiffResponses = [
      .success(MockSession.makeDiff(files: [MockSession.makeFile(path: "File.swift", additions: UInt32(2), deletions: UInt32(1))])),
      .failure(MockError.transient)
    ]
    session.headDiffResponses = [.failure(MockError.transient)]

    let client = MockSessionClient(responses: [.success(session)])
    let store = makeStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/prism"))

    guard case let .loaded(initialDiff) = store.diffPhase else {
      return XCTFail("Expected loaded diff state")
    }

    XCTAssertEqual(initialDiff.files.first?.path, "File.swift")

    await store.reloadDiff()

    guard case let .failed(message) = store.diffPhase else {
      return XCTFail("Expected failed diff state")
    }

    XCTAssertFalse(message.isEmpty)
  }

  func testLoadDiffFallsBackToHeadOnWorkspaceFailure() async throws {
    let session = MockSession(
      info: RepositoryInfo(root: "/tmp/prism", defaultBranch: "main"),
      status: WorkspaceStatus(currentBranch: "feature/prism", dirty: false)
    )

    session.workspaceDiffResponses = [.failure(MockError.transient)]
    session.headDiffResponses = [
      .success(MockSession.makeDiff(files: [MockSession.makeFile(path: "Fallback.swift", additions: UInt32(1), deletions: UInt32(0))]))
    ]

    let client = MockSessionClient(responses: [.success(session)])
    let store = makeStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/prism"))

    guard case let .loaded(diffViewModel) = store.diffPhase else {
      return XCTFail("Expected loaded diff state using fallback")
    }

    XCTAssertEqual(diffViewModel.files.map(\.path), ["Fallback.swift"])
  }
}

@MainActor
private final class InMemorySessionPersistence: SessionPersisting {
  private var sessions: [StoredSession] = []

  func fetchSessions() throws -> [StoredSession] {
    sessions.sorted { $0.lastOpened > $1.lastOpened }
  }

  @discardableResult
  func upsertSession(from viewModel: SessionViewModel, openedAt date: Date) throws -> StoredSession {
    if let index = sessions.firstIndex(where: { $0.repositoryPath == viewModel.repositoryPath }) {
      var updated = sessions[index]
      updated.repositoryName = viewModel.repositoryName
      updated.defaultBranch = viewModel.defaultBranch
      updated.currentBranch = viewModel.currentBranch
      updated.hasUncommittedChanges = viewModel.hasUncommittedChanges
      updated.lastOpened = date
      sessions[index] = updated
      return updated
    }

    let stored = StoredSession(
      repositoryName: viewModel.repositoryName,
      repositoryPath: viewModel.repositoryPath,
      defaultBranch: viewModel.defaultBranch,
      currentBranch: viewModel.currentBranch,
      hasUncommittedChanges: viewModel.hasUncommittedChanges,
      lastOpened: date
    )
    sessions.append(stored)
    return stored
  }

  func deleteSession(id: UUID) throws {
    sessions.removeAll { $0.id == id }
  }

  @discardableResult
  func addThread(for repositoryPath: String, pluginID: String, payload: SessionStorage.ThreadPayload) throws -> StoredSession.StoredThread {
    guard let index = sessions.firstIndex(where: { $0.repositoryPath == repositoryPath }) else {
      throw SessionStorageError.sessionNotFound
    }

    let thread = makeThread(from: payload, pluginID: pluginID)
    var session = sessions[index]
    var threads = session.threads
    threads.append(thread)
    sortThreads(&threads)
    session.threads = threads
    sessions[index] = session
    return thread
  }

  @discardableResult
  func replaceThreads(for repositoryPath: String, pluginID: String, threads: [SessionStorage.ThreadPayload]) throws -> [StoredSession.StoredThread] {
    guard let index = sessions.firstIndex(where: { $0.repositoryPath == repositoryPath }) else {
      throw SessionStorageError.sessionNotFound
    }

    var session = sessions[index]
    var updatedThreads = session.threads.filter { $0.pluginID != pluginID }
    let newThreads = threads.map { makeThread(from: $0, pluginID: pluginID) }
    updatedThreads.append(contentsOf: newThreads)
    sortThreads(&updatedThreads)
    session.threads = updatedThreads
    sessions[index] = session
    return newThreads
  }

  func threads(for repositoryPath: String, pluginID: String) throws -> [StoredSession.StoredThread] {
    guard let session = sessions.first(where: { $0.repositoryPath == repositoryPath }) else {
      return []
    }

    return session.threads.filter { $0.pluginID == pluginID }
  }

  private func makeThread(from payload: SessionStorage.ThreadPayload, pluginID: String) -> StoredSession.StoredThread {
    let comments = payload.comments.map { comment -> StoredSession.StoredThread.StoredComment in
      StoredSession.StoredThread.StoredComment(
        id: comment.id ?? UUID(),
        externalID: comment.externalID,
        authorName: comment.authorName,
        body: comment.body,
        createdAt: comment.createdAt,
        filePath: comment.filePath,
        lineNumber: comment.lineNumber,
        columnNumber: comment.columnNumber,
        diffSide: comment.diffSide
      )
    }

    return StoredSession.StoredThread(
      id: payload.id ?? UUID(),
      externalID: payload.externalID,
      pluginID: pluginID,
      title: payload.title,
      createdAt: payload.createdAt,
      lastUpdated: payload.lastUpdated ?? payload.createdAt,
      comments: comments
    )
  }

  private func sortThreads(_ threads: inout [StoredSession.StoredThread]) {
    threads.sort { lhs, rhs in
      let lhsDate = lhs.lastUpdated ?? lhs.createdAt ?? Date.distantPast
      let rhsDate = rhs.lastUpdated ?? rhs.createdAt ?? Date.distantPast
      return lhsDate > rhsDate
    }
  }
}

private enum MockError: Error {
  case notRepository
  case noResponse
  case transient
}

private final class MockSessionClient: PrismSessionClient {
  var responses: [Result<any PrismSession, Error>]

  init(responses: [Result<any PrismSession, Error>]) {
    self.responses = responses
  }

  func openSession(at path: String) async throws -> any PrismSession {
    guard !responses.isEmpty else {
      throw MockError.noResponse
    }

    let response = responses.removeFirst()

    switch response {
    case let .success(session):
      return session
    case let .failure(error):
      throw error
    }
  }
}

private final class MockSession: PrismSession {
  var info: RepositoryInfo
  var status: WorkspaceStatus
  var workspaceDiff: Diff
  var headDiff: Diff
  var repositoryInfoHandler: (() async throws -> RepositoryInfo)?
  var workspaceStatusHandler: (() async throws -> WorkspaceStatus)?
  var workspaceDiffResponses: [Result<Diff, Error>] = []
  var headDiffResponses: [Result<Diff, Error>] = []
  var pluginSummaries: [PluginSummary]
  var pluginThreadsByPluginID: [String: [ThreadRef]] = [:]
  var attachPluginResponses: [Result<PluginSession, Error>] = []

  init(info: RepositoryInfo, status: WorkspaceStatus, diff: Diff = MockSession.makeDiff()) {
    self.info = info
    self.status = status
    self.workspaceDiff = diff
    self.headDiff = diff
    self.pluginSummaries = [
      PluginSummary(
        id: "amp",
        label: "Mock Amp",
        capabilities: PluginCapabilities(
          supportsListThreads: false,
          supportsAttachWithoutThread: true,
          supportsPolling: false
        )
      )
    ]
  }

  func repositoryInfo() async throws -> RepositoryInfo {
    if let handler = repositoryInfoHandler {
      return try await handler()
    }
    return info
  }

  func workspaceStatus() async throws -> WorkspaceStatus {
    if let handler = workspaceStatusHandler {
      return try await handler()
    }
    return status
  }

  func diffWorkspace() async throws -> Diff {
    if !workspaceDiffResponses.isEmpty {
      let response = workspaceDiffResponses.removeFirst()
      switch response {
      case let .success(diff):
        self.workspaceDiff = diff
        return diff
      case let .failure(error):
        throw error
      }
    }
    return workspaceDiff
  }

  func diffHead() async throws -> Diff {
    if !headDiffResponses.isEmpty {
      let response = headDiffResponses.removeFirst()
      switch response {
      case let .success(diff):
        self.headDiff = diff
        return diff
      case let .failure(error):
        throw error
      }
    }
    return headDiff
  }

  func plugins() async throws -> [PluginSummary] {
    pluginSummaries
  }

  func pluginThreads(pluginId: String) async throws -> [ThreadRef] {
    pluginThreadsByPluginID[pluginId] ?? []
  }

  func attachPlugin(pluginId: String, threadId: String?) async throws -> PluginSession {
    if !attachPluginResponses.isEmpty {
      let response = attachPluginResponses.removeFirst()
      switch response {
      case let .success(session):
        return session
      case let .failure(error):
        throw error
      }
    }

    let thread = ThreadRef(id: threadId ?? "mock-thread-\(pluginId)", title: nil)
    return PluginSession(pluginId: pluginId, sessionId: "mock-session-\(pluginId)", thread: thread)
  }

  static func makeDiff(files: [DiffFile] = []) -> Diff {
    Diff(
      range: RevisionRange(
        base: Revision(oid: "BASE", reference: nil, summary: nil, author: nil, committer: nil, timestamp: nil),
        head: Revision(oid: "HEAD", reference: nil, summary: nil, author: nil, committer: nil, timestamp: nil)
      ),
      files: files
    )
  }

  static func makeFile(path: String, additions: UInt32, deletions: UInt32) -> DiffFile {
    DiffFile(
      path: path,
      oldPath: nil,
      status: .modified,
      stats: DiffStats(additions: additions, deletions: deletions),
      isBinary: false,
      hunks: []
    )
  }
}
