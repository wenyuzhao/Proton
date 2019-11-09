target := aarch64-proton
image := kernel8.img
raspi ?= 3
profile = $(if $(release),release,debug)
debug_interrupts = $(if $(dint),-d int)

kernel: init
	@cargo xbuild $(if $(release), --release) --target $(target).json --features raspi$(raspi)
	@cargo objcopy -- ./target/$(target)/$(profile)/proton -O binary ./kernel8.img

init: FORCE
	@cd ./init && cargo xbuild $(if $(release), --release) --target $(target).json
	@mkdir -p ./target
	@cp ./init/target/$(target)/$(profile)/init ./target/init

run: kernel kernel8.img objdump
	@qemu-system-aarch64 -M raspi3 -serial stdio -kernel ./kernel8.img $(debug_interrupts)

objdump:
	@llvm-objdump -d ./target/$(target)/$(profile)/proton > kernel.S
	@llvm-objdump -d ./init/target/$(target)/$(profile)/init > init.S

debug: kernel kernel8.img objdump
	@qemu-system-aarch64 -M raspi3 -serial stdio -kernel ./kernel8.img $(debug_interrupts) -s -S

gdb:
	@gdb-multiarch -quiet "target/aarch64-proton/$(profile)/proton" -ex "set arch aarch64" -ex "target remote :1234"

clean:
	@cargo clean
	@cd init && cargo clean
	@rm ./Cargo.lock ./kernel.S ./kernel8.img

FORCE: ;