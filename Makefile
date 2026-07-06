.DEFAULT_GOAL := build

.PHONY: build
build:
	cargo build --profile debug-release --target thumbv6m-none-eabi
	picotool uf2 convert -t elf target/thumbv6m-none-eabi/debug-release/crsf2pwm target/thumbv6m-none-eabi/debug-release/crsf2pwm.uf2

.PHONY: build
run:
	cargo run --profile debug-release --target thumbv6m-none-eabi
