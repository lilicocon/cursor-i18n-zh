import AppKit

/// Composite an SVG mark onto a light rounded tile.
/// usage: render-brand-tile.swift <symbol.svg> <dest.png> <bgHex> [inset]

let args = Array(CommandLine.arguments.dropFirst())
guard args.count >= 3 else {
  fputs("usage: render-brand-tile.swift <symbol.svg> <dest.png> <bgHex> [inset]\n", stderr)
  exit(1)
}

let symbolURL = URL(fileURLWithPath: args[0])
let dest = URL(fileURLWithPath: args[1])
let bgHex = args[2]
let insetRatio = args.count >= 4 ? CGFloat(Double(args[3]) ?? 0.16) : 0.16
let size: CGFloat = 256

func parseHex(_ value: String) -> CGColor {
  var hex = value.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
  if hex.count == 3 {
    hex = hex.map { "\($0)\($0)" }.joined()
  }
  let scanner = Scanner(string: hex)
  var rgb: UInt64 = 0
  scanner.scanHexInt64(&rgb)
  let r = CGFloat((rgb >> 16) & 0xFF) / 255
  let g = CGFloat((rgb >> 8) & 0xFF) / 255
  let b = CGFloat(rgb & 0xFF) / 255
  return CGColor(srgbRed: r, green: g, blue: b, alpha: 1)
}

func loadSymbol() -> NSImage? {
  if let image = NSImage(contentsOf: symbolURL), image.size.width > 0 {
    return image
  }
  let previewDir = FileManager.default.temporaryDirectory
  let process = Process()
  process.executableURL = URL(fileURLWithPath: "/usr/bin/qlmanage")
  process.arguments = ["-t", "-s", "512", "-o", previewDir.path, symbolURL.path]
  process.standardOutput = FileHandle.nullDevice
  process.standardError = FileHandle.nullDevice
  try? process.run()
  process.waitUntilExit()
  let preview = previewDir.appendingPathComponent(symbolURL.lastPathComponent + ".png")
  defer { try? FileManager.default.removeItem(at: preview) }
  return NSImage(contentsOf: preview)
}

guard let symbol = loadSymbol() else {
  fputs("failed to load symbol\n", stderr)
  exit(1)
}

let colorSpace = CGColorSpaceCreateDeviceRGB()
guard let ctx = CGContext(
  data: nil,
  width: Int(size),
  height: Int(size),
  bitsPerComponent: 8,
  bytesPerRow: 0,
  space: colorSpace,
  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
) else {
  fputs("failed to create bitmap context\n", stderr)
  exit(1)
}

ctx.setAllowsAntialiasing(true)
ctx.setShouldAntialias(true)

let padding = size * 0.02
let plate = CGRect(x: padding, y: padding, width: size - padding * 2, height: size - padding * 2)
let corner = plate.width * 0.223
let squircle = CGPath(roundedRect: plate, cornerWidth: corner, cornerHeight: corner, transform: nil)

ctx.addPath(squircle)
ctx.setFillColor(parseHex(bgHex))
ctx.fillPath()

let inset = size * insetRatio
let markRect = CGRect(x: inset, y: inset, width: size - inset * 2, height: size - inset * 2)
let nsContext = NSGraphicsContext(cgContext: ctx, flipped: false)
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = nsContext
symbol.draw(in: markRect, from: .zero, operation: .sourceOver, fraction: 1)
NSGraphicsContext.restoreGraphicsState()

ctx.addPath(squircle)
ctx.setStrokeColor(CGColor(srgbRed: 0.894, green: 0.894, blue: 0.906, alpha: 1))
ctx.setLineWidth(size * 0.012)
ctx.strokePath()

guard let image = ctx.makeImage() else {
  fputs("failed to create image\n", stderr)
  exit(1)
}
let rep = NSBitmapImageRep(cgImage: image)
rep.size = NSSize(width: size, height: size)
guard let png = rep.representation(using: .png, properties: [:]) else {
  fputs("failed to encode png\n", stderr)
  exit(1)
}
try png.write(to: dest)
print("wrote \(dest.path)")
