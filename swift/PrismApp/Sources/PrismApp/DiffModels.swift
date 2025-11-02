import Foundation
import PrismFFI

enum DiffPhase: Equatable {
  case idle
  case loading
  case loaded(DiffBrowserViewModel)
  case failed(String)
}

struct DiffBrowserViewModel: Equatable {
  var range: RevisionRange
  var files: [DiffFileViewModel]
  var inlineThreads: [InlineThreadLocation: [InlineThreadViewModel]]

  init(diff: Diff, threads: [InlineThreadViewModel] = []) {
    self.range = diff.range
    self.files = diff.files.map(DiffFileViewModel.init)
    self.inlineThreads = Dictionary(grouping: threads, by: \.location)
  }

  func file(withID id: DiffFileViewModel.ID?) -> DiffFileViewModel? {
    guard let id else { return nil }
    return files.first { $0.id == id }
  }

  func threads(at location: InlineThreadLocation) -> [InlineThreadViewModel] {
    inlineThreads[location] ?? []
  }

  func adding(thread: InlineThreadViewModel) -> DiffBrowserViewModel {
    var copy = self
    copy.inlineThreads[thread.location, default: []].append(thread)
    return copy
  }
}

struct DiffFileViewModel: Identifiable, Equatable {
  typealias ID = String

  var path: String
  var oldPath: String?
  var status: FileStatus
  var stats: DiffStats
  var isBinary: Bool
  var hunks: [DiffHunkViewModel]

  var id: String { path }

  init(file: DiffFile) {
    self.path = file.path
    self.oldPath = file.oldPath
    self.status = file.status
    self.stats = file.stats
    self.isBinary = file.isBinary
    self.hunks = file.hunks.enumerated().map { DiffHunkViewModel(hunk: $0.element, index: $0.offset) }
  }
}

struct DiffHunkViewModel: Identifiable, Equatable {
  typealias ID = String

  var id: String
  var header: DiffRange
  var section: String?
  var lines: [DiffLineViewModel]

  init(hunk: DiffHunk, index: Int) {
    self.id = "hunk_\(index)_\(hunk.header.baseStart)_\(hunk.header.headStart)"
    self.header = hunk.header
    self.section = hunk.section
    self.lines = hunk.lines.enumerated().map { DiffLineViewModel(line: $0.element, index: $0.offset) }
  }
}

struct DiffLineViewModel: Identifiable, Equatable {
  typealias ID = String

  var id: String
  var kind: DiffLineKind
  var text: String
  var baseLine: UInt32?
  var headLine: UInt32?
  var highlights: [LineHighlight]

  init(line: DiffLine, index: Int) {
    self.id = "line_\(index)"
    self.kind = line.kind
    self.text = line.text
    self.baseLine = line.baseLine
    self.headLine = line.headLine
    self.highlights = line.highlights
  }
}

struct InlineThreadLocation: Hashable, Equatable {
  var filePath: String
  var diffSide: DiffSide
  var line: Int
}

struct InlineCommentViewModel: Identifiable, Equatable {
  var id: UUID
  var authorName: String?
  var body: String
  var createdAt: Date?
}

struct InlineThreadViewModel: Identifiable, Equatable {
  var id: UUID
  var pluginID: String
  var location: InlineThreadLocation
  var title: String?
  var comments: [InlineCommentViewModel]

  var lastUpdated: Date? {
    comments.map(\.createdAt).compactMap { $0 }.max()
  }
}

struct InlineCommentDraft {
  var location: InlineThreadLocation
  var body: String
}

enum InlineQuickLabel: String, CaseIterable {
  case nit = "Nit"
  case question = "Question"
  case blocking = "Blocking"

  var displayName: String { rawValue }

  var prefix: String { "[\(rawValue)] " }
}

extension DiffLineViewModel {
  func inlineLocations(newPath: String, basePath: String?) -> [InlineThreadLocation] {
    var locations: [InlineThreadLocation] = []

    if let headLine {
      locations.append(InlineThreadLocation(filePath: newPath, diffSide: .head, line: Int(headLine)))
    }

    if let baseLine {
      let path = basePath ?? newPath
      locations.append(InlineThreadLocation(filePath: path, diffSide: .base, line: Int(baseLine)))
    }

    return locations
  }

  func preferredComposerLocation(newPath: String, basePath: String?) -> InlineThreadLocation? {
    if let headLine {
      return InlineThreadLocation(filePath: newPath, diffSide: .head, line: Int(headLine))
    }

    if let baseLine {
      let path = basePath ?? newPath
      return InlineThreadLocation(filePath: path, diffSide: .base, line: Int(baseLine))
    }

    return nil
  }
}
