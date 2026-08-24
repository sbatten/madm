require "minitest/autorun"
require "ripper"
require "tempfile"

require_relative "../render-homebrew-formula"

class RenderHomebrewFormulaTest < Minitest::Test
  TEMPLATE = File.expand_path("../../packaging/homebrew/madm.rb.erb", __dir__)
  VERSION = "1.2.3"

  def test_renders_each_supported_target
    with_checksums(checksum_lines) do |path|
      formula = render(path)

      HomebrewFormulaRenderer::TARGETS.each_with_index do |target, index|
        name = "madm-#{VERSION}-#{target}.tar.gz"
        assert_includes formula, "https://github.com/sbatten/madm/releases/download/v#{VERSION}/#{name}"
        assert_includes formula, format("%064x", index + 1)
      end
      refute_includes formula, "<%="
      refute_nil Ripper.sexp(formula)
    end
  end

  def test_rejects_a_missing_required_checksum
    with_checksums(checksum_lines.drop(1)) do |path|
      error = assert_raises(HomebrewFormulaRenderer::RenderError) { render(path) }
      assert_match "missing checksum", error.message
    end
  end

  def test_rejects_duplicate_checksum_entries
    lines = checksum_lines
    with_checksums(lines + [lines.first]) do |path|
      error = assert_raises(HomebrewFormulaRenderer::RenderError) { render(path) }
      assert_match "duplicate checksum", error.message
    end
  end

  def test_rejects_malformed_checksum_entries
    with_checksums(["not-a-checksum"]) do |path|
      error = assert_raises(HomebrewFormulaRenderer::RenderError) { render(path) }
      assert_match "malformed checksum", error.message
    end
  end

  private

  def checksum_lines
    HomebrewFormulaRenderer::TARGETS.each_with_index.map do |target, index|
      "#{format("%064x", index + 1)}  madm-#{VERSION}-#{target}.tar.gz"
    end
  end

  def render(checksums_path)
    HomebrewFormulaRenderer.render(
      version: VERSION,
      repository: "https://github.com/sbatten/madm",
      checksums_path: checksums_path,
      template_path: TEMPLATE
    )
  end

  def with_checksums(lines)
    Tempfile.create("SHA256SUMS") do |file|
      file.write("#{lines.join("\n")}\n")
      file.flush
      yield file.path
    end
  end
end
