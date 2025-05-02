PREFIX ?= /usr

all:
	@echo RUN \'make install\' to install mpvctl

install:
	@install -Dm755 mpvctl $(DESTDIR)$(PREFIX)/bin/mpvctl
	@install -Dm755 mpvd $(DESTDIR)$(PREFIX)/bin/mpvd

uninstall:
	@rm -f $(DESTDIR)$(PREFIX)/bin/mpvctl
	@rm -f $(DESTDIR)$(PREFIX)/bin/mpvd

