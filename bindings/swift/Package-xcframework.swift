// swift-tools-version:5.9
// Alternative Package.swift that uses XCFramework instead of direct library linking
// Use this if you've built GraphLiteFFI.xcframework for iOS support
//
// To use:
//   1. Build XCFramework: ./build-ios.sh
//   2. Rename this file: mv Package-xcframework.swift Package.swift
//   3. Build: swift build

import PackageDescription

let package = Package(
    name: "GraphLite",
    platforms: [
        .macOS(.v10_15),
        .iOS(.v13),
    ],
    products: [
        .library(
            name: "GraphLite",
            targets: ["GraphLite"]
        ),
    ],
    targets: [
        // Binary XCFramework target
        .binaryTarget(
            name: "GraphLiteFFI",
            path: "GraphLiteFFI.xcframework"
        ),

        // C module wrapper
        .target(
            name: "CGraphLite",
            dependencies: ["GraphLiteFFI"],
            path: "Sources/CGraphLite"
        ),

        // Swift wrapper
        .target(
            name: "GraphLite",
            dependencies: ["CGraphLite"],
            path: "Sources/GraphLite"
        ),

        // Tests
        .testTarget(
            name: "GraphLiteTests",
            dependencies: ["GraphLite"],
            path: "Tests/GraphLiteTests"
        ),
    ]
)
