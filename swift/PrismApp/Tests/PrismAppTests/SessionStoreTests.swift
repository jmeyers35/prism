import XCTest
@testable import PrismApp
import PrismFFI

@MainActor
final class SessionStoreTests: XCTestCase {
  func testOpenSessionSuccess() async throws {
    let session = MockSession(
      info: RepositoryInfo(root: "/tmp/prism", defaultBranch: "main"),
      status: WorkspaceStatus(currentBranch: "feature/prism", dirty: true)
    )

    let client = MockSessionClient(responses: [.success(session)])
    let store = SessionStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/prism"))

    guard case let .ready(viewModel) = store.phase else {
      return XCTFail("Expected ready state")
    }

    XCTAssertEqual(viewModel.repositoryName, "prism")
    XCTAssertEqual(viewModel.defaultBranch, "main")
    XCTAssertEqual(viewModel.currentBranch, "feature/prism")
    XCTAssertTrue(viewModel.hasUncommittedChanges)
    XCTAssertTrue(store.hasActiveSession())
  }

  func testOpenSessionFailure() async {
    let client = MockSessionClient(responses: [.failure(MockError.notRepository)])
    let store = SessionStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/invalid"))

    guard case let .failed(message) = store.phase else {
      return XCTFail("Expected failed state")
    }

    XCTAssertFalse(message.isEmpty)
    XCTAssertFalse(store.hasActiveSession())
  }

  func testRefreshActiveSessionUpdatesState() async throws {
    let session = MockSession(
      info: RepositoryInfo(root: "/tmp/prism", defaultBranch: "main"),
      status: WorkspaceStatus(currentBranch: "feature/initial", dirty: true)
    )

    let client = MockSessionClient(responses: [.success(session)])
    let store = SessionStore(client: client)

    await store.openSession(at: URL(fileURLWithPath: "/tmp/prism"))

    session.status = WorkspaceStatus(currentBranch: "feature/updated", dirty: false)

    await store.refreshActiveSession()

    guard case let .ready(viewModel) = store.phase else {
      return XCTFail("Expected ready state after refresh")
    }

    XCTAssertEqual(viewModel.currentBranch, "feature/updated")
    XCTAssertFalse(viewModel.hasUncommittedChanges)
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
    let store = SessionStore(client: client)

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
    let store = SessionStore(client: client)

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
    let store = SessionStore(client: client)

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
  var repositoryInfoHandler: (() async throws -> RepositoryInfo)?
  var workspaceStatusHandler: (() async throws -> WorkspaceStatus)?

  init(info: RepositoryInfo, status: WorkspaceStatus) {
    self.info = info
    self.status = status
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
}
