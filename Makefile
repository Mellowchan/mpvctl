PREFIX ?= /usr

all:
	cargo build --release

install: all
	@install -Dm755 target/release/mpvctl $(DESTDIR)$(PREFIX)/bin/mpvctl

uninstall:
	@rm -f $(DESTDIR)$(PREFIX)/bin/mpvctl
