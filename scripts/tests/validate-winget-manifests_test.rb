require "fileutils"
require "minitest/autorun"
require "tmpdir"

require_relative "../validate-winget-manifests"

class ValidateWinGetManifestsTest < Minitest::Test
  VERSION = "0.1.0"
  MANIFESTS = File.expand_path("../../packaging/winget/0.1.0", __dir__)
  CHECKSUMS = {
    "madm-0.1.0-x86_64-pc-windows-msvc.zip" =>
      "97c101602d9e633b08abfd191be700f7ea22a8f351998434a9d9d9219cd6aa3c",
    "madm-0.1.0-aarch64-pc-windows-msvc.zip" =>
      "6f2c355950b442427a9a5d0c07475d8c988416d2ea08619d7fea9c519501a143"
  }.freeze

  def test_validates_seed_manifests
    with_checksums do |checksums|
      assert WinGetManifestValidator.validate(
        version: VERSION,
        manifests_path: MANIFESTS,
        checksums_path: checksums
      )
    end
  end

  def test_rejects_a_checksum_mismatch
    with_checksums(CHECKSUMS.merge(CHECKSUMS.keys.first => "0" * 64)) do |checksums|
      error = assert_raises(WinGetManifestValidator::ValidationError) do
        WinGetManifestValidator.validate(
          version: VERSION,
          manifests_path: MANIFESTS,
          checksums_path: checksums
        )
      end
      assert_match "InstallerSha256", error.message
    end
  end

  def test_rejects_an_incorrect_command_alias
    Dir.mktmpdir("winget-manifests") do |directory|
      FileUtils.cp_r("#{MANIFESTS}/.", directory)
      installer = File.join(directory, "monekoluv.madm.installer.yaml")
      File.write(installer, File.read(installer).sub("PortableCommandAlias: madm", "PortableCommandAlias: wrong"))

      with_checksums do |checksums|
        error = assert_raises(WinGetManifestValidator::ValidationError) do
          WinGetManifestValidator.validate(
            version: VERSION,
            manifests_path: directory,
            checksums_path: checksums
          )
        end
        assert_match "PortableCommandAlias", error.message
      end
    end
  end

  private

  def with_checksums(entries = CHECKSUMS)
    Dir.mktmpdir("winget-checksums") do |directory|
      path = File.join(directory, "SHA256SUMS")
      lines = entries.map { |name, digest| "#{digest}  #{name}" }
      File.write(path, "#{lines.join("\n")}\n")
      yield path
    end
  end
end
