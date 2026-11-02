.PHONY: run check build install dist

PREFIX ?= $(HOME)/.local
DATA_HOME := $(if $(XDG_DATA_HOME),$(XDG_DATA_HOME),$(HOME)/.local/share)
ARCH := $(shell uname -m)
DIST_NAME := focus-linux-$(ARCH)

run:
	cargo run

check:
	cargo fmt --all -- --check
	cargo test
	cargo clippy --all-targets -- -D warnings

build:
	cargo build --release

install: build
	install -Dm755 target/release/focus "$(DESTDIR)$(PREFIX)/bin/focus"
	install -Dm644 packaging/io.github.focus.Focus.desktop "$(DESTDIR)$(DATA_HOME)/applications/io.github.focus.Focus.desktop"
	install -Dm644 assets/focus.svg "$(DESTDIR)$(DATA_HOME)/icons/hicolor/scalable/apps/io.github.focus.Focus.svg"

dist: build
	mkdir -p "target/dist/$(DIST_NAME)"
	install -m755 target/release/focus "target/dist/$(DIST_NAME)/focus"
	cp README.md packaging/io.github.focus.Focus.desktop assets/focus.svg "target/dist/$(DIST_NAME)/"
	tar -C target/dist -czf "target/dist/$(DIST_NAME).tar.gz" "$(DIST_NAME)"
