

.PHONY: install-gpt-cli
install-gpt-cli:
	cargo install --path . --no-default-features --bin gpt-cli
