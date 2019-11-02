#![feature(asm)]
#![feature(format_args_nl)]
#![feature(global_asm)]
#![feature(panic_info_message)]
#![feature(core_intrinsics)]
#![feature(stmt_expr_attributes)]
#![no_std]
#![no_main]

extern crate lazy_static;
extern crate spin;
#[allow(unused)]
mod gpio;
#[macro_use]
mod debug;
mod mailbox;
mod fb;
mod random;


global_asm!(include_str!("./boot.S"));


pub fn wait_forever() -> ! {
    loop {
        unsafe { asm!("wfe" :::: "volatile") }
    }
}

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    debug!("Hello Raspberry PI!");
    {
        let mut fb = fb::FRAME_BUFFER.lock();
        fb.init();
        fb.clear(fb::Color::rgba(0x37474FFF));
    }
    debug!("Random: {} {} {}", random::random(0, 100), random::random(0, 100), random::random(0, 100));
    wait_forever();
}



#[cfg(not(feature="rls"))]
#[panic_handler]
fn panic(info: &::core::panic::PanicInfo) -> ! {
    debug!("{}", info);
    loop {}
}