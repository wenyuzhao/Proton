#[cfg(feature="device-raspi4")]
use super::gic::*;
use proton_kernel::task::Task;
use proton_kernel::arch::*;
use crate::*;
#[cfg(feature="device-raspi4")]
use core::intrinsics::{volatile_load, volatile_store};




#[repr(usize)]
#[derive(Debug)]
pub enum ExceptionLevel {
    EL0 = 0,
    EL1 = 1,
    EL2 = 2,
}

#[repr(usize)]
#[derive(Debug)]
pub enum ExceptionKind {
    Synchronous = 0,
    IRQ = 1,
    FIQ = 2,
    SError = 3,
}

#[repr(u32)]
#[derive(Debug)]
pub enum ExceptionClass {
    SVCAArch64 = 0b010101,
    DataAbortLowerEL = 0b100100,
    DataAbortHigherEL = 0b100101,
}

#[repr(C)]
#[derive(Debug)]
pub struct ExceptionFrame {
    pub q: [u128; 32],
    pub elr_el1: usize,
    pub spsr_el1: usize,
    pub x30: usize,
    pub x31: usize,
    pub x8_to_x29: [u64; 29 - 8 + 1],
    pub x6: usize,
    pub x7: usize,
    pub x4: usize,
    pub x5: usize,
    pub x2: usize,
    pub x3: usize,
    pub x0: usize,
    pub x1: usize,
}

unsafe fn get_exception_class() -> ExceptionClass {
    let esr_el1: u32;
    llvm_asm!("mrs $0, esr_el1":"=r"(esr_el1));
    ::core::mem::transmute(esr_el1 >> 26)
}

#[no_mangle]
pub unsafe extern fn handle_exception(exception_frame: *mut ExceptionFrame) {
    // println!("EF = {:?}", exception_frame as *mut _);
    debug_assert!(Task::<Kernel>::current().unwrap().context.exception_frame as usize == 0);
    Task::<Kernel>::current().map(|t| t.context.exception_frame = exception_frame);
    let exception = get_exception_class();
    debug!(Kernel: "Exception received");
    match exception {
        ExceptionClass::SVCAArch64 => {
            debug!(Kernel: "SVCAArch64 Start {:?}", Task::<Kernel>::current().unwrap().id());
            let _r = super::interrupt::handle_interrupt(InterruptId::Soft, &mut *exception_frame);
            ::core::sync::atomic::fence(::core::sync::atomic::Ordering::SeqCst);
            // unsafe { (*exception_frame).x0 = ::core::mem::transmute(r) };
            debug!(Kernel: "SVCAArch64 End {:?}", Task::<Kernel>::current().unwrap().id());
        },
        ExceptionClass::DataAbortLowerEL | ExceptionClass::DataAbortHigherEL => {
            let far: usize;
            llvm_asm!("mrs $0, far_el1":"=r"(far));
            let elr: usize;
            llvm_asm!("mrs $0, elr_el1":"=r"(elr));
            debug!(Kernel: "Data Abort {:?} {:?}", far as *mut (), elr as *mut ());
            // debug!(Kernel: "Data Abort {:?}", far as *mut ());
            super::mm::handle_user_pagefault(far.into());
        },
        #[allow(unreachable_patterns)]
        v => {
            debug!(Kernel: "Exception Frame: {:?} {:?}", exception_frame, *exception_frame);
            let esr_el1: u32;
            llvm_asm!("mrs $0, esr_el1":"=r"(esr_el1));
            debug!(Kernel: "ESR_EL1 = <EC={:x}, IL={:x}>", esr_el1 >> 26, esr_el1 & ((1 << 26) - 1));
            // debug!(Kernel: "0x212ba4 -> 0x{:x}", *(0x212ba4usize as *const usize));
            panic!("Unknown exception 0b{:b}", ::core::mem::transmute::<_, u32>(v))
        },
    }
    
    ::core::sync::atomic::fence(::core::sync::atomic::Ordering::SeqCst);
    
    debug!(Kernel: "return_to_use00");
    Task::<Kernel>::current().unwrap().context.return_to_user();
}

