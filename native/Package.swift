// swift-tools-version: 5.9
// [LAYER: INFRASTRUCTURE]
import PackageDescription

let isMAS = Context.environment["HEAV3NS_MAS"] == "1"

let package = Package(
    name: "HEAV3NS-CODE",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "HEAV3NS-CODE", targets: ["Heav3nsCode"])],
    dependencies: isMAS ? [] : [
        .package(url: "https://github.com/sparkle-project/Sparkle", exact: "2.10.0")
    ],
    targets: [
        .executableTarget(
            name: "Heav3nsCode",
            dependencies: isMAS ? [] : [
                .product(name: "Sparkle", package: "Sparkle")
            ],
            resources: [.copy("Resources/Brand")],
            swiftSettings: isMAS ? [.define("MAC_APP_STORE")] : [],
            linkerSettings: [
                .unsafeFlags([
                    "-Xlinker", "-weak_framework", "-Xlinker", "FoundationModels",
                    "-Xlinker", "-weak_framework", "-Xlinker", "ImagePlayground"
                ])
            ]
        ),
        .testTarget(name: "Heav3nsCodeTests", dependencies: ["Heav3nsCode"])
    ]
)

