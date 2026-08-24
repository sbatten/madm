#!/usr/bin/env ruby

require "erb"
require "fileutils"
require "optparse"

module HomebrewFormulaRenderer
  TARGETS = %w[
    aarch64-apple-darwin
    x86_64-apple-darwin
    aarch64-unknown-linux-musl
    x86_64-unknown-linux-musl
  ].freeze

  class RenderError < StandardError; end

  module_function

  def render(version:, repository:, checksums_path:, template_path:)
    validate_version(version)
    checksums_by_file = parse_checksums(checksums_path)
    checksums = {}
    urls = {}

    TARGETS.each do |target|
      name = "madm-#{version}-#{target}.tar.gz"
      checksums[target] = checksums_by_file.fetch(name) do
        raise RenderError, "missing checksum for #{name}"
      end
      urls[target] = "#{repository}/releases/download/v#{version}/#{name}"
    end

    template = ERB.new(File.read(template_path), trim_mode: "-")
    template.result_with_hash(
      version: version,
      checksums: checksums,
      urls: urls
    )
  end

  def parse_checksums(path)
    checksums = {}
    File.foreach(path, chomp: true).with_index(1) do |line, line_number|
      match = line.match(/\A([0-9a-fA-F]{64}) [ *](\S+)\z/)
      unless match
        raise RenderError, "malformed checksum at #{path}:#{line_number}"
      end

      digest = match[1].downcase
      name = match[2]
      if checksums.key?(name)
        raise RenderError, "duplicate checksum for #{name}"
      end
      checksums[name] = digest
    end
    checksums
  end

  def validate_version(version)
    return if version.match?(/\A[0-9A-Za-z][0-9A-Za-z.+-]*\z/)

    raise RenderError, "invalid package version #{version.inspect}"
  end
end

if $PROGRAM_NAME == __FILE__
  options = {
    repository: "https://github.com/sbatten/madm"
  }
  parser = OptionParser.new do |args|
    args.banner = "Usage: render-homebrew-formula.rb [options]"
    args.on("--version VERSION", "Release version") { |value| options[:version] = value }
    args.on("--repository URL", "GitHub repository URL") { |value| options[:repository] = value }
    args.on("--checksums PATH", "SHA256SUMS file") { |value| options[:checksums_path] = value }
    args.on("--template PATH", "Formula ERB template") { |value| options[:template_path] = value }
    args.on("--output PATH", "Rendered formula path") { |value| options[:output_path] = value }
  end
  parser.parse!

  required = %i[version checksums_path template_path output_path]
  missing = required.reject { |key| options.key?(key) }
  parser.abort("Missing required options: #{missing.join(", ")}") unless missing.empty?

  begin
    formula = HomebrewFormulaRenderer.render(
      version: options.fetch(:version),
      repository: options.fetch(:repository),
      checksums_path: options.fetch(:checksums_path),
      template_path: options.fetch(:template_path)
    )
    FileUtils.mkdir_p(File.dirname(options.fetch(:output_path)))
    File.write(options.fetch(:output_path), formula)
  rescue HomebrewFormulaRenderer::RenderError, KeyError, SystemCallError => error
    warn error.message
    exit 1
  end
end
