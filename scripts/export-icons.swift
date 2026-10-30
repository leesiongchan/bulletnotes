#!/usr/bin/env swift
import AppKit
import Foundation

// Run on macOS: swift scripts/export-icons.swift
let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let source = NSImage(contentsOf: root.appendingPathComponent("docs/branding/bulletnotes-app-icon.png"))!
let temporary = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
try FileManager.default.createDirectory(at: temporary, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: temporary) }

func png(size: Int) -> Data {
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: size * 4, bitsPerPixel: 32)!
    bitmap.bitmapData!.initialize(repeating: 0, count: bitmap.bytesPerRow * size)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    NSGraphicsContext.current!.imageInterpolation = .high
    let transform = NSAffineTransform()
    transform.scale(by: CGFloat(size) / 1254)
    transform.concat()
    // Remove only the pale generation debris outside the approved paper tile.
    NSBezierPath(roundedRect: NSRect(x: 92, y: 92, width: 1070, height: 1070),
        xRadius: 248, yRadius: 248).addClip()
    source.draw(in: NSRect(x: 0, y: 0, width: 1254, height: 1254))
    NSGraphicsContext.restoreGraphicsState()
    return bitmap.representation(using: .png, properties: [:])!
}

func save(_ data: Data, to path: String) throws {
    try data.write(to: root.appendingPathComponent(path))
    print("Exported \(path)")
}

let sizes = [16, 24, 32, 48, 64, 128, 180, 256, 512, 1024]
let images = Dictionary(uniqueKeysWithValues: sizes.map { ($0, png(size: $0)) })
for (path, size) in [
    ("assets/bulletnotes-logo.png", 1024),
    ("site/assets/bulletnotes-logo.png", 512),
    ("site/assets/favicon-32.png", 32),
    ("site/assets/apple-touch-icon.png", 180),
    ("packaging/icons/32x32.png", 32),
    ("packaging/icons/128x128.png", 128),
    ("packaging/icons/128x128@2x.png", 256),
] {
    try save(images[size]!, to: path)
}

// ICO directory entries point to lossless PNG frames, including the 256 px frame.
func littleEndian(_ value: Int, bytes: Int) -> Data {
    Data((0..<bytes).map { UInt8(truncatingIfNeeded: value >> ($0 * 8)) })
}
let icoSizes = [16, 24, 32, 48, 64, 256]
var ico = Data([0, 0, 1, 0]) + littleEndian(icoSizes.count, bytes: 2)
var offset = 6 + icoSizes.count * 16
for size in icoSizes {
    let frame = images[size]!
    ico += Data([UInt8(truncatingIfNeeded: size), UInt8(truncatingIfNeeded: size), 0, 0])
    ico += littleEndian(1, bytes: 2) + littleEndian(32, bytes: 2)
    ico += littleEndian(frame.count, bytes: 4) + littleEndian(offset, bytes: 4)
    offset += frame.count
}
for size in icoSizes { ico += images[size]! }
try save(ico, to: "packaging/icons/icon.ico")

let iconset = temporary.appendingPathComponent("Bulletnotes.iconset")
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
for size in [16, 32, 128, 256, 512] {
    try images[size]!.write(to: iconset.appendingPathComponent("icon_\(size)x\(size).png"))
    try images[size * 2]!.write(to: iconset.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", iconset.path, "-o", root.appendingPathComponent("packaging/icons/icon.icns").path]
try iconutil.run()
iconutil.waitUntilExit()
guard iconutil.terminationStatus == 0 else { fatalError("iconutil export failed") }
print("Exported packaging/icons/icon.icns")