#[no_mangle]
pub unsafe extern fn handle_exception_serror(exception_frame: *mut ExceptionFrame) {
    // println!("EF = {:?}", exception_frame as *mut _);
    debug_assert!(Task::<Kernel>::current().unwrap().context.exception_frame as usize == 0);
    Task::<Kernel>::current().map(|t| t.context.exception_frame = exception_frame);
    let exception = get_exception_class();
    debug!(Kernel: "SError received {:?}", exception);
    match exception {
        ExceptionClass::SVCAArch64 => {
            debug!(Kernel: "SVCAArch64 Start {:?}", Task::<Kernel>::current().unwrap().id());
            let _r = super::interrupt::handle_interrupt(InterruptId::Soft, &mut *exception_frame);
            ::core::sync::atomic::fence(::core::sync::atomic::Ordering::SeqCst);
            // unsafe { (*exception_frame).x0 = ::core::mem::transmute(r) };
            debug!(Kernel: "SVCAArch64 End {:?}", Task::<Kernel>::current().unwrap().id());
        },
        ExceptionClass::DataAbortLowerEL | ExceptionClass::DataAbortHigherEL => {
            let far: usize;
            llvm_asm!("mrs $0, far_el1":"=r"(far));
            let elr: usize;
            llvm_asm!("mrs $0, elr_el1":"=r"(elr));
            debug!(Kernel: "Data Abort {:?} {:?}", far as *mut (), elr as *mut ());
            // debug!(Kernel: "Data Abort {:?}", far as *mut ());
            super::mm::handle_user_pagefault(far.into());
        },
        #[allow(unreachable_patterns)]
        v => {
            debug!(Kernel: "Exception Frame: {:?} {:?}", exception_frame, *exception_frame);
            let esr_el1: u32;
            llvm_asm!("mrs $0, esr_el1":"=r"(esr_el1));
            debug!(Kernel: "ESR_EL1 = <EC={:x}, IL={:x}>", esr_el1 >> 26, esr_el1 & ((1 << 26) - 1));
            // debug!(Kernel: "0x212ba4 -> 0x{:x}", *(0x212ba4usize as *const usize));
            panic!("Unknown exception 0b{:b}", ::core::mem::transmute::<_, u32>(v))
        },
    }
    
    ::core::sync::atomic::fence(::core::sync::atomic::Ordering::SeqCst);
    
    debug!(Kernel: "return_to_use00");
    Task::<Kernel>::current().unwrap().context.return_to_user();
}

#[cfg(feature="device-raspi4")]
#[no_mangle]
pub extern fn handle_interrupt(exception_frame: &mut ExceptionFrame) {
    Task::<Kernel>::current().unwrap().context.exception_frame = exception_frame;
    ::core::sync::atomic::fence(::core::sync::atomic::Ordering::SeqCst);
    #[allow(non_snake_case)]
    let GICC = GICC::get();
    let iar = unsafe { volatile_load(&GICC.IAR) };
    let irq = iar & GICC::IAR_INTERRUPT_ID__MASK;
    unsafe { volatile_store(&mut GICC.EOIR, iar) }; // FIXME: End of Interrupt ??? here ???
    if irq < 256 {
        if irq == 30 {
            unsafe { volatile_store(&mut GICC.EOIR, iar) };
            super::interrupt::handle_interrupt(InterruptId::Timer, &mut *exception_frame);
            return;
        } else {
            panic!("Unknown IRQ");
        }
    }
    
    ::core::sync::atomic::fence(::core::sync::atomic::Ordering::SeqCst);
    unsafe {
        // debug!(Kernel: "return_to_use00");
        Task::<Kernel>::current().unwrap().context.return_to_user();
    }
}

#[cfg(feature="device-raspi3-qemu")]
#[no_mangle]
pub extern fn handle_interrupt(exception_frame: &mut ExceptionFrame) {
    // println!("EF = {:?}", exception_frame as *mut _);
    // debug!(Kernel: "Interrupt received {:?}", exception_frame);
    
    debug_assert!(Task::<Kernel>::current().unwrap().context.exception_frame as usize == 0);
    Task::<Kernel>::current().unwrap().context.exception_frame = exception_frame;

    if super::timer::pending_timer_irq() {
        super::interrupt::handle_interrupt(InterruptId::Timer, exception_frame);
    } else {
        // println!(AArch64: "Unknown IRQ");
        loop {}
    }
    unsafe {
        Task::<Kernel>::current().unwrap().context.return_to_user();
    }
}

extern {
    pub static exception_handlers: u8;
    pub fn exit_exception() -> !;
}


