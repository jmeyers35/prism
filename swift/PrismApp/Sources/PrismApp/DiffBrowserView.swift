import SwiftUI
import PrismFFI

struct DiffBrowserView: View {
  var viewModel: DiffBrowserViewModel
  @Binding var selection: DiffFileViewModel.ID?
  var reload: () -> Void

  var body: some View {
    HStack(spacing: 0) {
      DiffFileListView(files: viewModel.files, selection: $selection)
        .frame(minWidth: 260, idealWidth: 280)

      Divider()

      DiffFileDetailView(file: viewModel.file(withID: selection), reload: reload)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
    .background(Color(nsColor: .windowBackgroundColor))
  }
}

private struct DiffFileListView: View {
  var files: [DiffFileViewModel]
  @Binding var selection: DiffFileViewModel.ID?

  var body: some View {
    List(selection: $selection) {
      ForEach(files) { file in
        DiffFileRow(file: file)
          .tag(file.id as DiffFileViewModel.ID?)
      }
    }
    .listStyle(.sidebar)
  }
}

private struct DiffFileRow: View {
  var file: DiffFileViewModel

  var body: some View {
    HStack(spacing: 8) {
      Image(systemName: symbol)
        .foregroundStyle(color)

      VStack(alignment: .leading, spacing: 2) {
        Text(file.path)
          .font(.body)
          .lineLimit(1)

        if let oldPath = file.oldPath, oldPath != file.path {
          Text("Renamed from \(oldPath)")
            .font(.caption)
            .foregroundStyle(.secondary)
            .lineLimit(1)
        }
      }

      Spacer()

      HStack(spacing: 6) {
        if file.stats.additions > 0 {
          Text("+\(file.stats.additions)")
            .font(.caption.monospacedDigit())
            .foregroundStyle(.green)
        }

        if file.stats.deletions > 0 {
          Text("−\(file.stats.deletions)")
            .font(.caption.monospacedDigit())
            .foregroundStyle(.red)
        }
      }
    }
    .padding(.vertical, 4)
  }

  private var symbol: String {
    switch file.status {
    case .added:
      return "plus.circle.fill"
    case .deleted:
      return "minus.circle.fill"
    case .modified:
      return "square.and.pencil"
    case .renamed:
      return "arrow.triangle.branch"
    case .copied:
      return "doc.on.doc"
    case .typeChange:
      return "arrow.triangle.2.circlepath"
    }
  }

  private var color: Color {
    switch file.status {
    case .added:
      return .green
    case .deleted:
      return .red
    case .modified:
      return .accentColor
    case .renamed:
      return .blue
    case .copied:
      return .purple
    case .typeChange:
      return .orange
    }
  }
}

private struct DiffFileDetailView: View {
  var file: DiffFileViewModel?
  var reload: () -> Void

  var body: some View {
    Group {
      if let file {
        if file.isBinary {
          BinaryFileView(file: file)
        } else if file.hunks.isEmpty {
          EmptyDiffView(reload: reload)
        } else {
          ScrollView {
            LazyVStack(alignment: .leading, spacing: 24) {
              ForEach(file.hunks) { hunk in
                DiffHunkView(hunk: hunk)
              }
            }
            .padding(20)
          }
          .background(Color(nsColor: .textBackgroundColor))
        }
      } else {
        Text("Select a file to view its diff")
          .font(.body)
          .foregroundStyle(.secondary)
      }
    }
  }
}

private struct BinaryFileView: View {
  var file: DiffFileViewModel

