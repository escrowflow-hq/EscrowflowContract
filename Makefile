CONTRACT := escrow
WASM_TARGET := wasm32-unknown-unknown
WASM_OUT := target/$(WASM_TARGET)/release/$(CONTRACT).wasm
WASM_OUT_OPT := target/$(WASM_TARGET)/release/$(CONTRACT).opt.wasm
STELLAR := stellar

# soroban-sdk 21.7.4 does not yet support the `reference-types` /
# `multivalue` wasm features that rustc enables by default from ~1.82
# onward. Scoped to the wasm build recipes only (not exported globally),
# since these flags don't apply to the host-target `test`/`lint` builds.
WASM_RUSTFLAGS := -C target-feature=-reference-types,-multivalue

NETWORK ?= testnet
SOURCE_ACCOUNT ?= default

.PHONY: all build build-opt test clean fmt fmt-check lint deploy-testnet

all: build

build:
	RUSTFLAGS="$(WASM_RUSTFLAGS)" cargo build --target $(WASM_TARGET) --release -p $(CONTRACT)

build-opt: build
	$(STELLAR) contract optimize --wasm $(WASM_OUT) --wasm-out $(WASM_OUT_OPT)

test:
	cargo test --workspace

clean:
	cargo clean

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	# `escrow`'s plain lib must be linted on its own (-p, not --workspace):
	# pulling in `test-utils` (which always needs soroban-sdk's testutils
	# feature) makes Cargo unify that feature onto escrow's lib build too,
	# tripping the testutils-only `extern crate std` guard in
	# contracts/escrow/src/lib.rs with no corresponding cfg(test) to catch
	# it. Tests are linted separately, where that guard is expected to fire.
	cargo clippy -p escrow --lib -- -D warnings
	cargo clippy -p test-utils --lib -- -D warnings
	cargo clippy --workspace --tests -- -D warnings

deploy-testnet: build-opt
	$(STELLAR) contract deploy \
		--wasm $(WASM_OUT_OPT) \
		--source $(SOURCE_ACCOUNT) \
		--network $(NETWORK)
