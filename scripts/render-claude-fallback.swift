import AppKit

/// Composite the public-domain Claude symbol onto a Desktop-like tile.
/// Usage: render-claude-fallback.swift <symbol.svg> <dest.png>

let args = Array(CommandLine.arguments.dropFirst())
guard args.count >= 2 else {
  fputs("usage: render-claude-fallback.swift <symbol.svg> <dest.png>\n", stderr)
  exit(1)
}

let symbolURL = URL(fileURLWithPath: args[0])
let dest = URL(fileURLWithPath: args[1])
let size: CGFloat = 256

func loadSymbolImage() -> NSImage? {
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

let whiteSVG = FileManager.default.temporaryDirectory
  .appendingPathComponent("claude-symbol-white.svg")
let source = try String(contentsOf: symbolURL, encoding: .utf8)
let recolored = source.replacingOccurrences(
  of: "fill=\"hsl(14.8, 63.1%, 59.6%)\"",
  with: "fill=\"#FAF7F2\""
)
try recolored.write(to: whiteSVG, atomically: true, encoding: .utf8)
defer { try? FileManager.default.removeItem(at: whiteSVG) }

func loadRecolored() -> NSImage? {
  if let image = NSImage(contentsOf: whiteSVG), image.size.width > 0 {
    return image
  }
  return loadSymbolImage()
}

guard let symbol = loadRecolored() else {
  fputs("failed to load Claude symbol\n", stderr)
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
ctx.setFillColor(CGColor(srgbRed: 0.757, green: 0.373, blue: 0.235, alpha: 1))
ctx.fillPath()

if let glow = CGGradient(
  colorsSpace: colorSpace,
  colors: [
    CGColor(srgbRed: 0.85, green: 0.48, blue: 0.34, alpha: 0.55),
    CGColor(srgbRed: 0.757, green: 0.373, blue: 0.235, alpha: 0),
  ] as CFArray,
  locations: [0, 1]
) {
  ctx.drawRadialGradient(
    glow,
    startCenter: CGPoint(x: size * 0.36, y: size * 0.72),
    startRadius: 0,
    endCenter: CGPoint(x: size * 0.36, y: size * 0.72),
    endRadius: size * 0.55,
    options: []
  )
}

let inset = size * 0.16
let markRect = CGRect(x: inset, y: inset, width: size - inset * 2, height: size - inset * 2)
let nsContext = NSGraphicsContext(cgContext: ctx, flipped: false)
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = nsContext
symbol.draw(in: markRect, from: .zero, operation: .sourceOver, fraction: 1)
NSGraphicsContext.restoreGraphicsState()

ctx.addPath(squircle)
ctx.setStrokeColor(CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 0.14))
ctx.setLineWidth(size * 0.018)
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
