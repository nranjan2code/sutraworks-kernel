//! System Call Interface
//!
//! Handles requests from User Mode (EL0).

use crate::kernel::scheduler::{self, SCHEDULER};
use crate::fs::vfs;
use crate::kprintln;
use crate::kernel::signal::{Signal, SigAction};
use alloc::sync::Arc;
use crate::arch::SpinLock;
use crate::fs::pipe;

/// System Call Numbers
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u64)]
pub enum SyscallNumber {
    Exit = 0,
    Yield = 1,
    Print = 2,
    Sleep = 3,
    Open = 4,
    Close = 5,
    Read = 6,
    Write = 7,
    Kill = 8,
    SigAction = 9,
    Pipe = 10,
    Dup2 = 11,
    Mmap = 12,
    Munmap = 13,
    Socket = 14,
    Bind = 15,
    Connect = 16,
    GetPid = 17,
    Fork = 18,
    Wait = 19,
    Exec = 20,
    Unknown,
}

impl From<u64> for SyscallNumber {
    fn from(n: u64) -> Self {
        match n {
            0 => SyscallNumber::Exit,
            1 => SyscallNumber::Yield,
            2 => SyscallNumber::Print,
            3 => SyscallNumber::Sleep,
            4 => SyscallNumber::Open,
            5 => SyscallNumber::Close,
            6 => SyscallNumber::Read,
            7 => SyscallNumber::Write,
            8 => SyscallNumber::Kill,
            9 => SyscallNumber::SigAction,
            10 => SyscallNumber::Pipe,
            11 => SyscallNumber::Dup2,
            12 => SyscallNumber::Mmap,
            13 => SyscallNumber::Munmap,
            14 => SyscallNumber::Socket,
            15 => SyscallNumber::Bind,
            16 => SyscallNumber::Connect,
            17 => SyscallNumber::GetPid,
            18 => SyscallNumber::Fork,
            19 => SyscallNumber::Wait,
            20 => SyscallNumber::Exec,
            _ => SyscallNumber::Unknown,
        }
    }
}

/// Handle a system call
/// 
/// Arguments are passed in x0-x7.
/// Return value is placed in x0.
/// Handle a system call
/// 
/// Arguments are passed in x0-x7.
/// Return value is placed in x0.
pub fn dispatcher(num: u64, arg0: u64, arg1: u64, arg2: u64, frame: &mut crate::kernel::exception::ExceptionFrame) -> u64 {
    let syscall = SyscallNumber::from(num);
    
    match syscall {
        SyscallNumber::Exit => {
            sys_exit(arg0 as i32);
            0 // Should not return
        }
        SyscallNumber::Yield => {
            scheduler::yield_task();
            0
        }
        SyscallNumber::Print => {
            sys_print(arg0, arg1)
        }
        SyscallNumber::Sleep => {
            sys_sleep(arg0);
            0
        }
        SyscallNumber::Open => {
            sys_open(arg0, arg1)
        }
        SyscallNumber::Close => {
            sys_close(arg0)
        }
        SyscallNumber::Read => {
            sys_read(arg0, arg1, arg2)
        }
        SyscallNumber::Write => {
            sys_write(arg0, arg1, arg2)
        }
        SyscallNumber::Kill => {
            sys_kill(arg0, arg1 as i32)
        }
        SyscallNumber::SigAction => {
            sys_sigaction(arg0 as i32, arg1, arg2)
        }
        SyscallNumber::Pipe => {
            sys_pipe(arg0)
        }
        SyscallNumber::Dup2 => {
            sys_dup2(arg0, arg1)
        }
        SyscallNumber::Mmap => {
            // arg0: len, arg1: perms, arg2: flags
            sys_mmap(arg0, arg1, arg2)
        }
        SyscallNumber::Munmap => {
            // arg0: addr, arg1: len
            sys_munmap(arg0, arg1)
        }
        SyscallNumber::Socket => {
            // arg0: domain, arg1: type, arg2: protocol
            sys_socket(arg0, arg1, arg2)
        }
        SyscallNumber::Bind => {
            // arg0: fd, arg1: addr_ptr, arg2: addr_len
            sys_bind(arg0, arg1, arg2)
        }
        SyscallNumber::Connect => {
            // arg0: fd, arg1: addr_ptr, arg2: addr_len
            sys_connect(arg0, arg1, arg2)
        }
        SyscallNumber::GetPid => {
            sys_getpid()
        }
        SyscallNumber::Fork => {
            sys_fork(frame)
        }
        SyscallNumber::Wait => {
            sys_wait(arg0 as i32)
        }
        SyscallNumber::Exec => {
            sys_exec(arg0, frame)
        }
        SyscallNumber::Unknown => {
            kprintln!("Unknown syscall: {}", num);
            u64::MAX
        }
    }
}

