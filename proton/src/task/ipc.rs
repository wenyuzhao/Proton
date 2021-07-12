use super::{Message, Task, TaskId};
use crate::arch::*;
pub use crate::user::ipc::IPC;

pub fn init() {
    TargetArch::interrupt().set_handler(
        InterruptId::Soft,
        Some(box |ipc, a, b, c, d, e| {
            let ipc: IPC = unsafe { core::mem::transmute(ipc) };
            match ipc {
                IPC::Log => log(a, b, c, d, e),
                IPC::Send => send(a, b, c, d, e),
                IPC::Receive => receive(a, b, c, d, e),
            }
        }),
    );
}

// =====================
// ===   IPC Calls   ===
// =====================

fn log(a: usize, _: usize, _: usize, _: usize, _: usize) -> isize {
    let string_pointer = a as *const &str;
    let s: &str = unsafe { &*string_pointer };
    crate::log::_print(format_args!("{}", s));
    0
}

fn send(x1: usize, _: usize, _: usize, _: usize, _: usize) -> isize {
    let mut msg = unsafe { (*(x1 as *const Message)).clone() };
    let current_task = Task::current().unwrap();
    msg.sender = current_task.id();
    Task::send_message(msg)
}

fn receive(x1: usize, _: usize, _: usize, _: usize, _: usize) -> isize {
    let from_id = unsafe {
        let id = core::mem::transmute::<_, isize>(x1);
        if id < 0 {
            None
        } else {
            Some(core::mem::transmute::<_, TaskId>(id))
        }
    };
    log!(
        "{:?} start receiving from {:?}",
        Task::current().unwrap().id(),
        from_id
    );
    Task::receive_message(from_id)
}