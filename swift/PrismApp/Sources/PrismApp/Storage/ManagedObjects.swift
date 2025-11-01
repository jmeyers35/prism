import CoreData

@objc(SessionEntity)
final class SessionEntity: NSManagedObject {
  @NSManaged var id: UUID
  @NSManaged var repositoryName: String
  @NSManaged var repositoryPath: String
  @NSManaged var defaultBranch: String?
  @NSManaged var currentBranch: String?
  @NSManaged var hasUncommittedChanges: Bool
  @NSManaged var lastOpened: Date
  @NSManaged var threads: NSSet?
}

extension SessionEntity {
  @nonobjc class func fetchRequest() -> NSFetchRequest<SessionEntity> {
    NSFetchRequest<SessionEntity>(entityName: "SessionEntity")
  }

  var threadSet: Set<ThreadEntity> {
    (threads as? Set<ThreadEntity>) ?? []
  }
}

@objc(ThreadEntity)
final class ThreadEntity: NSManagedObject {
  @NSManaged var id: UUID
  @NSManaged var externalID: String?
  @NSManaged var pluginID: String
  @NSManaged var title: String?
  @NSManaged var createdAt: Date?
  @NSManaged var lastUpdated: Date?
  @NSManaged var comments: NSSet?
  @NSManaged var session: SessionEntity
}

extension ThreadEntity {
  @nonobjc class func fetchRequest() -> NSFetchRequest<ThreadEntity> {
    NSFetchRequest<ThreadEntity>(entityName: "ThreadEntity")
  }

  var commentSet: Set<CommentEntity> {
    (comments as? Set<CommentEntity>) ?? []
  }
}

@objc(CommentEntity)
final class CommentEntity: NSManagedObject {
  @NSManaged var id: UUID
  @NSManaged var externalID: String?
  @NSManaged var authorName: String?
  @NSManaged var body: String
  @NSManaged var createdAt: Date?
  @NSManaged var filePath: String?
  @NSManaged var lineNumber: NSNumber?
  @NSManaged var columnNumber: NSNumber?
  @NSManaged var diffSide: String?
  @NSManaged var thread: ThreadEntity
}

extension CommentEntity {
  @nonobjc class func fetchRequest() -> NSFetchRequest<CommentEntity> {
    NSFetchRequest<CommentEntity>(entityName: "CommentEntity")
  }
}
