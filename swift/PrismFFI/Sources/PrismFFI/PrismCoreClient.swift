import Foundation

public enum PrismCoreError: Error, Equatable {
    case notARepository(message: String)
    case bareRepository(message: String)
    case missingHeadRevision(message: String)
    case git(message: String)
    case io(message: String)
    case unimplemented(message: String)
    case internalError(message: String)
    case pluginNotRegistered(message: String)
    case plugin(message: String)

    init(coreError: CoreError) {
        switch coreError {
        case let .NotARepository(message):
            self = .notARepository(message: message)
        case let .BareRepository(message):
            self = .bareRepository(message: message)
        case let .MissingHeadRevision(message):
            self = .missingHeadRevision(message: message)
        case let .Git(message):
            self = .git(message: message)
        case let .Io(message):
            self = .io(message: message)
        case let .Unimplemented(message):
            self = .unimplemented(message: message)
        case let .Internal(message):
            self = .internalError(message: message)
        case let .PluginNotRegistered(message):
            self = .pluginNotRegistered(message: message)
        case let .Plugin(message):
            self = .plugin(message: message)
        }
    }
}

public final class PrismCoreClient {
    private let queue: DispatchQueue

    public init(queue: DispatchQueue = DispatchQueue(label: "app.prism.core")) {
        self.queue = queue
    }

    public func openSession(at path: String) async throws -> PrismCoreSession {
        try await perform {
            let session = try open(path: path)
            return PrismCoreSession(coreSession: session, queue: self.queue)
        }
    }

    public func openSession(at url: URL) async throws -> PrismCoreSession {
        try await openSession(at: url.path)
    }

    private func perform<T>(_ work: @escaping () throws -> T) async throws -> T {
        let queue = self.queue
        return try await withCheckedThrowingContinuation { continuation in
            queue.async {
                do {
                    let value = try work()
                    continuation.resume(returning: value)
                } catch let coreError as CoreError {
                    continuation.resume(throwing: PrismCoreError(coreError: coreError))
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }
}

public final class PrismCoreSession {
    private let coreSession: CoreSession
    private let queue: DispatchQueue

    init(coreSession: CoreSession, queue: DispatchQueue) {
        self.coreSession = coreSession
        self.queue = queue
    }

    public func repositoryInfo() async throws -> RepositoryInfo {
        try await call { try self.coreSession.repositoryInfo() }
    }

    public func workspaceStatus() async throws -> WorkspaceStatus {
        try await call { try self.coreSession.workspaceStatus() }
    }

    public func headRevision() async throws -> Revision? {
        try await call { try self.coreSession.headRevision() }
    }

    public func baseRevision() async throws -> Revision? {
        try await call { try self.coreSession.baseRevision() }
    }

    public func snapshot() async throws -> RepositorySnapshot {
        try await call { try self.coreSession.snapshot() }
    }

    public func refresh() async throws -> RepositorySnapshot {
        try await call { try self.coreSession.refresh() }
    }

    public func diffHead() async throws -> Diff {
        try await call { try self.coreSession.diffHead() }
    }

    public func diffWorkspace() async throws -> Diff {
        try await call { try self.coreSession.diffWorkspace() }
    }

    public func diffForRange(_ range: RevisionRange) async throws -> Diff {
        try await call { try self.coreSession.diffForRange(range: range) }
    }

    private func call<T>(_ work: @escaping () throws -> T) async throws -> T {
        let queue = self.queue
        return try await withCheckedThrowingContinuation { continuation in
            queue.async {
                do {
                    let value = try work()
                    continuation.resume(returning: value)
                } catch let coreError as CoreError {
                    continuation.resume(throwing: PrismCoreError(coreError: coreError))
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }
}
