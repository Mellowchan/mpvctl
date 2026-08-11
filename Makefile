PREFIX ?= /usr

all:
	@echo RUN \'make install\' to install mpvctl

install:
	@install -Dm755 mpvctl $(DESTDIR)$(PREFIX)/bin/mpvctl

uninstall:
	@rm -f $(DESTDIR)$(PREFIX)/bin/mpvctl
