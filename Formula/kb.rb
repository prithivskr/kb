class Kb < Formula
  desc "Terminal Kanban board"
  homepage "https://github.com/prithivskr/kb"
  head "https://github.com/prithivskr/kb.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match(/\A\d+\.\d+\.\d+\n\z/, shell_output("#{bin}/kb --version"))
  end
end