fn sys_exit(code: i32) {
    kprintln!("Process exited with code {}", code);
    
    // Set state to Terminated and wake parent
    let mut scheduler = SCHEDULER.lock();
    scheduler.exit_current(code);
    drop(scheduler);
    
    // Yield CPU (Scheduler will drop this agent)
    loop {
        scheduler::yield_task();
    }
}

fn sys_sleep(ms: u64) {
    let mut scheduler = SCHEDULER.lock();
    scheduler.with_current_agent(|agent| {
        agent.state = crate::kernel::process::AgentState::Sleeping;
        agent.wake_time = crate::drivers::timer::uptime_ms() + ms;
    });
    drop(scheduler);
    scheduler::yield_task();
}

fn sys_print(ptr: u64, len: u64) -> u64 {
    let ptr_raw = ptr as *const u8;
    let len = len as usize;
    
    if len > 1024 {
        return u64::MAX; // Error: Too long
    }
    
    // Validate against VMM if possible
    let mut scheduler = SCHEDULER.lock();
    let valid = scheduler.with_current_agent(|agent| {
        if let Some(vmm) = &agent.vmm {
            // Check start and end pages
            let start = ptr;
            let end = ptr + len as u64;
            let mut curr = start & !0xFFF;
            while curr < end {
                if !vmm.is_mapped(curr) { return false; }
                curr += 4096;
            }
            true
        } else {
            // Kernel thread or no VMM, fallback to range check
            crate::kernel::memory::validate_read_ptr(ptr_raw, len).is_ok()
        }
    }).unwrap_or(false);
    drop(scheduler);

    if !valid { return u64::MAX; }

    match crate::kernel::memory::validate_user_str(ptr_raw, len) {
        Ok(s) => {
            crate::kprint!("{}", s);
            0
        }
        Err(_) => u64::MAX
    }
}

fn sys_open(path_ptr: u64, flags: u64) -> u64 {
    let ptr = path_ptr as *const u8;
    
    // Validate pointer
    let mut scheduler = SCHEDULER.lock();
    let valid = scheduler.with_current_agent(|agent| {
        if let Some(vmm) = &agent.vmm {
            vmm.is_mapped(path_ptr)
        } else {
            crate::kernel::memory::validate_read_ptr(ptr, 1).is_ok()
        }
    }).unwrap_or(false);
    
    if !valid { 
        return u64::MAX; 
    }
    // Drop lock to allow VFS open (which might take time, though VFS uses SpinLock so we should be careful about lock ordering)
    // SCHEDULER lock is held? No, we dropped it?
    // Wait, with_current_agent takes &mut self, so we hold the lock during closure.
    // We need to drop scheduler lock before VFS open.
    drop(scheduler);

    // Read up to 64 bytes for path
    let mut path_buf = [0u8; 64];
    let mut len = 0;
    for i in 0..64 {
        let c = unsafe { *ptr.add(i) };
        if c == 0 { break; }
        path_buf[i] = c;
        len += 1;
    }
    let path = core::str::from_utf8(&path_buf[0..len]).unwrap_or("");
    
    // Open file via VFS
    let file_res = crate::fs::VFS.lock().open(path, flags as usize);
    
    match file_res {
        Ok(file) => {
            // Allocate FD in current process
            let mut scheduler = SCHEDULER.lock();
            scheduler.with_current_agent(|agent| {
                match agent.file_table.alloc_fd(file, flags as usize) {
                    Ok(fd) => fd as u64,
                    Err(_) => u64::MAX // EMFILE
                }
            }).unwrap_or(u64::MAX) // No current agent
        },
        Err(_) => u64::MAX // ENOENT
    }
}

