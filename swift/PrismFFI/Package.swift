// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "PrismFFI",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .library(
            name: "PrismFFI",
            targets: ["PrismFFI"]
        )
    ],
    targets: [
        .binaryTarget(
            name: "PrismCoreFFI",
            path: "PrismCoreFFI.xcframework"
        ),
        .target(
            name: "PrismFFI",
            dependencies: [
                "PrismCoreFFI"
            ],
            path: "Sources/PrismFFI",
            sources: [
                "prism_core.swift",
                "PrismCoreClient.swift"
            ],
            linkerSettings: [
                .linkedLibrary("z"),
                .linkedLibrary("iconv")
            ]
        ),
        .testTarget(
            name: "PrismFFITests",
            dependencies: ["PrismFFI"],
            path: "Tests/PrismFFITests"
        )
    ]
)
