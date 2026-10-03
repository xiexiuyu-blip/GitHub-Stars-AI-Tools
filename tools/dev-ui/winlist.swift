import Cocoa
let opts: CGWindowListOption = [.optionAll]
if let list = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] {
  for w in list {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? -1
    if (owner.contains("gsat") || owner.contains("Fox Stars")) && layer == 0 {
      let b = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
      print("id=\(w[kCGWindowNumber as String] ?? "") owner=\(owner) name=\(w[kCGWindowName as String] ?? "(无)") onscreen=\(w[kCGWindowIsOnscreen as String] ?? false) bounds=\(b)")
    }
  }
}
