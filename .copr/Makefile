# Copr "make srpm" hook (F-63). Copr runs: make -f .copr/Makefile srpm outdir=... spec=...
# Builds a source tarball with vendored crates and node deps are NOT vendored: see installer.spec.
outdir ?= .
spec ?= .copr/installer.spec

srpm:
	dnf install -y --setopt=install_weak_deps=False rpm-build git nodejs npm cargo
	git archive --format=tar.gz --prefix=installer-$(shell sed -n 's/^version *= *"\(.*\)"/\1/p' src-tauri/Cargo.toml | head -n1)/ \
	  -o $(outdir)/installer-$(shell sed -n 's/^version *= *"\(.*\)"/\1/p' src-tauri/Cargo.toml | head -n1).tar.gz HEAD
	rpmbuild -bs $(spec) --define "_sourcedir $(outdir)" --define "_srcrpmdir $(outdir)"
