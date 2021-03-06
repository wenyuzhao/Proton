use super::*;
use cortex_a::{asm, regs::*, barrier};
use super::uart::boot_time_log;
use proton_kernel::*;

pub static mut BOOTED: bool = false;

#[no_mangle]
#[naked]
pub unsafe fn _start() -> ! {
    // Halt non-promary processors
    llvm_asm! {"
            mrs     x0, mpidr_el1
            and     x0, x0, #3
            cbz     x0, 2f
        1:  wfe
            b       1b
        2:
    "};
    // Setup core 0 stack
    llvm_asm!("mov sp, $0"::"r"(0x80000));
    
    super::uart::UART0::init();
    assert!(CurrentEL.get() == CurrentEL::EL::EL2.value);
    boot_time_log("[boot...]");
    CNTHCTL_EL2.write(CNTHCTL_EL2::EL1PCEN::SET + CNTHCTL_EL2::EL1PCTEN::SET);
    CNTVOFF_EL2.set(0);
    // Switch to EL1
    boot_time_log("[boot: switch to exception level 1...]");
    SCTLR_EL1.set((3 << 28) | (3 << 22) | (1 << 20) | (1 << 11)); // Disable MMU
    HCR_EL2.write(HCR_EL2::RW::EL1IsAarch64); // Set execution mode = AArch64
    SPSR_EL2.write(SPSR_EL2::D::Masked + SPSR_EL2::A::Masked + SPSR_EL2::I::Masked + SPSR_EL2::F::Masked + SPSR_EL2::M::EL1h);
    ELR_EL2.set(_start_el1 as *const () as u64); // EL1 PC after return from `eret`
    SP_EL1.set(0x80000); // EL1 stack
    asm::eret();
}


extern {
    static mut __bss_start: usize;
    static mut __bss_end: usize;
}

#[inline(never)]
unsafe fn zero_bss() {
    let start = (&mut __bss_start as *mut usize as usize & 0x0000ffff_ffffffff) as *mut u8;
    let end = (&mut __bss_end as *mut usize as usize & 0x0000ffff_ffffffff) as *mut u8;
    let mut cursor = start;
    while cursor < end {
        ::core::intrinsics::volatile_store(cursor, 0);
        cursor = cursor.offset(1);
    }
}

/// Starting from this function,
/// kernel code is running in Exception Level 1
unsafe extern fn _start_el1() -> ! {
    // Enable all co-processors
    boot_time_log("[boot: enable all co-processors]");
    llvm_asm!("msr cpacr_el1, $0"::"r"(0xfffffff));
    boot_time_log("[boot: zero bss]");
    zero_bss();
    // Setup paging
    boot_time_log("[boot: setup kernel pagetable]");
    super::mm::paging::setup_kernel_pagetables();
    boot_time_log("[boot: switch to high address space]");
    // loop {}
    SP.set(SP.get() | 0xffff0000_00000000);
    let fn_addr = _start_el1_high_address_space as usize | 0xffff0000_00000000;
    let func: unsafe extern fn() -> ! = ::core::mem::transmute(fn_addr);
    func()
}

/// Starting from this function,
/// all kernel (virtual) addresses are located in the high address space.
/// Including SP, PC and other registers
/// i.e. `address & 0xffff0000_00000000 == 0xffff0000_00000000`
unsafe extern fn _start_el1_high_address_space() -> ! {
    BOOTED = true;
    // println!("[boot: clear temporary user page table]");
    super::mm::paging::clear_temp_user_pagetable();
    // Set EL1 interrupt vector
    // println!("[boot: setup interrupt vector]");
    VBAR_EL1.set((&exception::exception_handlers as *const _ as usize | 0xffff0000_00000000) as _);
    barrier::isb(barrier::SY);
    // Call kmain
    // set_booted();
    
    TTBR0_EL1.set(0);
    
    debug!(Kernel: "[boot: kernel_end = 0x{:x}]", crate::heap::constants::kernel_end());
    debug!(Kernel: "[boot: kernel_heap_end = 0x{:x}]", crate::heap::constants::kernel_heap_end());
    debug!(Kernel: "[boot: current execution level = {}]", (CurrentEL.get() & 0b1100) >> 2);
    <Kernel as AbstractKernel>::start();
}
