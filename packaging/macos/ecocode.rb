# Homebrew formula for the ecocode CLI (builds from source).
#
# To publish: tag a release (e.g. v1.0.0), fill in `url` and `sha256`
# (`curl -L <url> | shasum -a 256`), and add this file to a tap repository
# (github.com/<owner>/homebrew-ecocode → `brew install <owner>/ecocode/ecocode`).
# Until then: `brew install --HEAD --formula packaging/macos/ecocode.rb`.
class Ecocode < Formula
  desc "Measure the energy and carbon footprint of programs and ML training runs"
  homepage "https://github.com/uri-airi-lab/EcoCode"
  url "https://github.com/uri-airi-lab/EcoCode/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "REPLACE_WITH_RELEASE_TARBALL_SHA256"
  license "AGPL-3.0-only"
  head "https://github.com/uri-airi-lab/EcoCode.git", branch: "main"

  # Needs Rust 1.99 or newer (see rust-toolchain.toml).
  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/ecocode-cli")
  end

  test do
    assert_match "ecocode", shell_output("#{bin}/ecocode --version")
    system bin/"ecocode", "doctor"
    system bin/"ecocode", "run", "--sensors", "fake", "-o", "json", "-f", testpath/"out.json", "-i", "0.2", "--", "sleep", "1"
    assert_path_exists testpath/"out.json"
  end
end