  var body: some View {
    VStack(spacing: 12) {
      Image(systemName: "doc.richtext")
        .font(.system(size: 40))
        .foregroundStyle(.secondary)

      Text("\(file.path) is a binary file")
        .font(.callout)
        .foregroundStyle(.secondary)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

private struct EmptyDiffView: View {
  var reload: () -> Void

  var body: some View {
    VStack(spacing: 12) {
      Text("No changes in this diff")
        .font(.callout)
        .foregroundStyle(.secondary)

      Button("Reload", action: reload)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

private struct DiffHunkView: View {
  var hunk: DiffHunkViewModel

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      DiffHunkHeaderView(hunk: hunk)

      VStack(spacing: 0) {
        ForEach(Array(hunk.lines.indices), id: \.self) { index in
          DiffLineRow(line: hunk.lines[index])
          if index < hunk.lines.count - 1 {
            Divider()
          }
        }
      }
      .background(Color(nsColor: .textBackgroundColor))
      .clipShape(RoundedRectangle(cornerRadius: 6))
      .overlay(
        RoundedRectangle(cornerRadius: 6)
          .stroke(Color(nsColor: .separatorColor), lineWidth: 1)
      )
    }
  }
}

private struct DiffHunkHeaderView: View {
  var hunk: DiffHunkViewModel

  var body: some View {
    HStack(spacing: 8) {
      Text("@@ \(format(hunk.header)) @@")
        .font(.caption.monospacedDigit())

      if let section = hunk.section, !section.isEmpty {
        Text(section)
          .font(.caption)
          .foregroundStyle(.secondary)
      }

      Spacer()
    }
    .padding(.horizontal, 8)
    .padding(.vertical, 6)
    .background(Color.accentColor.opacity(0.12))
    .clipShape(RoundedRectangle(cornerRadius: 4))
  }

  private func format(_ range: DiffRange) -> String {
    "-\(range.baseStart),\(range.baseLines) +\(range.headStart),\(range.headLines)"
  }
}

private struct DiffLineRow: View {
  var line: DiffLineViewModel

  var body: some View {
    HStack(alignment: .top, spacing: 8) {
      lineNumber(line.baseLine)
        .frame(width: 50, alignment: .trailing)

      lineNumber(line.headLine)
        .frame(width: 50, alignment: .trailing)

      Text(verbatim: line.text)
        .font(.system(size: 13, design: .monospaced))
        .frame(maxWidth: .infinity, alignment: .leading)
    }
    .padding(.horizontal, 8)
    .padding(.vertical, 2)
    .background(backgroundColor)
  }

  private func lineNumber(_ value: UInt32?) -> Text {
    if let value {
      return Text("\(value)").font(.system(size: 11, design: .monospaced))
    }
    return Text(" ").font(.system(size: 11, design: .monospaced))
  }

  private var backgroundColor: Color {
    switch line.kind {
    case .context:
      return Color.clear
    case .addition:
      return Color.green.opacity(0.15)
    case .deletion:
      return Color.red.opacity(0.15)
    }
  }
}

#if DEBUG
  private enum DiffPreviewFactory {
    static func previewViewModel() -> DiffBrowserViewModel {
      let lines = [
        DiffLine(kind: .context, text: " func greet() {", baseLine: UInt32(10), headLine: UInt32(10), highlights: []),
        DiffLine(kind: .deletion, text: "  print(\"Hello\")", baseLine: UInt32(11), headLine: nil, highlights: []),
        DiffLine(kind: .addition, text: "  print(\"Hello, Prism\")", baseLine: nil, headLine: UInt32(11), highlights: [])
      ]

      let hunk = DiffHunk(
        header: DiffRange(baseStart: UInt32(10), baseLines: UInt32(2), headStart: UInt32(10), headLines: UInt32(2)),
        section: "greet()",
        lines: lines
      )

      let file = DiffFile(
        path: "Sources/Greetings.swift",
        oldPath: nil,
        status: .modified,
        stats: DiffStats(additions: UInt32(1), deletions: UInt32(1)),
        isBinary: false,
        hunks: [hunk]
      )

      let diff = Diff(
        range: RevisionRange(
          base: Revision(oid: "BASE", reference: nil, summary: nil, author: nil, committer: nil, timestamp: nil),
          head: Revision(oid: "HEAD", reference: nil, summary: nil, author: nil, committer: nil, timestamp: nil)
        ),
        files: [file]
      )

      return DiffBrowserViewModel(diff: diff)
    }
  }

  struct DiffBrowserView_Previews: PreviewProvider {
    static var previews: some View {
      let viewModel = DiffPreviewFactory.previewViewModel()
      DiffBrowserView(
        viewModel: viewModel,
        selection: .constant(viewModel.files.first?.id),
        reload: {}
      )
      .frame(width: 800, height: 480)
    }
  }
#endif
