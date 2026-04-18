import SwiftUI

struct GeniuzMenu: View {
    @ObservedObject var service: GeniuzService

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            HStack {
                Text("Geniuz")
                    .font(.headline)
                Spacer()
                Text("v\(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.1.0")")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            .padding(.horizontal, 16)
            .padding(.top, 12)
            .padding(.bottom, 8)

            Divider()

            // Status
            VStack(alignment: .leading, spacing: 6) {
                if service.stationExists {
                    HStack(spacing: 6) {
                        Image(systemName: "circle.fill")
                            .font(.system(size: 6))
                            .foregroundColor(.green)
                        Text("\(service.memoryCount) memories")
                            .font(.system(.body, design: .rounded))
                    }

                    if !service.recentGists.isEmpty {
                        recentMemoriesSection
                            .padding(.top, 6)
                    }
                } else {
                    HStack(spacing: 6) {
                        Image(systemName: "circle.fill")
                            .font(.system(size: 6))
                            .foregroundColor(.secondary)
                        Text("No memories yet")
                            .foregroundColor(.secondary)
                    }
                    Text("Start a conversation in Claude Desktop. Say something worth remembering.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .padding(.top, 2)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)

            Divider()

            // Claude Desktop status
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    Image(systemName: service.mcpInstalled ? "checkmark.circle.fill" : "xmark.circle")
                        .foregroundColor(service.mcpInstalled ? .green : .orange)
                    Text(service.mcpInstalled ? "Claude Desktop connected" : "Claude Desktop not configured")
                        .font(.caption)
                }

                if !service.mcpInstalled {
                    Button("Configure Claude Connection") {
                        service.configureClaudeConnection()
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .tint(.orange)
                }

                if service.mcpInstalled && service.restartRequired {
                    HStack(alignment: .top, spacing: 6) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundColor(.yellow)
                            .font(.caption)
                        Text("Restart Claude Desktop to activate Geniuz.")
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .padding(.top, 2)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)

            if !service.cliOnPath {
                Divider()
                cliInstallSection
            }

            Divider()

            Button(action: { NSApplication.shared.terminate(nil) }) {
                Label("Quit Geniuz", systemImage: "power")
            }
            .buttonStyle(.plain)
            .padding(.horizontal, 16)
            .padding(.vertical, 6)
            .padding(.bottom, 4)
        }
        .frame(width: 280)
    }

    // MARK: - CLI install section

    private var cliInstallSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: "terminal")
                    .foregroundColor(.secondary)
                    .font(.caption)
                Text("Use Geniuz from Terminal")
                    .font(.caption)
            }

            if service.cliCopyConfirmation {
                HStack(alignment: .top, spacing: 6) {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.green)
                        .font(.caption)
                    Text("Command copied — paste into Terminal and press Return.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.top, 2)
            } else {
                Button("Copy Install Command") {
                    service.copyCliInstallCommand()
                }
                .buttonStyle(.bordered)
                .controlSize(.small)

                Text("Copies a one-line `sudo` command. Paste into Terminal to add `geniuz` to your PATH.")
                    .font(.caption2)
                    .foregroundColor(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 2)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
    }

    // MARK: - Recent memories section

    private var recentMemoriesSection: some View {
        VStack(alignment: .leading, spacing: 4) {
            Button(action: {
                withAnimation(.easeInOut(duration: 0.15)) {
                    service.recentExpanded.toggle()
                }
            }) {
                HStack(spacing: 4) {
                    Text("RECENT MEMORIES")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                        .textCase(.uppercase)
                    Spacer()
                    Image(systemName: service.recentExpanded ? "chevron.up" : "chevron.down")
                        .font(.system(size: 9, weight: .semibold))
                        .foregroundColor(.secondary)
                }
            }
            .buttonStyle(.plain)

            let visible = service.recentExpanded
                ? Array(service.recentGists.prefix(5))
                : Array(service.recentGists.prefix(1))

            VStack(alignment: .leading, spacing: 6) {
                ForEach(Array(visible.enumerated()), id: \.offset) { _, gist in
                    HStack(alignment: .top, spacing: 8) {
                        Image(systemName: "circle.fill")
                            .font(.system(size: 4))
                            .foregroundColor(.secondary)
                            .padding(.top, 5)
                        Text(gist)
                            .font(.caption)
                            .lineLimit(2)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
        }
    }
}
