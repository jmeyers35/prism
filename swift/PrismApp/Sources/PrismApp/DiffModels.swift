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
  var rows: [DiffLinePairViewModel]

  init(hunk: DiffHunk, index: Int) {
    self.id = "hunk_\(index)_\(hunk.header.baseStart)_\(hunk.header.headStart)"
    self.header = hunk.header
    self.section = hunk.section
    let lineViewModels = hunk.lines.enumerated().map { DiffLineViewModel(line: $0.element, index: $0.offset) }
    self.rows = DiffLinePairViewModel.buildRows(from: lineViewModels)
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

struct DiffLinePairViewModel: Identifiable, Equatable {
  typealias ID = String

  var id: String
  var base: DiffLineViewModel?
  var head: DiffLineViewModel?

  static func buildRows(from lines: [DiffLineViewModel]) -> [DiffLinePairViewModel] {
    var result: [DiffLinePairViewModel] = []
    var index = 0

    while index < lines.count {
      let line = lines[index]

      switch line.kind {
      case .context:
        result.append(DiffLinePairViewModel(id: "row_\(line.id)", base: line, head: line))
        index += 1

      case .addition, .deletion:
        var deletions: [DiffLineViewModel] = []
        var additions: [DiffLineViewModel] = []

        while index < lines.count, lines[index].kind != .context {
          let current = lines[index]
          if current.kind == .deletion {
            deletions.append(current)
          } else if current.kind == .addition {
            additions.append(current)
          }
          index += 1
        }

        let pairCount = max(deletions.count, additions.count)

        for pairIndex in 0..<pairCount {
          let base = pairIndex < deletions.count ? deletions[pairIndex] : nil
          let head = pairIndex < additions.count ? additions[pairIndex] : nil
          let identifier = base?.id ?? head?.id ?? UUID().uuidString
          result.append(DiffLinePairViewModel(id: "row_\(identifier)", base: base, head: head))
        }
      }
    }

    return result
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

extension DiffLinePairViewModel {
  func inlineLocations(newPath: String, basePath: String?) -> [InlineThreadLocation] {
    var locations: [InlineThreadLocation] = []
    var seen: Set<InlineThreadLocation> = []

    if let base {
      for location in base.inlineLocations(newPath: newPath, basePath: basePath) where seen.insert(location).inserted {
        locations.append(location)
      }
    }

    if let head {
      let isDuplicateContext = head.id == base?.id
      if !isDuplicateContext {
        for location in head.inlineLocations(newPath: newPath, basePath: basePath) where seen.insert(location).inserted {
          locations.append(location)
        }
      }
    }

    return locations
  }

  func preferredComposerLocation(newPath: String, basePath: String?) -> InlineThreadLocation? {
    if let head, let location = head.preferredComposerLocation(newPath: newPath, basePath: basePath) {
      return location
    }

    if let base, let location = base.preferredComposerLocation(newPath: newPath, basePath: basePath) {
      return location
    }

    return nil
  }
}
