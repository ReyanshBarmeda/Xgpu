TARGET      := x86_64-xdos
BUILD       := build
KERNEL      := $(BUILD)/xdos.bin
ISO         := $(BUILD)/xdos.iso
RUST_LIB    := target/$(TARGET)/release/libxdos.a

.PHONY: all kernel iso run run-serial debug clean rustlib

all: iso

$(BUILD):
	mkdir -p $(BUILD)

$(BUILD)/boot.o: boot/boot.S | $(BUILD)
	as --64 $< -o $@

$(BUILD)/long_mode_start.o: boot/long_mode_start.S | $(BUILD)
	as --64 $< -o $@

$(BUILD)/context_switch.o: boot/context_switch.S | $(BUILD)
	as --64 $< -o $@

$(BUILD)/syscall_entry.o: boot/syscall_entry.S | $(BUILD)
	as --64 $< -o $@

$(BUILD)/usermode_entry.o: boot/usermode_entry.S | $(BUILD)
	as --64 $< -o $@

# Defer staleness checking to cargo rather than listing sources here.
rustlib:
	cargo build --release

kernel: $(BUILD)/boot.o $(BUILD)/long_mode_start.o $(BUILD)/context_switch.o $(BUILD)/syscall_entry.o $(BUILD)/usermode_entry.o rustlib
	ld -n -z noexecstack -T boot/linker.ld -o $(KERNEL) \
		$(BUILD)/boot.o $(BUILD)/long_mode_start.o $(BUILD)/context_switch.o \
		$(BUILD)/syscall_entry.o $(BUILD)/usermode_entry.o $(RUST_LIB)

iso: kernel
	mkdir -p $(BUILD)/isofiles/boot/grub
	cp $(KERNEL) $(BUILD)/isofiles/boot/xdos.bin
	cp grub.cfg $(BUILD)/isofiles/boot/grub/grub.cfg
	grub-mkrescue -o $(ISO) $(BUILD)/isofiles 2> /dev/null

# Serial output appears in your terminal; VGA output in the QEMU window.
run: iso
	qemu-system-x86_64 -cdrom $(ISO) -serial stdio

# No graphical window; everything over serial. Handy over SSH.
run-serial: iso
	qemu-system-x86_64 -cdrom $(ISO) -nographic

# Wait for gdb to attach on :1234, then `target remote :1234` in gdb.
debug: iso
	qemu-system-x86_64 -cdrom $(ISO) -serial stdio -s -S

clean:
	rm -rf $(BUILD) target