fn sys_close(fd: u64) -> u64 {
    let mut scheduler = SCHEDULER.lock();
    scheduler.with_current_agent(|agent| {
        match agent.file_table.close_fd(fd as usize) {
            Ok(_) => 0,
            Err(_) => u64::MAX
        }
    }).unwrap_or(u64::MAX)
}

fn sys_read(fd: u64, buf_ptr: u64, len: u64) -> u64 {
    let buf_raw = buf_ptr as *mut u8;
    let len = len as usize;
    
    let mut scheduler = SCHEDULER.lock();
    let valid = scheduler.with_current_agent(|agent| {
        if let Some(vmm) = &agent.vmm {
             let start = buf_ptr;
             let end = buf_ptr + len as u64;
             let mut curr = start & !0xFFF;
             while curr < end {
                 if !vmm.is_mapped(curr) { return false; }
                 curr += 4096;
             }
             true
        } else {
             crate::kernel::memory::validate_read_ptr(buf_raw, len).is_ok()
        }
    }).unwrap_or(false);
    
    if !valid {
        return u64::MAX;
    }
    
    // We need to keep scheduler locked or re-lock to access file table?
    // We dropped lock? No, we are holding it inside with_current_agent if we didn't drop it.
    // But we need to access file table.
    // Let's just do it all in one go if possible, but reading file might block/take time?
    // VFS read takes SpinLock on file.
    // If we hold Scheduler lock and take File lock, is it safe?
    // Scheduler -> File.
    // Does File ever take Scheduler? No.
    // So it should be safe.
    
    scheduler.with_current_agent(|agent| {
        if let Ok(desc) = agent.file_table.get_fd(fd as usize) {
            let mut file = desc.file.lock();
            // Create a slice from user pointer
            let buf = unsafe { core::slice::from_raw_parts_mut(buf_raw, len) };
            match file.read(buf) {
                Ok(n) => n as u64,
                Err(_) => u64::MAX
            }
        } else {
            u64::MAX
        }
    }).unwrap_or(u64::MAX)
}

fn sys_write(fd: u64, buf_ptr: u64, len: u64) -> u64 {
    let buf_raw = buf_ptr as *const u8;
    let len = len as usize;
    
    let mut scheduler = SCHEDULER.lock();
    let valid = scheduler.with_current_agent(|agent| {
        if let Some(vmm) = &agent.vmm {
             let start = buf_ptr;
             let end = buf_ptr + len as u64;
             let mut curr = start & !0xFFF;
             while curr < end {
                 if !vmm.is_mapped(curr) { return false; }
                 curr += 4096;
             }
             true
        } else {
             crate::kernel::memory::validate_read_ptr(buf_raw, len).is_ok()
        }
    }).unwrap_or(false);
    
    if !valid {
        return u64::MAX;
    }
    
    scheduler.with_current_agent(|agent| {
        if let Ok(desc) = agent.file_table.get_fd(fd as usize) {
            let mut file = desc.file.lock();
            let buf = unsafe { core::slice::from_raw_parts(buf_raw, len) };
            match file.write(buf) {
                Ok(n) => n as u64,
                Err(_) => u64::MAX
            }
        } else {
            u64::MAX
        }
    }).unwrap_or(u64::MAX)
}

fn sys_kill(pid: u64, sig: i32) -> u64 {
    let mut scheduler = SCHEDULER.lock();
    if let Some(agent) = scheduler.get_agent_mut(pid) {
        if let Some(signal) = Signal::from_i32(sig) {
            agent.pending_signals |= 1 << (signal as u32);
            // If agent is Sleeping, wake it up?
            if agent.state == crate::kernel::process::AgentState::Sleeping {
                agent.state = crate::kernel::process::AgentState::Ready;
                agent.wake_time = 0;
            }
            0
        } else {
            u64::MAX // Invalid signal
        }
    } else {
        u64::MAX // ESRCH
    }
}

