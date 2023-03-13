DEB_BUILD_PATH ?= target/debian/chatty_smart_home*.deb

.PHONY: install-gpt-cli
install-gpt-cli:
	cargo install --path . --no-default-features --bin gpt-cli


.PHONY: uninstall-gpt-cli
uninstall-gpt-cli:
	rm $(which gpt-cli)

# installing server
.PHONY: build-chatty-smart-home
build-chatty-smart-home:
	cargo build --release --bin chatty_smart_home
	cargo deb --no-build

.PHONE: install-chatty-smart-home
install-chatty-smart-home: build-chatty-smart-home
	sudo dpkg -i $(DEB_BUILD_PATH)

.PHONY: install-dependencies-chatty-smart-home
install-dependencies-chatty-smart-home:
	cargo install cargo-deb cargo-get
