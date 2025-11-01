import CoreData
import Foundation

@MainActor
final class PersistenceController {
  static let shared = PersistenceController()

  static let preview: PersistenceController = {
    let controller = PersistenceController(inMemory: true)
    let context = controller.container.viewContext

    let session = SessionEntity(context: context)
    session.id = UUID()
    session.repositoryName = "Example Repo"
    session.repositoryPath = "/Users/example/dev/prism"
    session.defaultBranch = "main"
    session.currentBranch = "feature/preview"
    session.hasUncommittedChanges = true
    session.lastOpened = Date()

    let thread = ThreadEntity(context: context)
    thread.id = UUID()
    thread.externalID = "thread-preview"
    thread.pluginID = "amp"
    thread.title = "Preview Thread"
    thread.createdAt = Date()
    thread.lastUpdated = Date()
    thread.session = session

    let comment = CommentEntity(context: context)
    comment.id = UUID()
    comment.externalID = "comment-preview"
    comment.authorName = "Prism Bot"
    comment.body = "This is a preview comment stored in Core Data."
    comment.createdAt = Date()
    comment.filePath = "Sources/Example.swift"
    comment.lineNumber = NSNumber(value: 42)
    comment.columnNumber = NSNumber(value: 3)
    comment.diffSide = "head"
    comment.thread = thread

    do {
      try context.save()
    } catch {
      assertionFailure("Failed to populate preview storage: \(error)")
    }

    return controller
  }()

  private static let managedObjectModel: NSManagedObjectModel = {
    guard let model = NSManagedObjectModel.mergedModel(from: [Bundle.module]) else {
      fatalError("Unable to load PrismStorage Core Data model from bundle \(Bundle.module.bundlePath)")
    }
    return model
  }()

  let container: NSPersistentContainer

  init(inMemory: Bool = false) {
    container = NSPersistentContainer(name: "PrismStorage", managedObjectModel: Self.managedObjectModel)

    if inMemory {
      let description = NSPersistentStoreDescription()
      description.type = NSInMemoryStoreType
      description.url = URL(fileURLWithPath: "/dev/null")
      container.persistentStoreDescriptions = [description]
    }

    container.loadPersistentStores { _, error in
      if let error = error as NSError? {
        fatalError("Unresolved error \(error), \(error.userInfo)")
      }
    }

    container.viewContext.mergePolicy = NSMergeByPropertyObjectTrumpMergePolicy
    container.viewContext.automaticallyMergesChangesFromParent = true
  }
}
