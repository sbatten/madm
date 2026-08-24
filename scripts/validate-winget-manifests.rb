#!/usr/bin/env ruby

require "optparse"
require "yaml"

require_relative "render-homebrew-formula"

module WinGetManifestValidator
  PACKAGE_IDENTIFIER = "monekoluv.madm".freeze
  PUBLISHER = "monekoluv".freeze
  REPOSITORY = "https://github.com/sbatten/madm".freeze
  TARGETS = {
    "x64" => "x86_64-pc-windows-msvc",
    "arm64" => "aarch64-pc-windows-msvc"
  }.freeze

  class ValidationError < StandardError; end

  module_function

  def validate(version:, manifests_path:, checksums_path:)
    documents = load_documents(manifests_path)
    version_manifest = fetch_manifest(documents, "version")
    installer_manifest = fetch_manifest(documents, "installer")
    locale_manifest = fetch_manifest(documents, "defaultLocale")

    documents.each do |document|
      expect(document["PackageIdentifier"], PACKAGE_IDENTIFIER, "PackageIdentifier")
      expect(document["PackageVersion"].to_s, version, "PackageVersion")
    end
    expect(version_manifest["DefaultLocale"], "en-US", "DefaultLocale")
    expect(locale_manifest["PackageLocale"], "en-US", "PackageLocale")
    expect(locale_manifest["Publisher"], PUBLISHER, "Publisher")
    expect(locale_manifest["PackageName"], "madm", "PackageName")
    expect(locale_manifest["PackageUrl"], REPOSITORY, "PackageUrl")
    expect(locale_manifest["PublisherSupportUrl"], "#{REPOSITORY}/issues", "PublisherSupportUrl")
    expect(locale_manifest["License"], "MIT", "License")
    expect(
      locale_manifest["ReleaseNotesUrl"],
      "#{REPOSITORY}/releases/tag/v#{version}",
      "ReleaseNotesUrl"
    )

    expect(installer_manifest["InstallerType"], "zip", "InstallerType")
    expect(installer_manifest["NestedInstallerType"], "portable", "NestedInstallerType")
    expect(installer_manifest["UpgradeBehavior"], "uninstallPrevious", "UpgradeBehavior")
    unless Array(installer_manifest["Commands"]).include?("madm")
      raise ValidationError, "Commands must include madm"
    end

    checksums = HomebrewFormulaRenderer.parse_checksums(checksums_path)
    installers = Array(installer_manifest["Installers"])
    expect(installers.length, TARGETS.length, "installer count")

    TARGETS.each do |architecture, target|
      installer = installers.find { |candidate| candidate["Architecture"] == architecture }
      raise ValidationError, "missing #{architecture} installer" unless installer

      name = "madm-#{version}-#{target}.zip"
      expect(installer["InstallerUrl"], "#{REPOSITORY}/releases/download/v#{version}/#{name}", "#{architecture} InstallerUrl")
      expected_checksum = checksums.fetch(name) do
        raise ValidationError, "missing checksum for #{name}"
      end
      expect(installer["InstallerSha256"].to_s.downcase, expected_checksum, "#{architecture} InstallerSha256")

      nested_files = Array(installer["NestedInstallerFiles"])
      expect(nested_files.length, 1, "#{architecture} nested installer count")
      nested = nested_files.first
      expect(
        nested["RelativeFilePath"],
        "madm-#{version}-#{target}\\madm.exe",
        "#{architecture} RelativeFilePath"
      )
      expect(nested["PortableCommandAlias"], "madm", "#{architecture} PortableCommandAlias")
    end

    true
  rescue KeyError => error
    raise ValidationError, error.message
  end

  def load_documents(path)
    files = Dir[File.join(path, "*.yaml")]
    raise ValidationError, "no YAML manifests found in #{path}" if files.empty?

    files.map do |file|
      document = YAML.safe_load(File.read(file))
      unless document.is_a?(Hash)
        raise ValidationError, "manifest #{file} must contain a YAML mapping"
      end
      document
    rescue Psych::SyntaxError => error
      raise ValidationError, "invalid YAML in #{file}: #{error.message}"
    end
  end

  def fetch_manifest(documents, type)
    matches = documents.select { |document| document["ManifestType"] == type }
    unless matches.length == 1
      raise ValidationError, "expected exactly one #{type} manifest, found #{matches.length}"
    end
    matches.first
  end

  def expect(actual, expected, field)
    return if actual == expected

    raise ValidationError, "#{field} must be #{expected.inspect}, got #{actual.inspect}"
  end
end

if $PROGRAM_NAME == __FILE__
  options = {}
  parser = OptionParser.new do |args|
    args.banner = "Usage: validate-winget-manifests.rb [options]"
    args.on("--version VERSION", "Release version") { |value| options[:version] = value }
    args.on("--manifests PATH", "Manifest directory") { |value| options[:manifests_path] = value }
    args.on("--checksums PATH", "SHA256SUMS file") { |value| options[:checksums_path] = value }
  end
  parser.parse!

  required = %i[version manifests_path checksums_path]
  missing = required.reject { |key| options.key?(key) }
  parser.abort("Missing required options: #{missing.join(", ")}") unless missing.empty?

  begin
    WinGetManifestValidator.validate(
      version: options.fetch(:version),
      manifests_path: options.fetch(:manifests_path),
      checksums_path: options.fetch(:checksums_path)
    )
    puts "Validated WinGet manifests for #{options.fetch(:version)}."
  rescue WinGetManifestValidator::ValidationError, HomebrewFormulaRenderer::RenderError, SystemCallError => error
    warn error.message
    exit 1
  end
end
