// swift-tools-version: 5.9

import PackageDescription

let package = Package(
  name: "PrismApp",
  platforms: [
    .macOS(.v13)
  ],
  products: [
    .executable(name: "PrismApp", targets: ["PrismApp"])
  ],
  dependencies: [
    .package(path: "../PrismFFI")
  ],
  targets: [
    .executableTarget(
      name: "PrismApp",
      dependencies: [
        .product(name: "PrismFFI", package: "PrismFFI")
      ],
      path: "Sources"
    ),
    .testTarget(
      name: "PrismAppTests",
      dependencies: ["PrismApp"],
      path: "Tests"
    )
  ]
)
