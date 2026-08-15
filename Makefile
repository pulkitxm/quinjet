CARGO ?= cargo
NIGHTLY ?= nightly
COVERAGE_MIN ?= 55

.PHONY: ci ci-fast fmt fmt-check lint lint-nightly test doc msrv build package install-check \
        comments secrets typos deny audit unused hack coverage shell actions yaml markdown toml \
        editorconfig ruff tools

ci: fmt-check lint test doc comments secrets typos deny audit unused hack shell actions yaml markdown toml editorconfig ruff package

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
	$(CARGO) +1.85 check --all-targets --all-features --locked

build:
	$(CARGO) build --release --locked

package:
	$(CARGO) package --locked

install-check: build
	$(CARGO) install --path . --locked --root /tmp/quinjet-install
	/tmp/quinjet-install/bin/quinjet --version
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
	$(CARGO) audit --deny warnings

unused:
	$(CARGO) machete --with-metadata

hack:
	$(CARGO) hack --feature-powerset --no-dev-deps check --locked

coverage:
	$(CARGO) llvm-cov --all-features --locked --fail-under-lines $(COVERAGE_MIN)

shell:
	shellcheck --severity=style --enable=all install.sh tests/install.sh
	shfmt --diff --indent 2 --case-indent install.sh tests/install.sh

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
	ruff check --select ALL --ignore D203,D213,COM812,ISC001 scripts
	ruff format --check scripts

tools:
	$(CARGO) install cargo-deny cargo-audit cargo-machete cargo-hack cargo-llvm-cov typos-cli taplo-cli