// FIXME: We may need to switch stack after enter an exception,
//        to avoid stack overflow.
// Exception handlers table
global_asm! {"
.global exception_handlers
.global exit_exception

.macro push_all
    stp x0,  x1,  [sp, #-16]!
    stp x2,  x3,  [sp, #-16]!
    stp x4,  x5,  [sp, #-16]!
    stp x6,  x7,  [sp, #-16]!
    stp x8,  x9,  [sp, #-16]!
    stp x10, x11, [sp, #-16]!
    stp x12, x13, [sp, #-16]!
    stp x14, x15, [sp, #-16]!
    stp x16, x17, [sp, #-16]!
    stp x18, x19, [sp, #-16]!
    stp x20, x21, [sp, #-16]!
    stp x22, x23, [sp, #-16]!
    stp x24, x25, [sp, #-16]!
    stp x26, x27, [sp, #-16]!
    stp x28, x29, [sp, #-16]!
    mrs	x21, sp_el0
    mrs x22, elr_el1
    mrs x23, spsr_el1
    stp x30, x21, [sp, #-16]!
    stp x22, x23, [sp, #-16]!
    stp q0,  q1,  [sp, #-32]!
    stp q2,  q3,  [sp, #-32]!
    stp q4,  q5,  [sp, #-32]!
    stp q6,  q7,  [sp, #-32]!
    stp q8,  q9,  [sp, #-32]!
    stp q10, q11, [sp, #-32]!
    stp q12, q13, [sp, #-32]!
    stp q14, q15, [sp, #-32]!
    stp q16, q17, [sp, #-32]!
    stp q18, q19, [sp, #-32]!
    stp q20, q21, [sp, #-32]!
    stp q22, q23, [sp, #-32]!
    stp q24, q25, [sp, #-32]!
    stp q26, q27, [sp, #-32]!
    stp q28, q29, [sp, #-32]!
    stp q30, q31, [sp, #-32]!
.endm

.macro pop_all
    ldp q30, q31, [sp], #32
    ldp q28, q29, [sp], #32
    ldp q26, q27, [sp], #32
    ldp q24, q25, [sp], #32
    ldp q22, q23, [sp], #32
    ldp q20, q21, [sp], #32
    ldp q18, q19, [sp], #32
    ldp q16, q17, [sp], #32
    ldp q14, q15, [sp], #32
    ldp q12, q13, [sp], #32
    ldp q10, q11, [sp], #32
    ldp q8,  q9,  [sp], #32
    ldp q6,  q7,  [sp], #32
    ldp q4,  q5,  [sp], #32
    ldp q2,  q3,  [sp], #32
    ldp q0,  q1,  [sp], #32
    ldp x22, x23, [sp], #16
    ldp x30, x21, [sp], #16
    msr	sp_el0, x21
    msr elr_el1, x22  
    msr spsr_el1, x23
    ldp x28, x29, [sp], #16
    ldp x26, x27, [sp], #16
    ldp x24, x25, [sp], #16
    ldp x22, x23, [sp], #16
    ldp x20, x21, [sp], #16
    ldp x18, x19, [sp], #16
    ldp x16, x17, [sp], #16
    ldp x14, x15, [sp], #16
    ldp x12, x13, [sp], #16
    ldp x10, x11, [sp], #16
    ldp x8,  x9,  [sp], #16
    ldp x6,  x7,  [sp], #16
    ldp x4,  x5,  [sp], #16
    ldp x2,  x3,  [sp], #16
    ldp x0,  x1,  [sp], #16
.endm

.macro except_hang, exception_id
    .align 7
0:  wfi
    b 0b
.endm

exit_exception:
    pop_all
    eret

except:
    push_all
    mov x0, sp
    bl handle_exception
    except_hang 0

serror:
    push_all
    mov x0, sp
    bl handle_exception_serror
    except_hang 0

irq:
    push_all
    mov x0, sp
    bl handle_interrupt
    except_hang 0

    .balign 4096
exception_handlers:
    // Same exeception level, EL0
    .align 9; b except
    .align 7; b irq
    .align 7; b serror
    .align 7; b serror
    // Same exeception level, ELx
    .align 9; b except
    .align 7; b irq
    .align 7; b serror
    .align 7; b serror
    // Transit to upper exeception level, AArch64
    .align 9; b except
    .align 7; b irq
    .align 7; b serror
    .align 7; b serror
    // Transit to upper exeception level, AArch32: Unreachable
    .align 9; b except
    .align 7; b irq
    .align 7; b serror
    .align 7; b serror
"}