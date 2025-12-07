//! Agent Management
//!
//! Defines the Agent Control Block (ACB) and associated structures.
//! Simplified for stroke-native kernel.


use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::kernel::memory::paging::UserAddressSpace;
use crate::kernel::memory::{Stack, alloc_stack};
use crate::kernel::capability::Capability;
use crate::fs::vfs::ProcessFileTable;
use crate::kernel::signal::SigAction;
use crate::arch::SpinLock;
use alloc::collections::vec_deque::VecDeque;

/// IPC Message (Fixed Size 64 bytes)
#[derive(Debug, Clone, Copy)]
pub struct Message {
    pub sender: AgentId,
    pub data: [u8; 64],
}

/// Unique Agent Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentId(pub u64);

impl AgentId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        AgentId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Agent State
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Ready,
    Running,
    Blocked,
    Sleeping,
    Terminated,
}

/// CPU Context (Callee-saved registers)
/// This matches the layout expected by `switch_to` in assembly.
/// CRITICAL: Field order must match assembly offsets exactly!
/// Assembly stores: x19-x28 at 0-72, x29 at 80, x30/LR at 88, SP at 96, TTBR0 at 104
#[repr(C)]
#[derive(Debug, Default)]
pub struct Context {
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub x28: u64,
    pub x29: u64,   // Frame Pointer (offset 80)
    pub lr: u64,    // Link Register x30 (offset 88) - stored by "stp x29, x30, [x0, #80]"
    pub sp: u64,    // Stack Pointer (offset 96) - stored by "str x9, [x0, #96]"
    pub ttbr0: u64, // Page Table Base (offset 104)
}

/// Agent Control Block
/// 
/// An Agent is a lightweight execution unit.
#[repr(C)]
pub struct Agent {
    pub context: Context,
    pub id: AgentId,
    pub state: AgentState,
    pub capabilities: Vec<Capability>,
    pub vmm: Option<UserAddressSpace>,
    pub kernel_stack: Stack,
    pub user_stack: Option<Stack>, // User stack might be managed by user? For now kernel manages it.
    pub file_table: ProcessFileTable,
    pub wake_time: u64,
    pub sig_actions: [SigAction; 32],
    pub vma_manager: crate::kernel::memory::vma::VmaManager,
    pub pending_signals: u32,
    pub blocked_signals: u32,
    pub parent_id: Option<u64>,
    pub cpu_cycles: u64,
    pub last_scheduled: u64,
    pub mailbox: SpinLock<VecDeque<Message>>,
}

impl Agent {
    /// Create a new kernel agent (simple)
    pub fn new_kernel_simple(entry: fn()) -> Result<Self, &'static str> {
        let kernel_stack = alloc_stack(4).ok_or("Failed to alloc kernel stack")?; // 16KB (4 pages)
        
        let mut agent = Agent {
            id: AgentId::new(),
            state: AgentState::Ready,
            context: Context::default(),
            capabilities: Vec::new(),
            vmm: None,
            kernel_stack,
            user_stack: None,
            file_table: ProcessFileTable::new(),
            wake_time: 0,
            sig_actions: [SigAction::default(); 32],
            vma_manager: crate::kernel::memory::vma::VmaManager::new(),
            pending_signals: 0,
            blocked_signals: 0,
            parent_id: None,
            cpu_cycles: 0,
            last_scheduled: 0,
            mailbox: SpinLock::new(VecDeque::new()),
        };

        let stack_top = agent.kernel_stack.top;
        
        // Align stack to 16 bytes (already aligned by page, but good practice)
        let stack_top = stack_top & !0xF;

        agent.context.sp = stack_top;
        agent.context.lr = entry as usize as u64;
        agent.context.ttbr0 = 0; // Kernel threads share TTBR1, TTBR0 is unused/zeroed

