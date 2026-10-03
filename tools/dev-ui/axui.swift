import Cocoa
import ApplicationServices

let a = CommandLine.arguments
func fail(_ m: String) -> Never { FileHandle.standardError.write((m + "\n").data(using: .utf8)!); exit(2) }
guard a.count >= 3, let pid = pid_t(a[2]) else { fail("usage: axui <cmd> <pid> ...") }
let cmd = a[1]
let app = AXUIElementCreateApplication(pid)

func attr(_ el: AXUIElement, _ n: String) -> CFTypeRef? { var v: CFTypeRef?; return AXUIElementCopyAttributeValue(el, n as CFString, &v) == .success ? v : nil }
func s(_ el: AXUIElement, _ n: String) -> String {
  guard let v = attr(el, n) else { return "" }
  if let t = v as? String { return t }
  if let num = v as? NSNumber { return num.stringValue }
  return ""
}
func kids(_ el: AXUIElement) -> [AXUIElement] { (attr(el, "AXChildren") as? [AXUIElement]) ?? [] }
func rect(_ el: AXUIElement) -> CGRect {
  var p = CGPoint.zero; var z = CGSize.zero
  if let v = attr(el, "AXPosition") { AXValueGetValue(v as! AXValue, .cgPoint, &p) }
  if let v = attr(el, "AXSize") { AXValueGetValue(v as! AXValue, .cgSize, &z) }
  return CGRect(origin: p, size: z)
}
func label(_ el: AXUIElement) -> String {
  for n in ["AXTitle", "AXDescription", "AXValue", "AXHelp"] { let t = s(el, n); if !t.isEmpty { return t } }
  for c in kids(el) where s(c, "AXRole") == "AXStaticText" { let t = s(c, "AXValue"); if !t.isEmpty { return t } }
  return ""
}
struct Node { let el: AXUIElement; let role: String; let label: String; let value: String; let ph: String; let r: CGRect; let depth: Int }
func walk(_ root: AXUIElement, max: Int = 8000) -> [Node] {
  var out: [Node] = []; var stack: [(AXUIElement, Int)] = [(root, 0)]
  while let (el, d) = stack.popLast(), out.count < max {
    out.append(Node(el: el, role: s(el, "AXRole"), label: label(el), value: s(el, "AXValue"), ph: s(el, "AXPlaceholderValue"), r: rect(el), depth: d))
    for c in kids(el).reversed() { stack.append((c, d + 1)) }
  }
  return out
}
func win() -> AXUIElement {
  let ws = (attr(app, "AXWindows") as? [AXUIElement]) ?? []
  for w in ws where s(w, "AXTitle").contains("Fox Stars Lab Dev") { return w }
  fail("找不到开发版窗口（AXWindows=\(ws.count)）")
}
let interactive: Set<String> = ["AXButton", "AXLink", "AXTextField", "AXTextArea", "AXComboBox", "AXSearchField", "AXRadioButton", "AXCheckBox", "AXPopUpButton", "AXMenuItem", "AXTab", "AXSlider"]
func short(_ t: String, _ n: Int = 70) -> String { let u = t.replacingOccurrences(of: "\n", with: " "); return u.count > n ? String(u.prefix(n)) + "…" : u }

switch cmd {
case "front":
  AXUIElementSetAttributeValue(app, "AXFrontmost" as CFString, kCFBooleanTrue)
  AXUIElementPerformAction(win(), "AXRaise" as CFString)
  print("front ok")
case "move":
  guard a.count >= 5, let x = Double(a[3]), let y = Double(a[4]) else { fail("axui move <pid> <x> <y>") }
  var pt = CGPoint(x: x, y: y)
  let v = AXValueCreate(.cgPoint, &pt)!
  let r = AXUIElementSetAttributeValue(win(), "AXPosition" as CFString, v)
  print("move \(Int(x)),\(Int(y)) result=\(r.rawValue)")
case "dump":
  let all = a.count >= 4 && a[3] == "--all"
  let nodes = walk(win())
  for (i, n) in nodes.enumerated() {
    if all ? (!n.label.isEmpty || !n.ph.isEmpty) : interactive.contains(n.role) {
      print("[\(i)] \(n.role) | \(short(n.label)) | ph=\(short(n.ph, 30)) | (\(Int(n.r.minX)),\(Int(n.r.minY)) \(Int(n.r.width))x\(Int(n.r.height)))")
    }
  }
  print("-- nodes=\(nodes.count)")
case "press":
  guard a.count >= 4 else { fail("axui press <pid> <substr> [nth]") }
  let sub = a[3]; let nth = a.count >= 5 ? (Int(a[4]) ?? 0) : 0
  let hits = walk(win()).filter { interactive.contains($0.role) && $0.label.contains(sub) }
  guard nth < hits.count else { fail("没找到含「\(sub)」的可点元素（共 \(hits.count) 个匹配）") }
  let r = AXUIElementPerformAction(hits[nth].el, "AXPress" as CFString)
  print("press「\(short(hits[nth].label, 40))」 role=\(hits[nth].role) result=\(r.rawValue) (matches=\(hits.count))")
case "settext":
  guard a.count >= 5 else { fail("axui settext <pid> <placeholder-or-label substr> <text>") }
  let sub = a[3]
  let hits = walk(win()).filter { ["AXTextField", "AXTextArea", "AXComboBox", "AXSearchField"].contains($0.role) && ($0.ph.contains(sub) || $0.label.contains(sub)) }
  guard let t = hits.first(where: { $0.ph == sub || $0.label == sub }) ?? hits.first else { fail("没找到输入框「\(sub)」") }
  AXUIElementSetAttributeValue(t.el, "AXFocused" as CFString, kCFBooleanTrue)
  let r = AXUIElementSetAttributeValue(t.el, "AXValue" as CFString, a[4] as CFString)
  print("settext result=\(r.rawValue) -> \(short(s(t.el, "AXValue"), 40))")
case "text":
  // 把窗口里所有静态文本按顺序打印出来（读页面内容用）
  for n in walk(win()) where n.role == "AXStaticText" && !n.value.isEmpty { print(short(n.value, 120)) }
default: fail("未知命令 \(cmd)")
}
