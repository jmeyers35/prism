import CoreData
import Foundation

struct StoredSession: Identifiable, Equatable {
  struct StoredThread: Identifiable, Equatable {
    struct StoredComment: Identifiable, Equatable {
      var id: UUID
      var externalID: String?
      var authorName: String?
      var body: String
      var createdAt: Date?
      var filePath: String?
      var lineNumber: Int?
      var columnNumber: Int?
      var diffSide: String?

      init(
        id: UUID = UUID(),
        externalID: String? = nil,
        authorName: String? = nil,
        body: String,
        createdAt: Date? = nil,
        filePath: String? = nil,
        lineNumber: Int? = nil,
        columnNumber: Int? = nil,
        diffSide: String? = nil
      ) {
        self.id = id
        self.externalID = externalID
        self.authorName = authorName
        self.body = body
        self.createdAt = createdAt
        self.filePath = filePath
        self.lineNumber = lineNumber
        self.columnNumber = columnNumber
        self.diffSide = diffSide
      }

      init(entity: CommentEntity) {
        id = entity.id
        externalID = entity.externalID
        authorName = entity.authorName
        body = entity.body
        createdAt = entity.createdAt
        filePath = entity.filePath
        lineNumber = entity.lineNumber?.intValue
        columnNumber = entity.columnNumber?.intValue
        diffSide = entity.diffSide
      }
    }

    var id: UUID
    var externalID: String?
    var pluginID: String
    var title: String?
    var createdAt: Date?
    var lastUpdated: Date?
    var comments: [StoredComment]

    init(
      id: UUID = UUID(),
      externalID: String? = nil,
      pluginID: String,
      title: String? = nil,
      createdAt: Date? = nil,
      lastUpdated: Date? = nil,
      comments: [StoredComment] = []
    ) {
      self.id = id
      self.externalID = externalID
      self.pluginID = pluginID
      self.title = title
      self.createdAt = createdAt
      self.lastUpdated = lastUpdated
      self.comments = comments
    }

    init(entity: ThreadEntity) {
      id = entity.id
      externalID = entity.externalID
      pluginID = entity.pluginID
      title = entity.title
      createdAt = entity.createdAt
      lastUpdated = entity.lastUpdated
      comments = entity.commentSet
        .map(StoredComment.init)
        .sorted { lhs, rhs in
          (lhs.createdAt ?? Date.distantPast) < (rhs.createdAt ?? Date.distantPast)
        }
    }
  }

  var id: UUID
  var repositoryName: String
  var repositoryPath: String
  var defaultBranch: String?
  var currentBranch: String?
  var hasUncommittedChanges: Bool
  var lastOpened: Date
  var threads: [StoredThread]

  init(
    id: UUID = UUID(),
    repositoryName: String,
    repositoryPath: String,
    defaultBranch: String?,
    currentBranch: String?,
    hasUncommittedChanges: Bool,
    lastOpened: Date,
    threads: [StoredThread] = []
  ) {
    self.id = id
    self.repositoryName = repositoryName
    self.repositoryPath = repositoryPath
    self.defaultBranch = defaultBranch
    self.currentBranch = currentBranch
    self.hasUncommittedChanges = hasUncommittedChanges
    self.lastOpened = lastOpened
    self.threads = threads.sorted { lhs, rhs in
      let lhsDate = lhs.lastUpdated ?? lhs.createdAt ?? Date.distantPast
      let rhsDate = rhs.lastUpdated ?? rhs.createdAt ?? Date.distantPast
      return lhsDate > rhsDate
    }
  }

  init(entity: SessionEntity) {
    id = entity.id
    repositoryName = entity.repositoryName
    repositoryPath = entity.repositoryPath
    defaultBranch = entity.defaultBranch
    currentBranch = entity.currentBranch
    hasUncommittedChanges = entity.hasUncommittedChanges
    lastOpened = entity.lastOpened
    threads = entity.threadSet
      .map(StoredThread.init)
      .sorted { lhs, rhs in
        let lhsDate = lhs.lastUpdated ?? lhs.createdAt ?? Date.distantPast
        let rhsDate = rhs.lastUpdated ?? rhs.createdAt ?? Date.distantPast
        return lhsDate > rhsDate
      }
  }
}

enum SessionStorageError: Error {
  case sessionNotFound
}

@MainActor
protocol SessionPersisting {
  func fetchSessions() throws -> [StoredSession]
  @discardableResult
  func upsertSession(from viewModel: SessionViewModel, openedAt date: Date) throws -> StoredSession
  func deleteSession(id: UUID) throws
  @discardableResult
  func addThread(for repositoryPath: String, pluginID: String, payload: SessionStorage.ThreadPayload) throws -> StoredSession.StoredThread
}

@MainActor
final class SessionStorage: SessionPersisting {
  struct ThreadPayload {
    var id: UUID?
    var externalID: String?
    var title: String?
    var createdAt: Date?
    var lastUpdated: Date?
    var comments: [CommentPayload]
  }

  struct CommentPayload {
    var id: UUID?
    var externalID: String?
    var authorName: String?
    var body: String
    var createdAt: Date?
    var filePath: String?
    var lineNumber: Int?
    var columnNumber: Int?
    var diffSide: String?
  }

  private let controller: PersistenceController

  private var context: NSManagedObjectContext {
    controller.container.viewContext
  }

  init(controller: PersistenceController? = nil) {
    self.controller = controller ?? PersistenceController.shared
  }