        Ok(agent)
    }

    /// Create a new user agent (simple)
    pub fn new_user_simple(entry: fn(), arg: u64) -> Result<Self, &'static str> {
        // 1. Create Address Space
        let mut space = UserAddressSpace::new().ok_or("Failed to create user address space")?;
        
        // 2. Allocate Stacks (Kernel & User)
        // Kernel stack: 16KB (4 pages)
        let kernel_stack = alloc_stack(4).ok_or("Failed to alloc kernel stack")?;
        // User stack: 16KB (4 pages)
        let user_stack = alloc_stack(4).ok_or("Failed to alloc user stack")?;
        
        // 3. Map User Stack into Address Space
        // We identity map it for now, but with User Permissions
        // Note: alloc_stack returns a kernel mapped address.
        // We need to map the physical pages to user space.
        // Since we are identity mapping kernel space, the virtual address is the physical address.
        // But wait, alloc_stack returns a virtual address in kernel space.
        // In our current identity-mapped kernel, Virt == Phys.
        // So we can use stack.bottom (start of usable stack) to stack.top.
        // We should map from bottom (inclusive) to top (exclusive).
        let _ustack_phys = user_stack.bottom - 4096; // Include guard page? No, user shouldn't access guard.
        // Actually, let's just map the usable stack pages.
        let ustack_phys_start = user_stack.bottom;
        let ustack_size = (user_stack.top - user_stack.bottom) as usize;
        
        // Map at high memory (same as new_user_elf)
        let ustack_virt_top = 0x0000_FFFF_FFFF_0000;
        let ustack_virt_bottom = ustack_virt_top - ustack_size as u64;
        
        space.map_user(ustack_virt_bottom, ustack_phys_start, ustack_size).map_err(|_| "Failed to map user stack")?;
        
        // 4. Map User Code (Copy to separate page to avoid Huge Page conflict)
        // Allocate a new page for user code (1 page = 4KB)
        let code_page = alloc_stack(1).ok_or("Failed to alloc code page")?;
        
        // Copy code from entry to code_page.bottom (usable start)
        let code_phys = code_page.bottom;
        
        unsafe {
            core::ptr::copy_nonoverlapping(entry as *const u8, code_phys as *mut u8, 4096);
        }
        
        let code_virt = 0x2_0000_0000; // 8GB mark (above kernel identity map)
        
        space.map_user(code_virt, code_phys, 4096).map_err(|_| "Failed to map user code")?;
        
        // Leak the code page so it persists
        core::mem::forget(code_page);

        let mut agent = Agent {
            id: AgentId::new(),
            state: AgentState::Ready,
            context: Context::default(),
            capabilities: Vec::new(),
            vmm: Some(space),
            kernel_stack,
            user_stack: Some(user_stack),
            file_table: ProcessFileTable::new(),
            wake_time: 0,
            sig_actions: [SigAction::default(); 32],
            vma_manager: crate::kernel::memory::vma::VmaManager::new(),
            pending_signals: 0,
            blocked_signals: 0,
            parent_id: None,
            cpu_cycles: 0,
            last_scheduled: 0,
            mailbox: SpinLock::new(VecDeque::new()),
        };

        // Kernel Stack Setup (for when we are in kernel mode handling this process)
        let kstack_top = agent.kernel_stack.top;
        let kstack_top = kstack_top & !0xF;
        agent.context.sp = kstack_top;

        // User Stack Setup (Virtual Address)
        let ustack_top = 0x0000_FFFF_FFFF_0000 & !0xF;

        // Set up trampoline
        // switch_to restores x19..x29. We use them to pass args to jump_to_userspace.
        agent.context.lr = user_trampoline as *const () as u64;
        agent.context.x19 = 0x2_0000_0000;      // Entry point (code_virt)
        agent.context.x20 = ustack_top;        // User Stack
        agent.context.x21 = arg;               // Argument

        // Set TTBR0 to the new User Table (with ASID)
        let vmm = agent.vmm.as_ref().expect("VMM must exist for user process");
        agent.context.ttbr0 = vmm.table_base() | ((vmm.asid() as u64) << 48);

        Ok(agent)
    }

    /// Create a new user agent from ELF binary
    pub fn new_user_elf(elf_data: &[u8]) -> Result<Self, &'static str> {
        // 1. Parse ELF
        let loader = crate::kernel::elf::ElfLoader::new(elf_data)?;
        
        // 2. Create Address Space
        let mut space = UserAddressSpace::new().ok_or("Failed to create user address space")?;
        
        // 3. Load Segments
        loader.load(&mut space)?;
        
        // 4. Allocate Stacks
        let kernel_stack = alloc_stack(4).ok_or("Failed to alloc kernel stack")?;
        let user_stack = alloc_stack(4).ok_or("Failed to alloc user stack")?;
        
        // 5. Map User Stack
        // We map it at a fixed high address for the user?
        // Or just identity map the allocated pages?
        // Let's map it at 0x0000_FFFF_FFFF_F000 (Top of user space - 4KB)
        // Stack grows down.
        // User Stack Size = 16KB (4 pages)
        // Top = 0x0000_FFFF_FFFF_F000
        // Bottom = Top - 16KB = 0x0000_FFFF_FFFB_F000
        
        // Wait, our VMM map_user takes virt, phys, size.
        // user_stack.ptr points to the physical pages (identity mapped in kernel).
        // The first page is the guard page.
        // user_stack.bottom is the start of usable memory.
        
        let ustack_phys_start = user_stack.bottom; // Physical address of bottom of stack
        let ustack_size = (user_stack.top - user_stack.bottom) as usize;
        
        // Let's pick a virtual address for the stack top.
        // Move to lower memory to avoid potential 48-bit boundary issues
        let ustack_virt_top = 0x2000_0000;
        let ustack_virt_bottom = ustack_virt_top - ustack_size as u64;
        
        space.map_user(ustack_virt_bottom, ustack_phys_start, ustack_size)?;
        
        // 6. Create Agent
        let mut agent = Agent {
            id: AgentId::new(),
            state: AgentState::Ready,
            context: Context::default(),
            capabilities: Vec::new(),
            vmm: Some(space),
            kernel_stack,
            user_stack: Some(user_stack),
            file_table: ProcessFileTable::new(),
            wake_time: 0,
            sig_actions: [SigAction::default(); 32],
            vma_manager: crate::kernel::memory::vma::VmaManager::new(),
            pending_signals: 0,
            blocked_signals: 0,
            parent_id: None,
            cpu_cycles: 0,
            last_scheduled: 0,
            mailbox: SpinLock::new(VecDeque::new()),
        };

        // Kernel Stack Setup
        let kstack_top = agent.kernel_stack.top & !0xF;
        agent.context.sp = kstack_top;

        // User Stack Setup (Virtual Address)
        let ustack_top_virt = ustack_virt_top & !0xF;

        // Trampoline Setup
        agent.context.lr = user_trampoline as *const () as u64;
        agent.context.x19 = loader.entry_point(); // Entry point
        agent.context.x20 = ustack_top_virt;      // User Stack (Virtual)
        agent.context.x21 = 0;                    // Arg (argc/argv ptr?)

        // Set TTBR0 (with ASID)
        let vmm = agent.vmm.as_ref().expect("VMM must exist for ELF process");
        agent.context.ttbr0 = vmm.table_base() | ((vmm.asid() as u64) << 48);

        // 7. Initialize VMAs
        use crate::kernel::memory::vma::{VMA, VmaPerms, VmaFlags};
        
        // Stack VMA (RW)
        let stack_vma = VMA::new(
            ustack_virt_bottom,
            ustack_size as u64,
            VmaPerms::RW,
            VmaFlags { private: true, anonymous: true, fixed: true }
        );
        let _ = agent.vma_manager.add_vma(stack_vma);
        
        // Code VMA (RX) - For now, just map the entry point page
        // In a real ELF loader, we'd iterate segments.
        // Here we just protect the entry page.
        let entry_page = loader.entry_point() & !0xFFF;
        let code_vma = VMA::new(
            entry_page,
            4096,
            VmaPerms::RX,
            VmaFlags { private: true, anonymous: false, fixed: true }
        );
        let _ = agent.vma_manager.add_vma(code_vma);

        Ok(agent)
    }


    pub fn fork(&self, frame: &crate::kernel::exception::ExceptionFrame, sp_el0: u64) -> Result<Self, &'static str> {
        // 1. Create new Address Space
        let mut space = UserAddressSpace::new().ok_or("Failed to create user address space")?;
        
        // 2. Deep Copy Memory (VMAs)
        // Iterate over all VMAs and copy data
        for vma in &self.vma_manager.vmas {
            // Check permissions (must be readable to copy)
            if !vma.perms.read { continue; }
            
            let mut virt = vma.start;
            while virt < vma.end {
                // Get physical address of current page
                if let Some(old_phys) = self.vmm.as_ref().expect("VMM required for fork").translate(virt) {
                    // Allocate new page
                    let new_page = unsafe { crate::kernel::memory::alloc_pages(1) }.ok_or("Out of memory for fork")?;
                    let new_phys = new_page.as_ptr() as u64;
                    
                    // Copy data
                    unsafe {
                        core::ptr::copy_nonoverlapping(old_phys as *const u8, new_phys as *mut u8, 4096);
                    }
                    
                    // Map new page in new VMM
                    // Use same flags as VMA (approximate)
                    // We need EntryFlags.
                    // VMA perms -> EntryFlags
                    use crate::kernel::memory::paging::EntryFlags;
                    let mut flags = EntryFlags::ATTR_NORMAL | EntryFlags::SH_INNER | EntryFlags::AF;
                    if vma.perms.write { flags |= EntryFlags::AP_RW_USER; } else { flags |= EntryFlags::AP_RO_USER; }
                    if !vma.perms.execute { flags |= EntryFlags::UXN; }
                    
                    space.map_user(virt, new_phys, 4096).map_err(|_| "Failed to map forked page")?;
                }
                virt += 4096;
            }
        }
        
        // 3. Allocate Kernel Stack
        let kernel_stack = alloc_stack(4).ok_or("Failed to alloc kernel stack")?;
        
        // 4. Copy Trap Frame to new Kernel Stack
        // Frame size = 280 bytes.
        // We place it at the top of the new stack.
        let kstack_top = kernel_stack.top;
        let frame_size = 280; // Must match assembly
        let frame_ptr = (kstack_top - frame_size) & !0xF; // Align
        
        unsafe {
            // Copy frame
            core::ptr::copy_nonoverlapping(
                frame as *const _ as *const u8, 
                frame_ptr as *mut u8, 
                frame_size as usize
            );
            
            // Set return value (x0) to 0 for child
            let frame_mut = &mut *(frame_ptr as *mut crate::kernel::exception::ExceptionFrame);
            frame_mut.x[0] = 0;
        }
        
        // 5. Create Agent
        let mut agent = Agent {
            id: AgentId::new(),
            state: AgentState::Ready,
            context: Context::default(),
            capabilities: self.capabilities.clone(),
            vmm: Some(space),
            kernel_stack,
            user_stack: None, // Managed by VMM/sp_el0
            file_table: ProcessFileTable::new(),
            wake_time: 0,
            sig_actions: self.sig_actions,
            vma_manager: self.vma_manager.clone(),
            pending_signals: 0,
            blocked_signals: self.blocked_signals,
            parent_id: Some(self.id.0),
            cpu_cycles: 0,
            last_scheduled: 0,
            mailbox: SpinLock::new(VecDeque::new()),
        };
        
        // Clone File Table (dup)
        agent.file_table = self.file_table.clone();

        // 6. Setup Context for Switch
        let kstack_ptr = frame_ptr; // SP points to the frame
        
        agent.context.sp = kstack_ptr;
        agent.context.lr = fork_return_trampoline as *const () as u64;
        agent.context.x19 = kstack_ptr; // Arg1: Frame Ptr
        agent.context.x20 = sp_el0;     // Arg2: SP_EL0
        
        let vmm = agent.vmm.as_ref().expect("VMM required for fork child");
        agent.context.ttbr0 = vmm.table_base() | ((vmm.asid() as u64) << 48);

        Ok(agent)
    }

    /// Execute a new program (replace current process)
    pub fn exec(&mut self, path: &str, frame: &mut crate::kernel::exception::ExceptionFrame) -> Result<(), &'static str> {
        // 1. Read file from VFS
        // We need to read the whole file into a buffer.
        // For this prototype, we'll use a fixed size buffer on the heap (Vec).
        // Limit: 1MB for now.
        
        let file = crate::fs::VFS.lock().open(path, crate::fs::O_RDONLY).map_err(|_| "File not found")?;
        let mut file_lock = file.lock();
        let size = file_lock.seek(crate::fs::SeekFrom::End(0)).map_err(|_| "Seek failed")? as usize;
        file_lock.seek(crate::fs::SeekFrom::Start(0)).map_err(|_| "Seek failed")?;
        
        if size > 1024 * 1024 {
            return Err("Executable too large");
        }
        
        let mut buffer = alloc::vec![0u8; size];
        let read = file_lock.read(&mut buffer).map_err(|_| "Read failed")?;
        if read != size {
            return Err("Incomplete read");
        }
        drop(file_lock);
        
        // 2. Parse ELF
        let loader = crate::kernel::elf::ElfLoader::new(&buffer)?;
        
        // 3. Create NEW Address Space
        let mut new_space = UserAddressSpace::new().ok_or("Failed to create user address space")?;
        
        // 4. Load Segments into NEW Space
        loader.load(&mut new_space)?;
        
        // 5. Allocate NEW User Stack
        let new_user_stack = alloc_stack(4).ok_or("Failed to alloc user stack")?;
        
        // 6. Map User Stack
        let ustack_phys_start = new_user_stack.bottom;
        let ustack_size = (new_user_stack.top - new_user_stack.bottom) as usize;
        let ustack_virt_top = 0x0000_FFFF_FFFF_0000;
        let ustack_virt_bottom = ustack_virt_top - ustack_size as u64;
        
        new_space.map_user(ustack_virt_bottom, ustack_phys_start, ustack_size).map_err(|_| "Failed to map user stack")?;
        
        // 7. Setup VMAs
        use crate::kernel::memory::vma::{VMA, VmaPerms, VmaFlags};
        let mut new_vma_manager = crate::kernel::memory::vma::VmaManager::new();
        
        // Stack VMA
        let stack_vma = VMA::new(
            ustack_virt_bottom,
            ustack_size as u64,
            VmaPerms::RW,
            VmaFlags { private: true, anonymous: true, fixed: true }
        );
        let _ = new_vma_manager.add_vma(stack_vma);
        
        // Code VMA (Entry point page)
        let entry_page = loader.entry_point() & !0xFFF;
        let code_vma = VMA::new(
            entry_page,
            4096,
            VmaPerms::RX,
            VmaFlags { private: true, anonymous: false, fixed: true }
        );
        let _ = new_vma_manager.add_vma(code_vma);
        
        // 8. Commit Changes (Point of no return)
        // Drop old resources
        self.vmm = Some(new_space);
        self.user_stack = Some(new_user_stack);
        self.vma_manager = new_vma_manager;
        
        // Reset signals?
        self.sig_actions = [SigAction::default(); 32];
        self.pending_signals = 0;
        
        // 9. Update Exception Frame
        // We are modifying the frame that will be restored upon return from syscall.
        frame.elr = loader.entry_point();
        // frame.sp_el0 = ustack_virt_top & !0xF; // Not in frame
        
        // Update SP_EL0 directly
        let new_sp = ustack_virt_top & !0xF;
        unsafe {
             core::arch::asm!("msr sp_el0, {}", in(reg) new_sp);
        }
        
        frame.spsr = 0; // EL0t
        frame.x[0] = 0; // argc?
        frame.x[1] = 0; // argv?
        
        // Update TTBR0 in Context (though context is saved on stack, 
        // switch_to uses the one in Agent struct? No, switch_to saves/restores from struct.
        // But we are currently RUNNING. The context struct is stale until we switch out.
        // However, we need to ensure that when we return to user mode, we use the NEW TTBR0.
        // The exception return (eret) doesn't change TTBR0.
        // We must manually switch TTBR0 before returning?
        // OR, we rely on the fact that we are in kernel, and when we return, we are still in the same process context.
        // But we just changed the VMM!
        // The hardware TTBR0_EL1 is still pointing to the OLD table!
        // We MUST update TTBR0_EL1 immediately.
        
        unsafe {
            let vmm = self.vmm.as_ref().expect("VMM must exist for exec");
            crate::arch::set_ttbr0_with_asid(vmm.table_base(), vmm.asid());
            // crate::arch::tlb_invalidate_all(); // Not needed if unique ASID? 
            // Better to invalidate for safety on EXEC (new mapping structure in same ASID? No, new ASID!)
            // Wait, exec created a NEW Address Space, which allocated a NEW ASID.
            // So we don't need to invalidate all if we switch to new ASID.
            // But just to be super safe, let's keep invalidate for now, removing it is obscure optimization.
            // Actually, if we use new ASID, we don't need to flush.
        }
        
        // Also update the context struct for future switches
        let vmm = self.vmm.as_ref().expect("VMM must exist after exec");
        self.context.ttbr0 = vmm.table_base() | ((vmm.asid() as u64) << 48);
        
        Ok(())
    }
}


