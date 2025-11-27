// swift-tools-version:5.9
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
        // Binary XCFramework containing the Rust FFI library
        .binaryTarget(
            name: "GraphLiteFFI",
            path: "GraphLiteFFI.xcframework"
        ),
        // C module wrapper for FFI
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
