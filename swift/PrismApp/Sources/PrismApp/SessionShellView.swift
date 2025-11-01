import SwiftUI

struct SessionShellView: View {
  @EnvironmentObject private var sessionStore: SessionStore

  var viewModel: SessionViewModel

  @State private var selection: SidebarItem? = .overview

  var body: some View {
    NavigationSplitView {
      List(selection: $selection) {
        Section("Workspace") {
          Label("Overview", systemImage: "square.grid.2x2")
            .tag(SidebarItem.overview)

          Label("Files", systemImage: "doc.text.magnifyingglass")
            .tag(SidebarItem.files)

          Label("Activity", systemImage: "clock")
            .tag(SidebarItem.activity)
        }
      }
      .listStyle(.sidebar)
    } detail: {
      DetailView(item: selection ?? .overview, viewModel: viewModel)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(nsColor: .windowBackgroundColor))
    }
    .navigationTitle(viewModel.repositoryName)
    .toolbar {
      ToolbarItemGroup(placement: .status) {
        if let branch = viewModel.currentBranch {
          Label(branch, systemImage: "arrow.branch")
        }

        if viewModel.hasUncommittedChanges {
          Label("Uncommitted Changes", systemImage: "exclamationmark.circle")
            .foregroundStyle(.orange)
        }
      }

      ToolbarItem(placement: .primaryAction) {
        Button("Refresh") {
          Task { await sessionStore.refreshActiveSession() }
        }
        .disabled(!sessionStore.hasActiveSession())
      }
    }
  }
}

private enum SidebarItem: Hashable {
  case overview
  case files
  case activity
}

private struct DetailView: View {
  var item: SidebarItem
  var viewModel: SessionViewModel

  var body: some View {
    switch item {
    case .overview:
      OverviewDetail(viewModel: viewModel)
    case .files:
      FilesDetail()
    case .activity:
      PlaceholderDetail(
        title: "Activity",
        message: "Recent review activity will be summarized here."
      )
    }
  }
}

private struct OverviewDetail: View {
  var viewModel: SessionViewModel

  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      Text(viewModel.repositoryName)
        .font(.largeTitle.weight(.semibold))

      VStack(alignment: .leading, spacing: 8) {
        Label(viewModel.repositoryPath, systemImage: "folder")

        if let branch = viewModel.defaultBranch {
          Label("Default branch: \(branch)", systemImage: "arrow.triangle.branch")
        }

        if let current = viewModel.currentBranch {
          Label("Current branch: \(current)", systemImage: "arrow.branch")
        }

        Label(
          viewModel.hasUncommittedChanges ? "Workspace has uncommitted changes" : "Workspace clean",
          systemImage: viewModel.hasUncommittedChanges ? "exclamationmark.circle" : "checkmark.circle"
        )
        .foregroundStyle(viewModel.hasUncommittedChanges ? .orange : .green)
      }

      Spacer()
    }
    .padding(32)
  }
}

private struct PlaceholderDetail: View {
  var title: String
  var message: String

  var body: some View {
    VStack(spacing: 12) {
      Text(title)
        .font(.title2.weight(.semibold))

      Text(message)
        .font(.body)
        .foregroundStyle(.secondary)
        .multilineTextAlignment(.center)
        .frame(maxWidth: 320)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

private struct FilesDetail: View {
  @EnvironmentObject private var sessionStore: SessionStore

  var body: some View {
    switch sessionStore.diffPhase {
    case .idle:
      VStack(spacing: 12) {
        Text("Open a repository to view diffs")
          .font(.callout)
          .foregroundStyle(.secondary)
      }
      .frame(maxWidth: .infinity, maxHeight: .infinity)
    case .loading:
      ProgressView("Loading diff…")
        .progressViewStyle(.circular)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    case .failed(let message):
      VStack(spacing: 12) {
        Text("We couldn't load the diff")
          .font(.headline)

        Text(message)
          .font(.body)
          .multilineTextAlignment(.center)
          .foregroundStyle(.secondary)

        Button("Retry") {
          Task { await sessionStore.reloadDiff() }
        }
      }
      .frame(maxWidth: .infinity, maxHeight: .infinity)
    case .loaded(let diffViewModel):
      DiffBrowserView(
        viewModel: diffViewModel,
        selection: Binding(
          get: { sessionStore.selectedDiffFileID },
          set: { sessionStore.selectedDiffFileID = $0 }
        ),
        reload: {
          Task { await sessionStore.reloadDiff() }
        }
      )
    }
  }
}