/// Trampoline to jump to userspace
/// 
/// Called when `switch_to` returns for a user process.
/// Expects:
///   x19 = User Entry Point
///   x20 = User Stack Pointer
///   x21 = User Argument
extern "C" fn user_trampoline() {
    unsafe {
        let entry: u64;
        let stack: u64;
        let arg: u64;
        
        // Read from callee-saved registers
        core::arch::asm!("mov {}, x19", out(reg) entry);
        core::arch::asm!("mov {}, x20", out(reg) stack);
        core::arch::asm!("mov {}, x21", out(reg) arg);
        
        crate::kprintln!("[DEBUG] Trampoline: Entry={:#x} Stack={:#x} Arg={:#x}", entry, stack, arg);

        crate::arch::jump_to_userspace(entry, stack, arg);
    }
}

/// Trampoline to return from fork
/// 
/// Expects:
///   x19 = Frame Pointer
///   x20 = SP_EL0
extern "C" fn fork_return_trampoline() {
    unsafe {
        let frame_ptr: u64;
        let sp_el0: u64;
        core::arch::asm!("mov {}, x19", out(reg) frame_ptr);
        core::arch::asm!("mov {}, x20", out(reg) sp_el0);
        
        crate::arch::restore_exception_frame(frame_ptr as *const u8, sp_el0);
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}