fn sys_sigaction(sig: i32, act_ptr: u64, oldact_ptr: u64) -> u64 {
    let signal = match Signal::from_i32(sig) {
        Some(s) => s,
        None => return u64::MAX,
    };
    
    // Validate pointers
    let mut scheduler = SCHEDULER.lock();
    
    // Check if we can access the memory
    let valid = scheduler.with_current_agent(|agent| {
        let mut ok = true;
        if oldact_ptr != 0 {
            if let Some(vmm) = &agent.vmm {
                if !vmm.is_mapped(oldact_ptr) { ok = false; }
            } else {
                if crate::kernel::memory::validate_write_ptr(oldact_ptr as *mut u8, core::mem::size_of::<SigAction>()).is_err() { ok = false; }
            }
        }
        if act_ptr != 0 {
            if let Some(vmm) = &agent.vmm {
                if !vmm.is_mapped(act_ptr) { ok = false; }
            } else {
                if crate::kernel::memory::validate_read_ptr(act_ptr as *const u8, core::mem::size_of::<SigAction>()).is_err() { ok = false; }
            }
        }
        ok
    }).unwrap_or(false);

    if !valid {
        return u64::MAX;
    }

    scheduler.with_current_agent(|agent| {
        let sig_idx = signal as usize;
        
        if oldact_ptr != 0 {
            let old_act = agent.sig_actions[sig_idx];
            // Write old_act to user memory
            let ptr = oldact_ptr as *mut SigAction;
            unsafe { *ptr = old_act };
        }
        
        if act_ptr != 0 {
            let ptr = act_ptr as *const SigAction;
            let new_act = unsafe { *ptr };
            agent.sig_actions[sig_idx] = new_act;
        }
        0
    }).unwrap_or(u64::MAX)
}

fn sys_pipe(pipefd_ptr: u64) -> u64 {
    // 1. Create pipe
    let (reader, writer) = pipe::create_pipe();
    
    // 2. Allocate FDs
    let mut scheduler = SCHEDULER.lock();
    let res: Option<Result<(usize, usize), &'static str>> = scheduler.with_current_agent(|agent| {
        let r_fd = agent.file_table.alloc_fd(Arc::new(SpinLock::new(reader)), vfs::O_RDONLY)?;
        let w_fd = agent.file_table.alloc_fd(Arc::new(SpinLock::new(writer)), vfs::O_WRONLY)?;
        Ok((r_fd, w_fd))
    });
    drop(scheduler);
    
    match res {
        Some(Ok((r, w))) => {
            // Write FDs to user memory
            // Write FDs to user memory
            let ptr = pipefd_ptr as *mut i32;
            
            // Validate pointer
            // We need to check if we can write 8 bytes (2 x i32)
            if crate::kernel::memory::validate_write_ptr(ptr as *mut u8, 8).is_ok() {
                unsafe {
                   *ptr = r as i32;
                   *ptr.add(1) = w as i32;
                }
                crate::kprintln!("Pipe created: read={}, write={}", r, w);
                0
            } else {
                crate::kprintln!("Pipe created but failed to write to user memory");
                u64::MAX
            }
        },
        _ => u64::MAX
    }
}

fn sys_dup2(oldfd: u64, newfd: u64) -> u64 {
    let mut scheduler = SCHEDULER.lock();
    scheduler.with_current_agent(|agent| {
        match agent.file_table.dup2(oldfd as usize, newfd as usize) {
            Ok(fd) => fd as u64,
            Err(_) => u64::MAX
        }
    }).unwrap_or(u64::MAX)
}

fn sys_mmap(len: u64, perms: u64, flags: u64) -> u64 {
    use crate::kernel::memory::vma::{VmaPerms, VmaFlags};
    
    // Decode permissions
    let r = (perms & 1) != 0;
    let w = (perms & 2) != 0;
    let x = (perms & 4) != 0;
    let vma_perms = VmaPerms::new(r, w, x);
    
    // Decode flags (simplified)
    // 1 = Private, 2 = Anonymous, 4 = Fixed
    let private = (flags & 1) != 0;
    let anonymous = (flags & 2) != 0;
    let fixed = (flags & 4) != 0;
    let vma_flags = VmaFlags { private, anonymous, fixed };
    
    let mut scheduler = SCHEDULER.lock();
    let res = scheduler.with_current_agent(|agent| {
        // 1. Allocate VMA
        let addr = agent.vma_manager.mmap(len, vma_perms, vma_flags)?;
        
        // 2. Map pages if Anonymous
        if anonymous {
            if let Some(vmm) = &mut agent.vmm {
                // Allocate pages
                // Align len to page size
                let align = 4096;
                let size = (len + align - 1) & !(align - 1);
                let pages = size / 4096;
                
                if let Some(ptr) = unsafe { crate::kernel::memory::alloc_pages(pages as usize) } {
                    let phys = ptr.as_ptr() as u64;
                    // Map to user space
                    if vmm.map_user(addr, phys, size as usize).is_err() {
                        // Rollback VMA
                        agent.vma_manager.munmap(addr, len);
                        return None;
                    }
                    // Zero memory
                    unsafe { core::ptr::write_bytes(ptr.as_ptr(), 0, size as usize) };
                } else {
                    return None; // OOM
                }
            }
        }
        
        Some(addr)
    });
    
    res.flatten().unwrap_or(u64::MAX)
}

