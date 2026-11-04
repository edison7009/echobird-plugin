import AppKit
import Foundation

if CommandLine.arguments[1] == "control" {
    let app = NSApplication.shared
    guard app.setActivationPolicy(.regular) else { exit(1) }
    print(ProcessInfo.processInfo.processIdentifier)
    fflush(stdout)
    _ = readLine()
} else {
    guard let pid = Int32(CommandLine.arguments[1]),
          let app = NSRunningApplication(processIdentifier: pid) else { exit(1) }
    print(app.activationPolicy.rawValue)
}
