#!/usr/bin/env bash
set -euo pipefail

working_directory="${1:?expected gem working directory}"

cc_versions="$(cd "$working_directory" && cibuildgem print_ruby_cc_version)"
normalized_platform="$(cd "$working_directory" && cibuildgem print_normalized_platform)"
current_ruby_version="$(ruby -e 'print RUBY_VERSION')"

runner_temp="${RUNNER_TEMP:-/tmp}"
rubies_dir="${runner_temp}/rubies"
config_dir="${HOME}/.rake-compiler"
config_path="${config_dir}/config.yml"

mkdir -p "${rubies_dir}" "${config_dir}"
: > "${config_path}"

host_os="$(uname -s)"
host_arch="$(uname -m)"

case "${host_os}" in
  Darwin) asset_platform="darwin" ;;
  Linux) asset_platform="ubuntu-22.04" ;;
  *)
    echo "Unsupported host OS: ${host_os}" >&2
    exit 1
    ;;
esac

case "${host_arch}" in
  x86_64) asset_arch="x64" ;;
  arm64|aarch64) asset_arch="arm64" ;;
  *)
    echo "Unsupported host architecture: ${host_arch}" >&2
    exit 1
    ;;
esac

IFS=':' read -r -a ruby_versions <<< "${cc_versions}"
for ruby_version in "${ruby_versions[@]}"; do
  archive_path="${runner_temp}/ruby-${ruby_version}-${asset_platform}-${asset_arch}.tar.gz"
  extract_path="${rubies_dir}/${ruby_version}"
  download_url="https://github.com/ruby/ruby-builder/releases/download/ruby-${ruby_version}/ruby-${ruby_version}-${asset_platform}-${asset_arch}.tar.gz"

  mkdir -p "${extract_path}"
  curl --fail --silent --show-error --location --retry 5 \
    --output "${archive_path}" \
    "${download_url}"
  tar -xzf "${archive_path}" -C "${extract_path}"

  rbconfig_path="$(find "${extract_path}" -path '*/lib/ruby/*/*/rbconfig.rb' -print -quit)"
  if [[ -z "${rbconfig_path}" ]]; then
    echo "Could not find rbconfig.rb for Ruby ${ruby_version}" >&2
    exit 1
  fi

  if [[ "${ruby_version}" != "${current_ruby_version}" ]]; then
    printf 'rbconfig-%s-%s: %s\n' "${normalized_platform}" "${ruby_version}" "${rbconfig_path}" >> "${config_path}"
  fi
done
