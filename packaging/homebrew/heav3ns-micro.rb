# [LAYER: UI]
class Heav3nsMicro < Formula
  desc "Hyper-compressed Rust micro agent runtime with Joyzoning sovereignty"
  homepage "https://github.com/CardSorting/HEAV3NS-MICRO"
  url "https://github.com/CardSorting/HEAV3NS-MICRO/archive/refs/tags/v1.0.0.tar.gz"
  license "Apache-2.0"
  head "https://github.com/CardSorting/HEAV3NS-MICRO.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "HEAV3NS-MICRO", shell_output("#{bin}/heav3ns-micro --help")
  end
end
