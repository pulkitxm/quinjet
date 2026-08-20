CARGO ?= cargo
NIGHTLY ?= nightly
COVERAGE_MIN ?= 65

.PHONY: ci ci-fast deep fmt fmt-check lint lint-nightly test doc msrv build package \
	install-check comments secrets typos spellcheck deny audit osv sbom unused sort hack wiki \
	coverage shell actions yaml markdown toml editorconfig ruff miri careful sanitize mutants \
	minimal-versions udeps bloat optimization-docs tools tools-deep

ci: fmt-check lint test doc comments secrets typos spellcheck deny audit osv unused sort hack \
	shell actions yaml markdown toml editorconfig ruff wiki package

ci-fast: fmt-check lint test comments secrets

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

lint:
	$(CARGO) clippy --all-targets --all-features --locked -- -D warnings

lint-nightly:
	$(CARGO) +$(NIGHTLY) clippy --all-targets --all-features -- -D warnings

test:
	$(CARGO) test --all-features --locked
	$(CARGO) test --no-default-features --locked

doc:
	RUSTDOCFLAGS="-D warnings -D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links" \
		$(CARGO) doc --no-deps --all-features --document-private-items --locked

msrv:
	$(CARGO) +1.88 check --all-targets --all-features --locked

build:
	$(CARGO) build --release --locked

package:
	$(CARGO) package --locked

install-check: build
	rm -rf /tmp/quinjet-install
	$(CARGO) install --path . --locked --root /tmp/quinjet-install
	HOME=/tmp/quinjet-install/home XDG_DATA_HOME=/tmp/quinjet-install/data SHELL=/bin/bash \
		PATH=/tmp/quinjet-install/bin:$$PATH /tmp/quinjet-install/bin/quinjet --version
	test -s /tmp/quinjet-install/data/bash-completion/completions/quinjet
	test -L /tmp/quinjet-install/bin/q
	HOME=/tmp/quinjet-install/home XDG_DATA_HOME=/tmp/quinjet-install/data SHELL=/bin/bash \
		PATH=/tmp/quinjet-install/bin:$$PATH q --version
	test -s /tmp/quinjet-install/home/.local/state/quinjet/bash-installed
	test -s /tmp/quinjet-install/home/.local/state/quinjet/shortcut-installed
	HOME=/tmp/quinjet-install/home XDG_DATA_HOME=/tmp/quinjet-install/data SHELL=/bin/bash \
		/tmp/quinjet-install/bin/quinjet --help >/dev/null

comments:
	python3 scripts/check_comments.py --selftest
	python3 scripts/check_comments.py

secrets:
	python3 scripts/check_secrets.py --selftest
	python3 scripts/check_secrets.py

typos:
	typos

deny:
	$(CARGO) deny --all-features check

audit:
	$(CARGO) audit --deny warnings --ignore RUSTSEC-2024-0320 --ignore RUSTSEC-2025-0141

unused:
	$(CARGO) machete --with-metadata
	$(CARGO) shear

sort:
	$(CARGO) sort --check --check-format

spellcheck:
	$(CARGO) spellcheck --code 1 check

sbom:
	$(CARGO) cyclonedx --format json --all-features

osv:
	osv-scanner scan source --config osv-scanner.toml --recursive .

miri:
	MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-strict-provenance" $(CARGO) +$(NIGHTLY) miri test --all-features

careful:
	$(CARGO) +$(NIGHTLY) careful test --all-features

sanitize:
	RUSTFLAGS="-Zsanitizer=address" $(CARGO) +$(NIGHTLY) test --all-features \
		--target x86_64-unknown-linux-gnu -Zbuild-std

mutants:
	$(CARGO) mutants --no-shuffle --in-place --timeout 120

minimal-versions:
	$(CARGO) minimal-versions check --all-targets --all-features

udeps:
	$(CARGO) +$(NIGHTLY) udeps --all-targets --all-features

bloat:
	$(CARGO) bloat --release --crates -n 30

deep: miri careful sanitize mutants minimal-versions udeps bloat

hack:
	$(CARGO) hack --feature-powerset --no-dev-deps check --locked

coverage:
	$(CARGO) llvm-cov --all-features --locked --fail-under-lines $(COVERAGE_MIN)

shell:
	shellcheck --severity=style --enable=all install.sh tests/install.sh
	shfmt --diff --indent 4 --case-indent install.sh tests/install.sh

actions:
	actionlint
	zizmor --persona=pedantic .github/workflows

yaml:
	yamllint --strict .

markdown:
	markdownlint-cli2

toml:
	taplo fmt --check --diff
	taplo lint

editorconfig:
	editorconfig-checker

ruff:
	ruff check scripts
	ruff format --check scripts

optimization-docs:
	python3 scripts/check_optimization_docs.py --check

wiki: optimization-docs
	python3 scripts/sync_wiki.py --check

tools:
	$(CARGO) install cargo-deny cargo-audit cargo-machete cargo-shear cargo-hack cargo-llvm-cov \
		cargo-nextest cargo-sort cargo-spellcheck cargo-cyclonedx typos-cli taplo-cli

tools-deep:
	$(CARGO) install cargo-mutants cargo-careful cargo-minimal-versions cargo-udeps cargo-bloat \
		cargo-outdated cargo-msrv osv-scanner
