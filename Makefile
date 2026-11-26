# Checks of this mod. The WebAssembly build and the shipping are craftorio-launcher's `make horizon-mods`.
.PHONY: hooks check
hooks:
	git config core.hooksPath .githooks

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test
