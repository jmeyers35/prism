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
