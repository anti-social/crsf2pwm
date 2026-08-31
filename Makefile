.DEFAULT_GOAL := build

mcu = rp2040
ifeq ($(mcu),rp2040)
build_target = thumbv6m-none-eabi
else
build_target = thumbv8m.main-none-eabihf
endif

build_opts = --profile debug-release --target $(build_target) --features $(mcu)

.PHONY: build
build:
	cargo build $(build_opts)
	cd target/$(build_target)/debug-release && \
    picotool uf2 convert -t elf crsf2pwm crsf2pwm.uf2

.PHONY: build
run:
	cargo run $(build_opts)
