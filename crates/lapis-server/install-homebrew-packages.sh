#!/bin/sh
set -eu

brew=/home/linuxbrew/.linuxbrew/bin/brew
if [ ! -x "$brew" ]; then
  echo "Homebrew is unavailable at /home/linuxbrew/.linuxbrew; use a named volume so Docker can seed the image contents." >&2
  exit 70
fi

install_packages() {
  kind=$1
  packages=$2
  [ -n "$packages" ] || return 0

  set -f
  for package in $packages; do
    case "$package" in
      -* | /* | .* | */ | */.* | *//* | */*/*/* | *[!A-Za-z0-9@._+/-]*)
        echo "Invalid Homebrew $kind package name: $package" >&2
        exit 64
        ;;
    esac
    if [ "$kind" = formula ]; then
      "$brew" install --force-bottle "$package"
    else
      "$brew" install --cask "$package"
    fi
  done
  set +f
}

install_packages formula "${LAPIS_SERVER_HOMEBREW_PACKAGES:-}"
install_packages cask "${LAPIS_SERVER_HOMEBREW_CASKS:-}"
