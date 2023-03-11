

.PHONY: install-gpt-cli
install-gpt-cli:
	cargo install --path . --no-default-features --bin gpt-cli


.PHONY: uninstall-gpt-cli
uninstall-gpt-cli:
	rm $(which gpt-cli)
