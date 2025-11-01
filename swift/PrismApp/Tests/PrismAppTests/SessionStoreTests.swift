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

  init(info: RepositoryInfo, status: WorkspaceStatus, diff: Diff = MockSession.makeDiff()) {
    self.info = info
    self.status = status
    self.workspaceDiff = diff
    self.headDiff = diff
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
