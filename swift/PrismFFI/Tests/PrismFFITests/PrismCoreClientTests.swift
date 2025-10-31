import XCTest
@testable import PrismFFI

final class PrismCoreClientTests: XCTestCase {
    func testOpenInvalidRepositoryThrows() async {
        let client = PrismCoreClient()
        let tempURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("prism-invalid-\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: tempURL, withIntermediateDirectories: true)
        addTeardownBlock {
            try? FileManager.default.removeItem(at: tempURL)
        }

        do {
            _ = try await client.openSession(at: tempURL)
            XCTFail("Expected opening an invalid repository to throw")
        } catch {
            guard case let PrismCoreError.notARepository(message: message) = error else {
                XCTFail("Expected notARepository error, received \(error)")
                return
            }
            XCTAssertFalse(message.isEmpty)
        }
    }

    func testSnapshotFromInitializedRepository() async throws {
        let repoURL = try makeTemporaryDirectory()
        addTeardownBlock {
            try? FileManager.default.removeItem(at: repoURL)
        }

        try git(["init"], at: repoURL)
        try git(["config", "user.email", "tester@example.com"], at: repoURL)
        try git(["config", "user.name", "Tester"], at: repoURL)

        let readmeURL = repoURL.appendingPathComponent("README.md")
        try "hello\n".write(to: readmeURL, atomically: true, encoding: .utf8)

        try git(["add", "."], at: repoURL)
        try git(["commit", "-m", "Initial commit"], at: repoURL)

        let client = PrismCoreClient()
        let session = try await client.openSession(at: repoURL)

        let snapshot = try await session.snapshot()

        XCTAssertEqual(
            URL(fileURLWithPath: snapshot.info.root).standardizedFileURL.path,
            repoURL.standardizedFileURL.path
        )
        XCTAssertNotNil(snapshot.workspace.currentBranch)
        XCTAssertFalse(snapshot.workspace.dirty)
        XCTAssertEqual(snapshot.revisions?.head.summary, "Initial commit")
    }

    func testWorkspaceDiffLifecycle() async throws {
        let repoURL = try makeTemporaryDirectory()
        addTeardownBlock {
            try? FileManager.default.removeItem(at: repoURL)
        }

        try git(["init"], at: repoURL)
        try git(["config", "user.email", "tester@example.com"], at: repoURL)
        try git(["config", "user.name", "Tester"], at: repoURL)

        let readmeURL = repoURL.appendingPathComponent("README.md")
        try "hello\n".write(to: readmeURL, atomically: true, encoding: .utf8)

        try git(["add", "."], at: repoURL)
        try git(["commit", "-m", "Initial commit"], at: repoURL)

        let client = PrismCoreClient()
        let session = try await client.openSession(at: repoURL)

        var status = try await session.workspaceStatus()
        XCTAssertFalse(status.dirty)

        try "hello\nsecond line\n".write(to: readmeURL, atomically: true, encoding: .utf8)

        status = try await session.workspaceStatus()
        XCTAssertTrue(status.dirty)

        let dirtyDiff = try await session.diffHead()
        XCTAssertEqual(dirtyDiff.files.count, 1)
        let dirtyFile = try XCTUnwrap(dirtyDiff.files.first)
        XCTAssertEqual(dirtyFile.path, "README.md")
        XCTAssertTrue([FileStatus.modified, .added].contains(dirtyFile.status))
        XCTAssertFalse(dirtyFile.hunks.isEmpty)

        try git(["add", "README.md"], at: repoURL)
        try git(["commit", "-m", "Second commit"], at: repoURL)

        let snapshot = try await session.refresh()
        XCTAssertFalse(snapshot.workspace.dirty)
        XCTAssertEqual(snapshot.revisions?.head.summary, "Second commit")

        status = try await session.workspaceStatus()
        XCTAssertFalse(status.dirty)

        let headRevision = try await session.headRevision()
        let head = try XCTUnwrap(headRevision)
        let base = try await session.baseRevision()
        let range = RevisionRange(base: base, head: head)
        let rangeDiff = try await session.diffForRange(range)

        XCTAssertEqual(rangeDiff.files.count, 1)
        let rangeFile = try XCTUnwrap(rangeDiff.files.first)
        XCTAssertEqual(rangeFile.path, "README.md")
        XCTAssertEqual(rangeFile.status, .modified)
    }

    // MARK: - Helpers

    private func makeTemporaryDirectory() throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("prism-core-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func git(_ arguments: [String], at url: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git"] + arguments
        process.currentDirectoryURL = url

        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe

        try process.run()
        process.waitUntilExit()

        if process.terminationStatus != 0 {
            let outputData = pipe.fileHandleForReading.readDataToEndOfFile()
            let output = String(data: outputData, encoding: .utf8) ?? ""
            XCTFail("git \(arguments.joined(separator: " ")) failed: \(output)")
        }
    }
}
