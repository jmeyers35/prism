import SwiftUI
import PrismFFI

struct DiffBrowserView: View {
  var viewModel: DiffBrowserViewModel
  @Binding var selection: DiffFileViewModel.ID?
  var reload: () -> Void
  var onAddComment: (InlineCommentDraft) -> Void

  var body: some View {
    HStack(spacing: 0) {
      DiffFileListView(files: viewModel.files, selection: $selection)
        .frame(minWidth: 260, idealWidth: 280)

      Divider()

      DiffFileDetailView(
        file: viewModel.file(withID: selection),
        threads: viewModel.inlineThreads,
        reload: reload,
        onAddComment: onAddComment
      )
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
  var threads: [InlineThreadLocation: [InlineThreadViewModel]]
  var reload: () -> Void
  var onAddComment: (InlineCommentDraft) -> Void

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
                DiffHunkView(
                  newPath: file.path,
                  basePath: file.oldPath,
                  hunk: hunk,
                  threads: threads,
                  onAddComment: onAddComment
                )
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
  var newPath: String
  var basePath: String?
  var hunk: DiffHunkViewModel
  var threads: [InlineThreadLocation: [InlineThreadViewModel]]
  var onAddComment: (InlineCommentDraft) -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      DiffHunkHeaderView(hunk: hunk)

      VStack(spacing: 0) {
        ForEach(Array(hunk.lines.indices), id: \.self) { index in
          let line = hunk.lines[index]
          let locations = line.inlineLocations(newPath: newPath, basePath: basePath)
          let lineThreads = locations.flatMap { threads[$0] ?? [] }
          DiffLineBlockView(
            line: line,
            threads: lineThreads,
            preferredLocation: line.preferredComposerLocation(newPath: newPath, basePath: basePath),
            onAddComment: onAddComment
          )
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

private struct DiffLineBlockView: View {
  var line: DiffLineViewModel
  var threads: [InlineThreadViewModel]
  var preferredLocation: InlineThreadLocation?
  var onAddComment: (InlineCommentDraft) -> Void

  @State private var isComposerVisible = false
  @State private var draftText = ""

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      ZStack(alignment: .topTrailing) {
        DiffLineRow(line: line)
        if preferredLocation != nil, !isComposerVisible {
          Button {
            isComposerVisible = true
          } label: {
            Image(systemName: "plus.bubble")
              .imageScale(.small)
              .padding(6)
              .background(Color(nsColor: .windowBackgroundColor).opacity(0.9))
              .clipShape(Circle())
          }
          .buttonStyle(.plain)
          .padding(.top, 2)
          .padding(.trailing, 6)
        }
      }

      ForEach(threads) { thread in
        InlineThreadCardView(thread: thread)
      }

      if isComposerVisible {
        InlineCommentComposerView(
          text: $draftText,
          onSubmit: submitDraft,
          onCancel: cancelDraft,
          onApplyLabel: applyLabel
        )
      }
    }
  }

  private func submitDraft() {
    guard let location = preferredLocation else { return }
    let trimmed = draftText.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return }
    onAddComment(InlineCommentDraft(location: location, body: trimmed))
    draftText = ""
    isComposerVisible = false
  }

  private func cancelDraft() {
    draftText = ""
    isComposerVisible = false
  }

  private func applyLabel(_ label: InlineQuickLabel) {
    if !draftText.hasPrefix(label.prefix) {
      draftText = label.prefix + draftText
    }
    isComposerVisible = true
  }
}

private struct InlineThreadCardView: View {
  var thread: InlineThreadViewModel

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      if let title = thread.title, !title.isEmpty {
        Text(title)
          .font(.caption.weight(.semibold))
      }

      ForEach(thread.comments) { comment in
        VStack(alignment: .leading, spacing: 4) {
          if let author = comment.authorName, !author.isEmpty {
            HStack(spacing: 6) {
              Text(author)
                .font(.caption.weight(.medium))
              if let createdAt = comment.createdAt {
                Text(createdAt, style: .relative)
                  .font(.caption)
                  .foregroundStyle(.secondary)
              }
            }
          }

          Text(comment.body)
            .font(.body)
        }
        .padding(8)
        .background(Color(nsColor: .textBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(
          RoundedRectangle(cornerRadius: 8)
            .stroke(Color(nsColor: .separatorColor), lineWidth: 1)
        )
      }
    }
    .padding(.leading, 16)
  }
}

private struct InlineCommentComposerView: View {
  @Binding var text: String
  var onSubmit: () -> Void
  var onCancel: () -> Void
  var onApplyLabel: (InlineQuickLabel) -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      HStack(spacing: 8) {
        ForEach(InlineQuickLabel.allCases, id: \.self) { label in
          Button(label.displayName) {
            onApplyLabel(label)
          }
          .buttonStyle(.borderless)
          .padding(.horizontal, 8)
          .padding(.vertical, 4)
          .background(Color.accentColor.opacity(0.15))
          .clipShape(Capsule())
        }
      }

      TextEditor(text: $text)
        .frame(minHeight: 80)
        .padding(8)
        .background(Color(nsColor: .textBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(
          RoundedRectangle(cornerRadius: 8)
            .stroke(Color(nsColor: .separatorColor), lineWidth: 1)
        )

      HStack {
        Button("Cancel", action: onCancel)

        Spacer()

        Button("Add Comment", action: onSubmit)
          .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
          .keyboardShortcut(.return, modifiers: [.command])
      }
    }
    .padding(12)
    .background(Color(nsColor: .windowBackgroundColor))
    .clipShape(RoundedRectangle(cornerRadius: 10))
    .padding(.leading, 16)
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

      let location = InlineThreadLocation(filePath: file.path, diffSide: .head, line: 11)
      let comment = InlineCommentViewModel(
        id: UUID(),
        authorName: "Reviewer",
        body: "Consider renaming this method for clarity.",
        createdAt: Date()
      )
      let thread = InlineThreadViewModel(
        id: UUID(),
        pluginID: "preview",
        location: location,
        title: "Preview Thread",
        comments: [comment]
      )

      return DiffBrowserViewModel(diff: diff, threads: [thread])
    }
  }

  struct DiffBrowserView_Previews: PreviewProvider {
    static var previews: some View {
      let viewModel = DiffPreviewFactory.previewViewModel()
      DiffBrowserView(
        viewModel: viewModel,
        selection: .constant(viewModel.files.first?.id),
        reload: {},
        onAddComment: { _ in }
      )
      .frame(width: 800, height: 480)
    }
  }
#endif
