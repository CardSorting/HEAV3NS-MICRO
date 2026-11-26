# [LAYER: UI]
cask "heav3ns-code" do
  version "16.4.1"
  sha256 :no_check

  url "https://github.com/CardSorting/HEAV3NS-CODE-Releases/releases/latest/download/HEAV3NS-CODE-#{version}-arm64.dmg",
      verified: "github.com/CardSorting/HEAV3NS-CODE-Releases/"
  name "HEAV3NS-CODE"
  desc "Native macOS AppKit IDE for the HEAV3NS-MICRO sovereign coding runtime"
  homepage "https://github.com/CardSorting/HEAV3NS-MICRO"

  depends_on macos: ">= :sonoma"
  depends_on arch: :arm64

  app "HEAV3NS-CODE.app"

  zap trash: [
    "~/Library/Application Support/io.heav3ns.code",
    "~/Library/Caches/io.heav3ns.code",
    "~/Library/Preferences/io.heav3ns.code.plist",
  ]
end
