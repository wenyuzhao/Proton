use cortex_a::regs::*;
use proton_kernel::arch::*;
use proton_kernel::scheduler::AbstractScheduler;
#[cfg(feature="device-raspi4")]
use super::gic::*;
use crate::*;
use crate::peripherals::*;
use core::ptr::read_volatile;

const TIMER_INTERRUPT_FREQUENCY: usize = 1; // Hz

// pub const ARM_TIMER_BASE: usize = 0xffff0000_40000000;

#[cfg(feature="device-raspi3-qemu")]
pub const ARM_TIMER_BASE: usize = 0xffff0000_40000000;
#[cfg(feature="device-raspi4")]
pub const ARM_TIMER_BASE: usize = 0xFFFF0000_FF800000;

const ARM_CONTROL_REGISTER: *mut u32 = (ARM_TIMER_BASE + 0x0) as _;
const ARM_INTERRUPT_ROUTING: *mut u32 = (ARM_TIMER_BASE + 0x24) as _;
const ARM_LOCAL_TIMER_CONTROL_AND_STATUS: *mut u32 = (ARM_TIMER_BASE + 0x34) as _;
const ARM_LOCAL_TIMER_CLEARL_AND_RELOAD: *mut u32 = (ARM_TIMER_BASE + 0x38) as _;
const ARM_CORE_TIMER_INTERRUPT_CONTROL_BASE: usize = ARM_TIMER_BASE + 0x40;
const ARM_CORE_TIMER_IRQ_SOURCE_BASE: usize = ARM_TIMER_BASE + 0x60;

#[allow(non_snake_case)]
const fn ARM_CORE_TIMER_INTERRUPT_CONTROL(core: u8) -> *mut u32 {
    // 0x40, 0x44, 0x48, 0x4c: Core 0~3 Timers interrupt control
    (ARM_CORE_TIMER_INTERRUPT_CONTROL_BASE + 0x4 * (core as usize)) as _
}

#[allow(non_snake_case)]
const fn ARM_CORE_TIMER_IRQ_SOURCE(core: u8) -> *mut u32 {
    (ARM_CORE_TIMER_IRQ_SOURCE_BASE + 0x4 * (core as usize)) as _
}



static mut COUNT: u32 = 0;
// const TIMER_CS: *mut u32 = (PERIPHERAL_BASE + 0x3000) as _;
// const TIMER_CLO: *mut u32 = (PERIPHERAL_BASE + 0x3004) as _;
// const TIMER_C0: *mut u32 = (PERIPHERAL_BASE + 0x300C) as _;
// const TIMER_C1: *mut u32 = (PERIPHERAL_BASE + 0x3010) as _;
// const TIMER_C3: *mut u32 = (PERIPHERAL_BASE + 0x3018) as _;
pub const ARMTIMER_VALUE: *mut u32     = (PERIPHERAL_BASE + 0xB404) as _;

#[inline]
pub fn pending_timer_irq() -> bool {
    let x = unsafe { read_volatile(ARM_CORE_TIMER_IRQ_SOURCE(0) as *const u32) };
    (x & (1 << 1)) != 0
}

#[inline]
pub fn handle_timer_irq(_: usize, _: usize, _: usize, _: usize, _: usize, _: usize) -> isize {
    // println!("Timer iterrupt received");
    // Update compare value
    {
        let step = CNTFRQ_EL0.get() as u64 / TIMER_INTERRUPT_FREQUENCY as u64;
        unsafe {
            llvm_asm!("msr cntp_cval_el0, $0":: "r"(CNTPCT_EL0.get() + step));
        }
    }
    Kernel::global().scheduler.timer_tick();
    0
}



pub struct Timer;

impl AbstractTimer for Timer {
    #[cfg(feature="device-raspi4")]
    fn init() {
        debug!(Kernel: "Timer init raspi4");
        unsafe {
            llvm_asm!("dsb SY":::"memory");
            let timer_irq = 16 + 14;
            GICD::get().ISENABLER[timer_irq / 32] = 1 << (timer_irq % 32);
            let n_cntfrq: usize = CNTFRQ_EL0.get() as _;
            assert!(n_cntfrq % TIMER_INTERRUPT_FREQUENCY == 0);
            let clock_ticks_per_timer_irq = n_cntfrq / TIMER_INTERRUPT_FREQUENCY;
            let n_cntpct: usize = CNTPCT_EL0.get() as _;
            llvm_asm!("msr CNTP_CVAL_EL0, $0" :: "r" (n_cntpct + clock_ticks_per_timer_irq));
            CNTP_CTL_EL0.set(1);
            llvm_asm!("dmb SY":::"memory");
        }
        <AArch64 as AbstractArch>::Interrupt::set_handler(InterruptId::Timer, Some(box handle_timer_irq));
    }

    #[cfg(feature="device-raspi3-qemu")]
    fn init() {
        unsafe {
            let n_cntfrq: usize = CNTFRQ_EL0.get() as _;
            assert!(n_cntfrq % TIMER_INTERRUPT_FREQUENCY == 0);
            let clock_ticks_per_timer_irq = n_cntfrq / TIMER_INTERRUPT_FREQUENCY;
            let n_cntpct: usize = CNTPCT_EL0.get() as _;
            llvm_asm!("msr CNTP_CVAL_EL0, $0" :: "r" (n_cntpct + clock_ticks_per_timer_irq));
            CNTP_CTL_EL0.set(1);
            *ARM_CORE_TIMER_INTERRUPT_CONTROL(0) = 1 << 1;
        }
        <AArch64 as AbstractArch>::Interrupt::set_handler(InterruptId::Timer, Some(box handle_timer_irq));
    }

    fn wait(ms: usize) {
        let freq: usize = CNTFRQ_EL0.get() as _;
        let target_count: usize = CNTPCT_EL0.get() as usize + ((freq / 1000) * ms) / 1000;
        loop {
            let count: usize = CNTPCT_EL0.get() as usize;
            if count >= target_count {
                break
            }
        }
    }
}