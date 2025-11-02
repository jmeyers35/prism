import SwiftUI

struct AttachFlowView: View {
  @EnvironmentObject private var sessionStore: SessionStore

  var viewModel: SessionViewModel

  var body: some View {
    VStack(alignment: .leading, spacing: 24) {
      VStack(alignment: .leading, spacing: 8) {
        Text("Attach to Review Provider")
          .font(.title.weight(.semibold))

        Text("Select an integration for \(viewModel.repositoryName) and attach to an existing or new review thread.")
          .font(.body)
          .foregroundStyle(.secondary)
      }

      content

      Spacer()
    }
    .padding(32)
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    .background(Color(nsColor: .windowBackgroundColor))
    .task {
      await sessionStore.loadPluginsIfNeeded()
    }
  }

  @ViewBuilder
  private var content: some View {
    if sessionStore.isLoadingPlugins {
      ProgressView("Loading integrations…")
        .progressViewStyle(.circular)
    } else if let model = sessionStore.attachModel {
      attachForm(model: model)
    } else {
      VStack(alignment: .leading, spacing: 12) {
        Text(sessionStore.attachErrorMessage ?? "No integrations available.")
          .font(.body)

        HStack(spacing: 12) {
          Button("Retry") {
            Task { await sessionStore.reloadPlugins() }
          }
          Button("Cancel") {
            sessionStore.closeSession()
          }
        }
      }
    }
  }

  private func attachForm(model: AttachModel) -> some View {
    let selection = Binding(
      get: { model.selectedPluginID },
      set: { sessionStore.selectPlugin(id: $0) }
    )

    let threadBinding = Binding(
      get: { model.threadID },
      set: { sessionStore.updateThreadID($0) }
    )

    let selectedOption = model.option(for: model.selectedPluginID)
    let storedThreads = model.storedThreads(for: model.selectedPluginID)
    let providerThreads = model.pluginThreads(for: model.selectedPluginID)
    let currentThreadLabel = threadDisplayText(
      for: model.threadID,
      storedThreads: storedThreads,
      providerThreads: providerThreads
    )
    let threadPlaceholder = selectedOption?.supportsAttachWithoutThread == true
      ? "Paste existing thread ID (optional)"
      : "Paste thread ID (required)"

    return VStack(alignment: .leading, spacing: 16) {
      Picker("Integration", selection: selection) {
        ForEach(model.options) { option in
          Text(option.label)
            .tag(option.id)
        }
      }
      .pickerStyle(.menu)
      .frame(maxWidth: 320)

      if !storedThreads.isEmpty {
        VStack(alignment: .leading, spacing: 4) {
          Text("Recently linked threads")
            .font(.subheadline.weight(.semibold))

          Menu {
            ForEach(storedThreads) { thread in
              Button {
                sessionStore.selectStoredThread(id: thread.id)
              } label: {
                HStack {
                  Text(thread.title ?? thread.id)
                  if thread.id == model.threadID {
                    Spacer()
                    Image(systemName: "checkmark")
                  }
                }
              }
            }

            if !model.threadID.isEmpty {
              Divider()
              Button("Clear selection") {
                sessionStore.updateThreadID("")
              }
            }
          } label: {
            HStack {
              Text(model.threadID.isEmpty ? "Choose a thread" : currentThreadLabel)
              Spacer(minLength: 4)
              Image(systemName: "chevron.down")
                .font(.footnote)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(Color(nsColor: .controlBackgroundColor))
            .clipShape(RoundedRectangle(cornerRadius: 6))
          }
        }
      }

      if let selectedOption, selectedOption.summary.capabilities.supportsListThreads {
        VStack(alignment: .leading, spacing: 8) {
          if sessionStore.isLoadingPluginThreads {
            ProgressView("Fetching threads…")
              .progressViewStyle(.circular)
          } else {
            Button("Browse threads from \(selectedOption.label)") {
              Task { await sessionStore.loadPluginThreads() }
            }
            .buttonStyle(.bordered)
          }

          if !providerThreads.isEmpty {
            Menu {
              ForEach(providerThreads) { thread in
                Button {
                  sessionStore.selectPluginThread(id: thread.id)
                } label: {
                  HStack {
                    Text(thread.title ?? thread.id)
                    if thread.id == model.threadID {
                      Spacer()
                      Image(systemName: "checkmark")
                    }
                  }
                }
              }
            } label: {
              HStack {
                Text(model.threadID.isEmpty ? "Select fetched thread" : currentThreadLabel)
                Spacer(minLength: 4)
                Image(systemName: "chevron.down")
                  .font(.footnote)
              }
              .padding(.horizontal, 12)
              .padding(.vertical, 6)
              .background(Color(nsColor: .controlBackgroundColor))
              .clipShape(RoundedRectangle(cornerRadius: 6))
            }
          }
        }
      }

      VStack(alignment: .leading, spacing: 4) {
        TextField(threadPlaceholder, text: threadBinding)
          .textFieldStyle(.roundedBorder)
          .frame(maxWidth: 320)

        if let selectedOption {
          Text(selectedOption.supportsAttachWithoutThread ? "Leave blank to create a new thread." : "Provide an existing thread identifier to continue that conversation.")
            .font(.footnote)
            .foregroundStyle(.secondary)
        }
      }

      if let error = sessionStore.attachErrorMessage, !error.isEmpty {
        Text(error)
          .font(.footnote)
          .foregroundStyle(.red)
      }

      Button {
        Task { await sessionStore.attachSelectedPlugin() }
      } label: {
        if sessionStore.isAttachingPlugin {
          ProgressView()
            .progressViewStyle(.circular)
            .frame(maxWidth: .infinity)
        } else {
          Text("Attach")
            .frame(maxWidth: .infinity)
        }
      }
      .buttonStyle(.borderedProminent)
      .frame(maxWidth: 200)
      .disabled(sessionStore.isAttachingPlugin || model.selectedPluginID.isEmpty)
    }
  }
}

private extension AttachFlowView {
  func threadDisplayText(
    for id: String,
    storedThreads: [AttachModel.StoredThread],
    providerThreads: [AttachModel.PluginThread]
  ) -> String {
    guard !id.isEmpty else { return "" }
    if let stored = storedThreads.first(where: { $0.id == id }) {
      return stored.title ?? stored.id
    }
    if let fetched = providerThreads.first(where: { $0.id == id }) {
      return fetched.title ?? fetched.id
    }
    return id
  }
}
