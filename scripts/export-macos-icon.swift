#!/usr/bin/env swift
// Add the legacy macOS icon canvas to an Icon Composer macOS export.
import AppKit

let args = CommandLine.arguments
guard args.count == 3,
       let source = NSImage(contentsOfFile: args[1]),
       let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: 1024, pixelsHigh: 1024,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
        isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0),
       let context = NSGraphicsContext(bitmapImageRep: bitmap)
else { fatalError("Usage: export-macos-icon.swift INPUT.png OUTPUT.png") }
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = context
context.imageInterpolation = .high
NSColor.clear.setFill()
NSRect(x: 0, y: 0, width: 1024, height: 1024).fill()
// Standard macOS artwork footprint: 824px in a 1024px transparent canvas.
source.draw(in: NSRect(x: 100, y: 100, width: 824, height: 824),
            from: .zero, operation: .copy, fraction: 1)
context.flushGraphics()
NSGraphicsContext.restoreGraphicsState()
guard let png = bitmap.representation(using: .png, properties: [:]) else {
    fatalError("Unable to encode the macOS icon")
}
try png.write(to: URL(fileURLWithPath: args[2]))