fn sys_munmap(addr: u64, len: u64) -> u64 {
    let mut scheduler = SCHEDULER.lock();
    let success = scheduler.with_current_agent(|agent| {
        // 1. Remove VMA
        if let Some(vma) = agent.vma_manager.munmap(addr, len) {
            // 2. Unmap pages
            if let Some(vmm) = &mut agent.vmm {
                let align = 4096;
                let size = (len + align - 1) & !(align - 1);
                let mut curr = addr;
                let end = addr + size;
                
                while curr < end {
                    // Unmap from page table
                    if let Ok(Some(phys)) = vmm.unmap_page(curr) {
                        // If anonymous, free the physical page
                        if vma.flags.anonymous {
                            unsafe {
                                // Convert physical address to NonNull pointer
                                // We assume direct mapping or we reconstruct the pointer
                                // alloc_pages returns NonNull<u8>
                                // We need to reconstruct it.
                                if let Some(ptr) = core::ptr::NonNull::new(phys as *mut u8) {
                                    crate::kernel::memory::free_pages(ptr, 1);
                                }
                            }
                        }
                    }
                    curr += 4096;
                }
            }
            true
        } else {
            false
        }
    }).unwrap_or(false);
    
    if success { 0 } else { u64::MAX }
}

fn sys_socket(domain: u64, type_: u64, _protocol: u64) -> u64 {
    // domain: 2 = AF_INET
    // type: 1 = SOCK_STREAM, 2 = SOCK_DGRAM
    // protocol: 0 = IPPROTO_IP
    
    if domain != 2 {
        return u64::MAX; // EAFNOSUPPORT
    }
    
    let socket_type = match type_ {
        1 => crate::net::socket::SocketType::Stream,
        2 => crate::net::socket::SocketType::Datagram,
        _ => return u64::MAX, // EINVAL
    };
    
    let socket = crate::net::socket::Socket::new(socket_type);
    let socket_file = Arc::new(SpinLock::new(socket));
    
    let mut scheduler = SCHEDULER.lock();
    let res = scheduler.with_current_agent(|agent| {
        agent.file_table.alloc_fd(socket_file, vfs::O_RDWR)
    });
    drop(scheduler);
    
    match res {
        Some(Ok(fd)) => fd as u64,
        _ => u64::MAX
    }
}

fn sys_bind(fd: u64, addr_ptr: u64, addr_len: u64) -> u64 {
    if addr_len < 16 {
        return u64::MAX; // EINVAL
    }
    
    // Validate pointer
    if crate::kernel::memory::validate_read_ptr(addr_ptr as *const u8, addr_len as usize).is_err() {
        return u64::MAX; // EFAULT
    }
    
    // Read sockaddr_in from user space
    let mut buf = [0u8; 16];
    let ptr = addr_ptr as *const u8;
    for i in 0..16 {
        buf[i] = unsafe { *ptr.add(i) };
    }
    
    // Parse sockaddr_in
    // family (2 bytes), port (2 bytes), addr (4 bytes), zero (8 bytes)
    let family = u16::from_le_bytes([buf[0], buf[1]]); // Usually LE on ARM/x86 host, but network is BE?
    // sockaddr family is usually host byte order.
    // port and addr are network byte order (BE).
    
    if family != 2 { // AF_INET
        return u64::MAX;
    }
    
    let port = u16::from_be_bytes([buf[2], buf[3]]);
    // let addr = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    
    let mut scheduler = SCHEDULER.lock();
    let res = scheduler.with_current_agent(|agent| {
        if let Ok(desc) = agent.file_table.get_fd(fd as usize) {
            let mut file = desc.file.lock();
            // Downcast to Socket
            if let Some(socket) = file.as_any().downcast_mut::<crate::net::socket::Socket>() {
                if socket.bind(port).is_ok() {
                    0
                } else {
                    u64::MAX
                }
            } else {
                u64::MAX // ENOTSOCK
            }
        } else {
            u64::MAX // EBADF
        }
    }).unwrap_or(u64::MAX);
    
    res
}

