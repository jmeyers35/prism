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
        ForEach(Array(hunk.rows.enumerated()), id: \.element.id) { index, line in
          let locations = line.inlineLocations(newPath: newPath, basePath: basePath)
          let lineThreads = locations.flatMap { threads[$0] ?? [] }
          DiffLineBlockView(
            line: line,
            threads: lineThreads,
            preferredLocation: line.preferredComposerLocation(newPath: newPath, basePath: basePath),
            onAddComment: onAddComment
          )
          if index < hunk.rows.count - 1 {
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
  var line: DiffLinePairViewModel

  var body: some View {
    HStack(spacing: 0) {
      DiffLineSideView(
        number: line.base?.baseLine,
        text: baseText,
        background: baseBackground
      )

      Rectangle()
        .fill(Color(nsColor: .separatorColor).opacity(0.35))
        .frame(width: 1)

      DiffLineSideView(
        number: line.head?.headLine,
        text: headText,
        background: headBackground
      )
    }
    .frame(maxWidth: .infinity)
  }

  private var baseText: String { line.base?.text ?? "" }
  private var headText: String { line.head?.text ?? "" }

  private var baseBackground: Color {
    switch line.base?.kind {
    case .deletion:
      return Color.red.opacity(0.15)
    case .addition:
      return Color.green.opacity(0.08)
    default:
      return Color.clear
    }
  }

  private var headBackground: Color {
    switch line.head?.kind {
    case .addition:
      return Color.green.opacity(0.15)
    case .deletion:
      return Color.red.opacity(0.08)
    default:
      return Color.clear
    }
  }
}

private struct DiffLineSideView: View {
  var number: UInt32?
  var text: String
  var background: Color

  private let numberColumnWidth: CGFloat = 48

  var body: some View {
    HStack(alignment: .top, spacing: 8) {
      LineNumberView(number: number)
        .frame(width: numberColumnWidth, alignment: .trailing)

      Text(verbatim: text)
        .font(.system(size: 13, design: .monospaced))
        .frame(maxWidth: .infinity, alignment: .leading)
    }
    .padding(.vertical, 4)
    .padding(.horizontal, 8)
    .background(background)
  }
}

private struct LineNumberView: View {
  var number: UInt32?

  var body: some View {
    Group {
      if let number {
        Text("\(number)")
      } else {
        Text(" ")
      }
    }
    .font(.system(size: 11, design: .monospaced))
    .foregroundStyle(.secondary)
  }
}

private struct DiffLineBlockView: View {
  var line: DiffLinePairViewModel
  var threads: [InlineThreadViewModel]
  var preferredLocation: InlineThreadLocation?
  var onAddComment: (InlineCommentDraft) -> Void

  @State private var isComposerVisible = false
  @State private var draftText = ""

  var body: some View {
    HStack(alignment: .top, spacing: 12) {
      CommentGutterView(
        canAddComment: preferredLocation != nil,
        isComposerVisible: isComposerVisible,
        hasDiscussion: !threads.isEmpty || isComposerVisible,
        onShowComposer: showComposer
      )

      VStack(alignment: .leading, spacing: 12) {
        DiffLineRow(line: line)

        if !threads.isEmpty {
          VStack(alignment: .leading, spacing: 12) {
            ForEach(threads) { thread in
              InlineThreadCardView(thread: thread)
            }
          }
        }

        if isComposerVisible {
          InlineCommentComposerView(
            text: $draftText,
            onSubmit: submitDraft,
            onCancel: cancelDraft,
            onApplyLabel: applyLabel
          )
          .transition(.move(edge: .top).combined(with: .opacity))
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    }
    .padding(.vertical, 6)
    .padding(.horizontal, 8)
  }

  private func showComposer() {
    guard preferredLocation != nil else { return }
    withAnimation(.easeInOut(duration: 0.15)) {
      isComposerVisible = true
    }
  }

  private func submitDraft() {
    guard let location = preferredLocation else { return }
    let trimmed = draftText.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return }
    onAddComment(InlineCommentDraft(location: location, body: trimmed))
    draftText = ""
    withAnimation(.easeInOut(duration: 0.15)) {
      isComposerVisible = false
    }
  }

  private func cancelDraft() {
    draftText = ""
    withAnimation(.easeInOut(duration: 0.15)) {
      isComposerVisible = false
    }
  }

  private func applyLabel(_ label: InlineQuickLabel) {
    if !draftText.hasPrefix(label.prefix) {
      draftText = label.prefix + draftText
    }
    showComposer()
  }
}

private struct CommentGutterView: View {
  var canAddComment: Bool
  var isComposerVisible: Bool
  var hasDiscussion: Bool
  var onShowComposer: () -> Void

  var body: some View {
    VStack(spacing: 8) {
      if canAddComment {
        Button(action: onShowComposer) {
          Image(systemName: "plus")
            .font(.system(size: 11, weight: .bold))
            .foregroundStyle(isComposerVisible ? Color.white : Color.accentColor)
            .frame(width: 24, height: 24)
            .background(
              Circle()
                .fill(isComposerVisible ? Color.accentColor : Color(nsColor: .windowBackgroundColor))
            )
            .overlay(
              Circle()
                .stroke(Color.accentColor.opacity(0.35), lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Add inline comment")
      } else {
        Circle()
          .fill(Color.clear)
          .frame(width: 24, height: 24)
      }

      if hasDiscussion {
        Rectangle()
          .fill(Color(nsColor: .separatorColor).opacity(0.4))
          .frame(width: 2)
          .frame(maxHeight: .infinity)
      }

      Spacer(minLength: 0)
    }
    .frame(width: 32, alignment: .top)
  }
}

private struct InlineThreadCardView: View {
  var thread: InlineThreadViewModel

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      if let title = thread.title, !title.isEmpty {
        Text(title)
          .font(.caption.weight(.semibold))
          .foregroundStyle(.secondary)
      }

      VStack(alignment: .leading, spacing: 12) {
        ForEach(Array(thread.comments.enumerated()), id: \.element.id) { index, comment in
          InlineThreadCommentView(comment: comment)

          if index < thread.comments.count - 1 {
            Divider()
              .foregroundStyle(Color(nsColor: .separatorColor).opacity(0.4))
          }
        }
      }

      if let updated = thread.lastUpdated {
        Text("Updated \(updated, style: .relative)")
          .font(.caption)
          .foregroundStyle(.secondary)
      }
    }
    .padding(16)
    .background(
      RoundedRectangle(cornerRadius: 12)
        .fill(Color(nsColor: .windowBackgroundColor).opacity(0.95))
    )
    .overlay(
      RoundedRectangle(cornerRadius: 12)
        .stroke(Color(nsColor: .separatorColor).opacity(0.6), lineWidth: 1)
    )
  }
}

private struct InlineThreadCommentView: View {
  var comment: InlineCommentViewModel

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      if let author = comment.authorName, !author.isEmpty {
        HStack(spacing: 6) {
          Text(author)
            .font(.footnote.weight(.semibold))

          if let createdAt = comment.createdAt {
            Text(createdAt, style: .relative)
              .font(.footnote)
              .foregroundStyle(.secondary)
          }
        }
      }

      Text(comment.body)
        .font(.body)
        .fixedSize(horizontal: false, vertical: true)
    }
  }
}

private struct InlineCommentComposerView: View {
  @Binding var text: String
  var onSubmit: () -> Void
  var onCancel: () -> Void
  var onApplyLabel: (InlineQuickLabel) -> Void

  private var isSubmitDisabled: Bool {
    text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      Text("New inline comment")
        .font(.callout.weight(.semibold))
        .foregroundStyle(.secondary)

      TextEditor(text: $text)
        .frame(minHeight: 96)
        .padding(10)
        .background(
          RoundedRectangle(cornerRadius: 10)
            .fill(Color(nsColor: .textBackgroundColor))
        )
        .overlay(
          RoundedRectangle(cornerRadius: 10)
            .stroke(Color(nsColor: .separatorColor).opacity(0.5), lineWidth: 1)
        )

      HStack(spacing: 8) {
        ForEach(InlineQuickLabel.allCases, id: \.self) { label in
          Button(label.displayName) {
            onApplyLabel(label)
          }
          .buttonStyle(QuickLabelButtonStyle())
        }

        Spacer()

        Button("Cancel", action: onCancel)
          .buttonStyle(.bordered)

        Button("Add Comment", action: onSubmit)
          .buttonStyle(.borderedProminent)
          .disabled(isSubmitDisabled)
          .keyboardShortcut(.return, modifiers: [.command])
      }
    }
    .padding(16)
    .background(
      RoundedRectangle(cornerRadius: 12)
        .fill(Color(nsColor: .windowBackgroundColor).opacity(0.95))
    )
    .overlay(
      RoundedRectangle(cornerRadius: 12)
        .stroke(Color(nsColor: .separatorColor).opacity(0.6), lineWidth: 1)
    )
  }
}

private struct QuickLabelButtonStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .font(.footnote.weight(.semibold))
      .padding(.horizontal, 10)
      .padding(.vertical, 6)
      .background(
        Capsule()
          .fill(Color.accentColor.opacity(configuration.isPressed ? 0.3 : 0.18))
      )
      .foregroundStyle(Color.accentColor)
      .animation(.easeInOut(duration: 0.15), value: configuration.isPressed)
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
