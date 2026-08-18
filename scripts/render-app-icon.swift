import AppKit

/// Light Credit-matched macOS squircle: white plate, zinc glass crystal,
/// indigo 3-facet folded chevron (人). Not Cursor's NE arrow.

let size: CGFloat = 1024
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
ctx.translateBy(x: 0, y: size)
ctx.scaleBy(x: 1, y: -1)

func srgb(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1) -> CGColor {
  CGColor(srgbRed: r, green: g, blue: b, alpha: a)
}

func fill(_ path: CGPath, _ color: CGColor) {
  ctx.saveGState()
  ctx.addPath(path)
  ctx.setFillColor(color)
  ctx.fillPath()
  ctx.restoreGState()
}

func stroke(_ path: CGPath, _ color: CGColor, _ width: CGFloat) {
  ctx.saveGState()
  ctx.addPath(path)
  ctx.setStrokeColor(color)
  ctx.setLineWidth(width)
  ctx.setLineJoin(.round)
  ctx.setLineCap(.round)
  ctx.strokePath()
  ctx.restoreGState()
}

func poly(_ points: [CGPoint]) -> CGMutablePath {
  let path = CGMutablePath()
  path.addLines(between: points)
  path.closeSubpath()
  return path
}

func line(_ a: CGPoint, _ b: CGPoint) -> CGMutablePath {
  let path = CGMutablePath()
  path.move(to: a)
  path.addLine(to: b)
  return path
}

func lerp(_ a: CGPoint, _ b: CGPoint, _ t: CGFloat) -> CGPoint {
  CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t)
}

let padding = size * 0.098
let plate = CGRect(x: padding, y: padding, width: size - padding * 2, height: size - padding * 2)
let corner = plate.width * 0.223
let squircle = CGPath(roundedRect: plate, cornerWidth: corner, cornerHeight: corner, transform: nil)

let shadow = CGPath(
  roundedRect: plate.offsetBy(dx: 0, dy: size * 0.018),
  cornerWidth: corner,
  cornerHeight: corner,
  transform: nil
)
fill(shadow, srgb(0.09, 0.09, 0.12, 0.10))

ctx.saveGState()
ctx.addPath(squircle)
ctx.clip()
fill(squircle, srgb(1, 1, 1))

if let glow = CGGradient(
  colorsSpace: colorSpace,
  colors: [srgb(0.93, 0.94, 1, 0.85), srgb(1, 1, 1, 0)] as CFArray,
  locations: [0, 1]
) {
  ctx.drawRadialGradient(
    glow,
    startCenter: CGPoint(x: size * 0.38, y: size * 0.30),
    startRadius: 0,
    endCenter: CGPoint(x: size * 0.38, y: size * 0.30),
    endRadius: size * 0.52,
    options: []
  )
}

let cx = size * 0.5
let cy = size * 0.505
let rx = size * 0.228
let ry = size * 0.132

let n  = CGPoint(x: cx, y: cy - ry * 2)
let ne = CGPoint(x: cx + rx, y: cy - ry)
let se = CGPoint(x: cx + rx, y: cy + ry)
let s  = CGPoint(x: cx, y: cy + ry * 2)
let sw = CGPoint(x: cx - rx, y: cy + ry)
let nw = CGPoint(x: cx - rx, y: cy - ry)
let c  = CGPoint(x: cx, y: cy)

let hex = poly([n, ne, se, s, sw, nw])
let left = poly([nw, c, s, sw])
let right = poly([ne, se, s, c])
let top = poly([n, ne, c, nw])

fill(hex, srgb(0.96, 0.96, 0.97, 0.92))
fill(left, srgb(0.89, 0.89, 0.91, 0.95))
fill(right, srgb(0.82, 0.82, 0.86, 0.96))
fill(top, srgb(0.94, 0.94, 0.96, 0.94))

if let topLight = CGGradient(
  colorsSpace: colorSpace,
  colors: [srgb(1, 1, 1, 0.85), srgb(1, 1, 1, 0)] as CFArray,
  locations: [0, 1]
) {
  ctx.saveGState()
  ctx.addPath(top)
  ctx.clip()
  ctx.drawLinearGradient(topLight, start: n, end: c, options: [])
  ctx.restoreGState()
}

let crystalWidth = max(size * 0.006, 3)
stroke(hex, srgb(0.71, 0.71, 0.75, 0.9), crystalWidth)
stroke(line(nw, c), srgb(1, 1, 1, 0.7), crystalWidth * 0.7)
stroke(line(ne, c), srgb(0.63, 0.63, 0.70, 0.45), crystalWidth * 0.7)
stroke(line(c, s), srgb(0.55, 0.55, 0.62, 0.4), crystalWidth * 0.85)
stroke(line(n, c), srgb(1, 1, 1, 0.55), crystalWidth * 0.55)

let rim = CGMutablePath()
rim.move(to: nw)
rim.addLine(to: n)
rim.addLine(to: ne)
stroke(rim, srgb(1, 1, 1, 0.95), crystalWidth * 1.15)

func thick(_ a: CGPoint, _ b: CGPoint, _ width: CGFloat, toward: CGPoint) -> CGMutablePath {
  let dx = b.x - a.x
  let dy = b.y - a.y
  let len = max(hypot(dx, dy), 1)
  var ox = -dy / len * width
  var oy = dx / len * width
  let mid = lerp(a, b, 0.5)
  if ox * (toward.x - mid.x) + oy * (toward.y - mid.y) < 0 {
    ox = -ox
    oy = -oy
  }
  return poly([
    a,
    b,
    CGPoint(x: b.x + ox, y: b.y + oy),
    CGPoint(x: a.x + ox, y: a.y + oy),
  ])
}

let peak = lerp(n, c, 0.22)
let leftFoot = lerp(sw, nw, 0.18)
let rightFoot = lerp(se, ne, 0.12)
let thickness = size * 0.072
let leftFacet = thick(peak, leftFoot, thickness, toward: c)
let rightFacet = thick(peak, rightFoot, thickness * 0.92, toward: c)
let accent = poly([
  lerp(peak, c, 0.62),
  lerp(leftFoot, c, 0.55),
  lerp(rightFoot, c, 0.55),
])

fill(rightFacet, srgb(0.310, 0.275, 0.898))
fill(leftFacet, srgb(0.545, 0.533, 0.973))
fill(accent, srgb(0.388, 0.400, 0.945, 0.95))
stroke(leftFacet, srgb(1, 1, 1, 0.45), crystalWidth * 0.4)
stroke(line(peak, lerp(peak, c, 0.45)), srgb(0.31, 0.27, 0.70, 0.18), crystalWidth * 0.35)

ctx.restoreGState()
stroke(squircle, srgb(0.894, 0.894, 0.906), size * 0.008)

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

let dest = CommandLine.arguments.dropFirst().first ?? "icon.png"
try png.write(to: URL(fileURLWithPath: dest))
print("wrote \(dest)")