fn sys_connect(fd: u64, addr_ptr: u64, addr_len: u64) -> u64 {
    if addr_len < 16 {
        return u64::MAX;
    }
    
    // Validate pointer
    if crate::kernel::memory::validate_read_ptr(addr_ptr as *const u8, addr_len as usize).is_err() {
        return u64::MAX; // EFAULT
    }
    
    let mut buf = [0u8; 16];
    let ptr = addr_ptr as *const u8;
    for i in 0..16 {
        buf[i] = unsafe { *ptr.add(i) };
    }
    
    let family = u16::from_le_bytes([buf[0], buf[1]]);
    if family != 2 {
        return u64::MAX;
    }
    
    let port = u16::from_be_bytes([buf[2], buf[3]]);
    let addr_bytes = [buf[4], buf[5], buf[6], buf[7]];
    let addr = crate::net::ip::Ipv4Addr(addr_bytes);
    
    let mut scheduler = SCHEDULER.lock();
    let res = scheduler.with_current_agent(|agent| {
        if let Ok(desc) = agent.file_table.get_fd(fd as usize) {
            let mut file = desc.file.lock();
            if let Some(socket) = file.as_any().downcast_mut::<crate::net::socket::Socket>() {
                if socket.connect(addr, port).is_ok() {
                    0
                } else {
                    u64::MAX
                }
            } else {
                u64::MAX
            }
        } else {
            u64::MAX
        }
    }).unwrap_or(u64::MAX);
    res
}

fn sys_getpid() -> u64 {
    let scheduler = SCHEDULER.lock();
    scheduler.current_pid().unwrap_or(u64::MAX)
}

fn sys_fork(frame: &crate::kernel::exception::ExceptionFrame) -> u64 {
    // Read sp_el0
    let sp_el0: u64;
    unsafe { core::arch::asm!("mrs {}, sp_el0", out(reg) sp_el0); }
    
    let mut scheduler = SCHEDULER.lock();
    if let Some(parent_id) = scheduler.current_pid() {
        match scheduler.fork_agent(parent_id, frame, sp_el0) {
            Ok(child_pid) => child_pid,
            Err(e) => {
                kprintln!("Fork failed: {}", e);
                u64::MAX
            }
        }
    } else {
        u64::MAX
    }
}

fn sys_wait(pid: i32) -> u64 {
    // Only support wait(-1) for now (any child)
    if pid != -1 {
        // TODO: Support specific PID
        return u64::MAX;
    }

    loop {
        let mut scheduler = SCHEDULER.lock();
        let current_pid = scheduler.current_pid().unwrap_or(0);
        
        match scheduler.wait_child(current_pid) {
            Ok(Some(child_pid)) => {
                return child_pid;
            },
            Ok(None) => {
                // Children exist but running
                scheduler.with_current_agent(|agent| {
                    agent.state = crate::kernel::process::AgentState::Blocked;
                });
                drop(scheduler);
                scheduler::yield_task();
            },
            Err(_) => {
                return u64::MAX; // ECHILD
            }
        }
    }
}

fn sys_exec(path_ptr: u64, frame: &mut crate::kernel::exception::ExceptionFrame) -> u64 {
    // Validate pointer
    let ptr = path_ptr as *const u8;
    if crate::kernel::memory::validate_read_ptr(ptr, 1).is_err() {
        return u64::MAX;
    }
    
    // Read path string
    let mut path_buf = [0u8; 64];
    let mut len = 0;
    for i in 0..64 {
        let c = unsafe { *ptr.add(i) };
        if c == 0 { break; }
        path_buf[i] = c;
        len += 1;
    }
    let path = core::str::from_utf8(&path_buf[0..len]).unwrap_or("");
    
    let mut scheduler = SCHEDULER.lock();
    let res = scheduler.with_current_agent(|agent| {
        match agent.exec(path, frame) {
            Ok(_) => 0,
            Err(e) => {
                kprintln!("Exec failed: {}", e);
                u64::MAX
            }
        }
    }).unwrap_or(u64::MAX);
    
    res
}
