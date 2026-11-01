PREFIX ?= $(HOME)/.local

.PHONY: install
install:
	cargo build --release
	install -d $(PREFIX)/bin
	install -m 755 target/release/world-txt $(PREFIX)/bin/world.txt
