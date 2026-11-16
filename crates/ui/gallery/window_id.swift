// Prints the on-screen CGWindowID of the given process's window(s), one per line as
// `<id> <title>`. Used by `just capture-gallery` (justfile) to find the gallery window to
// capture, since GPUI 0.2.2 exposes no window handle a consumer crate can read (TASK-1.91's
// plan). CoreGraphics window listing needs no permission beyond the Screen Recording grant
// the capture itself already needs - no Accessibility permission, no AppleScript.
//
// macOS only, like the rest of this crate's platform (doc-16, doc-13 section 10). Not a
// Cargo dependency: invoked as `swift crates/ui/gallery/window_id.swift <pid>`, so it adds
// no new licence surface.
import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 2, let pid = Int(CommandLine.arguments[1]) else {
    FileHandle.standardError.write("usage: window_id.swift <pid>\n".data(using: .utf8)!)
    exit(2)
}

let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: AnyObject]] else {
    FileHandle.standardError.write("window_id.swift: CGWindowListCopyWindowInfo returned nothing\n".data(using: .utf8)!)
    exit(1)
}

for window in windows {
    guard let ownerPid = window[kCGWindowOwnerPID as String] as? Int, ownerPid == pid else {
        continue
    }
    let windowNumber = window[kCGWindowNumber as String] as? Int ?? -1
    let title = window[kCGWindowName as String] as? String ?? ""
    print("\(windowNumber) \(title)")
}