  func fetchSessions() throws -> [StoredSession] {
    let request = SessionEntity.fetchRequest()
    request.sortDescriptors = [
      NSSortDescriptor(key: #keyPath(SessionEntity.lastOpened), ascending: false)
    ]
    let entities = try context.fetch(request)
    return entities.map(StoredSession.init)
  }

  @discardableResult
  func upsertSession(from viewModel: SessionViewModel, openedAt date: Date = Date()) throws -> StoredSession {
    let session = try findOrCreateSession(repositoryPath: viewModel.repositoryPath)
    session.repositoryName = viewModel.repositoryName
    session.repositoryPath = viewModel.repositoryPath
    session.defaultBranch = viewModel.defaultBranch
    session.currentBranch = viewModel.currentBranch
    session.hasUncommittedChanges = viewModel.hasUncommittedChanges
    session.lastOpened = date

    try saveIfNeeded()
    return StoredSession(entity: session)
  }

  func deleteSession(id: UUID) throws {
    let request = SessionEntity.fetchRequest()
    request.predicate = NSPredicate(format: "id == %@", id as CVarArg)
    request.fetchLimit = 1
    if let entity = try context.fetch(request).first {
      context.delete(entity)
      try saveIfNeeded()
    }
  }

  @discardableResult
  func replaceThreads(for repositoryPath: String, pluginID: String, threads: [ThreadPayload]) throws -> [StoredSession.StoredThread] {
    guard let session = try findSession(repositoryPath: repositoryPath) else {
      throw SessionStorageError.sessionNotFound
    }

    session.threadSet
      .filter { $0.pluginID == pluginID }
      .forEach { context.delete($0) }

    let storedThreads = threads.map { payload -> StoredSession.StoredThread in
      let thread = ThreadEntity(context: context)
      thread.id = payload.id ?? UUID()
      thread.externalID = payload.externalID
      thread.pluginID = pluginID
      thread.title = payload.title
      thread.createdAt = payload.createdAt
      thread.lastUpdated = payload.lastUpdated ?? payload.createdAt
      thread.session = session

      payload.comments.forEach { commentPayload in
        let comment = CommentEntity(context: context)
        comment.id = commentPayload.id ?? UUID()
        comment.externalID = commentPayload.externalID
        comment.authorName = commentPayload.authorName
        comment.body = commentPayload.body
        comment.createdAt = commentPayload.createdAt
        comment.filePath = commentPayload.filePath
        if let line = commentPayload.lineNumber {
          comment.lineNumber = NSNumber(value: line)
        } else {
          comment.lineNumber = nil
        }
        if let column = commentPayload.columnNumber {
          comment.columnNumber = NSNumber(value: column)
        } else {
          comment.columnNumber = nil
        }
        comment.diffSide = commentPayload.diffSide
        comment.thread = thread
      }

      return StoredSession.StoredThread(entity: thread)
    }

    try saveIfNeeded()
    return storedThreads.sorted { lhs, rhs in
      let lhsDate = lhs.lastUpdated ?? lhs.createdAt ?? Date.distantPast
      let rhsDate = rhs.lastUpdated ?? rhs.createdAt ?? Date.distantPast
      return lhsDate > rhsDate
    }
  }

  func threads(for repositoryPath: String, pluginID: String) throws -> [StoredSession.StoredThread] {
    guard let session = try findSession(repositoryPath: repositoryPath) else {
      return []
    }

    return session.threadSet
      .filter { $0.pluginID == pluginID }
      .map(StoredSession.StoredThread.init)
      .sorted { lhs, rhs in
        let lhsDate = lhs.lastUpdated ?? lhs.createdAt ?? Date.distantPast
        let rhsDate = rhs.lastUpdated ?? rhs.createdAt ?? Date.distantPast
        return lhsDate > rhsDate
      }
  }

  @discardableResult
  func addThread(for repositoryPath: String, pluginID: String, payload: ThreadPayload) throws -> StoredSession.StoredThread {
    guard let session = try findSession(repositoryPath: repositoryPath) else {
      throw SessionStorageError.sessionNotFound
    }

    let thread = ThreadEntity(context: context)
    thread.id = payload.id ?? UUID()
    thread.externalID = payload.externalID
    thread.pluginID = pluginID
    thread.title = payload.title
    thread.createdAt = payload.createdAt ?? Date()
    thread.lastUpdated = payload.lastUpdated ?? payload.createdAt ?? Date()
    thread.session = session

    payload.comments.forEach { commentPayload in
      let comment = CommentEntity(context: context)
      comment.id = commentPayload.id ?? UUID()
      comment.externalID = commentPayload.externalID
      comment.authorName = commentPayload.authorName
      comment.body = commentPayload.body
      comment.createdAt = commentPayload.createdAt ?? Date()
      comment.filePath = commentPayload.filePath
      comment.lineNumber = commentPayload.lineNumber.map { NSNumber(value: $0) }
      comment.columnNumber = commentPayload.columnNumber.map { NSNumber(value: $0) }
      comment.diffSide = commentPayload.diffSide
      comment.thread = thread
    }

    try saveIfNeeded()
    return StoredSession.StoredThread(entity: thread)
  }

  private func findSession(repositoryPath: String) throws -> SessionEntity? {
    let request = SessionEntity.fetchRequest()
    request.predicate = NSPredicate(format: "repositoryPath == %@", repositoryPath)
    request.fetchLimit = 1
    return try context.fetch(request).first
  }

  private func findOrCreateSession(repositoryPath: String) throws -> SessionEntity {
    if let existing = try findSession(repositoryPath: repositoryPath) {
      return existing
    }

    let session = SessionEntity(context: context)
    session.id = UUID()
    session.repositoryPath = repositoryPath
    session.repositoryName = repositoryPath.components(separatedBy: "/").last ?? repositoryPath
    session.defaultBranch = nil
    session.currentBranch = nil
    session.hasUncommittedChanges = false
    session.lastOpened = Date()
    return session
  }

  private func saveIfNeeded() throws {
    if context.hasChanges {
      try context.save()
    }
  }
}
