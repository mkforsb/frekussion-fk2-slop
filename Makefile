# Convenience targets. Requires the Dioxus CLI (`dx`, 0.7.x) and the
# wasm32-unknown-unknown target (`rustup target add wasm32-unknown-unknown`).

APP := crates/app

.PHONY: web desktop serve-web run-desktop test lint renders bench clean

## Serve the web build with hot reload on http://127.0.0.1:8080
serve-web:
	cd $(APP) && dx serve --web --release

## Run the native Linux app (PulseAudio / PipeWire-pulse)
run-desktop:
	cd $(APP) && dx serve --desktop --release

## Release bundles: target/dx/frekussion/release/{web/public,linux/app}
web:
	cd $(APP) && dx build --web --release

desktop:
	cd $(APP) && dx build --desktop --release

test:
	cargo test --release -p frekussion-dsp -p frekussion

lint:
	cargo fmt --all --check
	cargo clippy --all-targets -p frekussion-dsp -p frekussion-worklet -- -D warnings
	cargo clippy --all-targets -p frekussion --features desktop -- -D warnings
	cargo clippy -p frekussion --features web --target wasm32-unknown-unknown -- -D warnings

## Render every factory preset and kit to WAV files in ./renders
renders:
	cargo run --release -p frekussion-dsp --example render -- renders

bench:
	cargo run --release -p frekussion-dsp --example bench

clean:
	cargo clean
