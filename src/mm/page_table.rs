use cortex_a::regs::*;
use core::marker::PhantomData;
use super::frame_allocator;
use super::address::*;
use super::page::*;
use super::heap_constants::*;


bitflags! {
    pub struct PageFlags: usize {
        const PRESENT     = 0b01;      // map a 4k page
        const SMALL_PAGE  = 0b10;      // map a 4k page
        const USER        = 1 << 6;    // enable EL0 Access
        const NO_WRITE    = 1 << 7;    // readonly
        const ACCESSED    = 1 << 10;   // accessed
        const NO_EXEC     = 1 << 54;   // no execute
        const INNER_SHARE = 0b10 << 8; // outter shareable
        const OUTER_SHARE = 0b11 << 8; // inner shareable
        const COPY_ON_WRITE = 1 << 53;
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct PageTableEntry(usize);

impl PageTableEntry {
    const ADDRESS_MASK: usize = 0x0000_ffff_ffff_f000;
    const FLAGS_MASK: usize = !Self::ADDRESS_MASK;
    
    pub fn clear(&mut self) {
        self.0 = 0;
    }
    pub fn present(&self) -> bool {
        self.flags().contains(PageFlags::PRESENT)
    }
    pub fn is_block(&self) -> bool {
        !self.flags().contains(PageFlags::SMALL_PAGE)
    }
    pub fn address(&self) -> Address<P> {
        (self.0 & Self::ADDRESS_MASK).into()
    }
    pub fn flags(&self) -> PageFlags {
        let v = self.0 & Self::FLAGS_MASK;
        PageFlags::from_bits_truncate(v)
    }
    pub fn update_flags(&mut self, new_flags: PageFlags) {
        self.0 = self.address().as_usize() | new_flags.bits();
    }
    pub fn set<S: PageSize>(&mut self, frame: Frame<S>, flags: PageFlags) {
        if S::LOG_SIZE == Size2M::LOG_SIZE {
            debug_assert!(flags.bits() & 0b10 == 0);
        } else {
            debug_assert!(flags.bits() & 0b10 == 0b10);
        }
        let mut a = frame.start().as_usize();
        a &= !(0xffff_0000_0000_0000);
        self.0 = a | flags.bits();
    }
}

pub trait TableLevel {
    const ID: usize;
    const SHIFT: usize;
    type NextLevel: TableLevel;
}

pub struct L4;

impl TableLevel for L4 {
    const ID: usize = 4;
    const SHIFT: usize = 12 + 9 * 3;
    type NextLevel = L3;
}

pub struct L3;

impl TableLevel for L3 {
    const ID: usize = 3;
    const SHIFT: usize = 12 + 9 * 2;
    type NextLevel = L2;
}

pub struct L2;

impl TableLevel for L2 {
    const ID: usize = 2;
    const SHIFT: usize = 12 + 9 * 1;
    type NextLevel = L1;
}

pub struct L1;

impl TableLevel for L1 {
    const ID: usize = 1;
    const SHIFT: usize = 12 + 9 * 0;
    type NextLevel = !;
}

impl TableLevel for ! {
    const ID: usize = 0;
    const SHIFT: usize = 0;
    type NextLevel = !;
}

#[repr(C, align(4096))]
pub struct PageTable<L: TableLevel + 'static> {
    pub entries: [PageTableEntry; 512],
    phantom: PhantomData<L>,
}

pub const PAGE_TABLE_FLAGS: PageFlags = PageFlags::from_bits_truncate(0b01 | 0b11 | (0b11 << 8) | (0b1 << 10));

impl <L: TableLevel> PageTable<L> {
    const MASK: usize = 0b111111111 << L::SHIFT;
    fn zero(&mut self) {
        for i in 0..512 {
            self.entries[i] = PageTableEntry(0);
        }
    }

    #[inline]
    fn get_index(a: Address<V>) -> usize {
        (a.as_usize() >> L::SHIFT) & 0b111111111
    }

    fn next_table_address(&self, index: usize) -> Option<usize> {
        debug_assert!(L::ID > 1);
        if self.entries[index].present() && !self.entries[index].is_block() {
            let table_address = self as *const _ as usize;
            let mut a = (table_address << 9) | (index << 12);
            if self as *const _ as usize & (0xffff << 48) == 0 {
                a &= 0x0000_ffff_ffff_ffff;
            }
            Some(a)
        } else {
            None
        }
    }

    fn next_table(&self, index: usize) ->  Option<&'static mut PageTable<L::NextLevel>> {
        debug_assert!(L::ID > 1);
        if let Some(address) = self.next_table_address(index) {
            Some(unsafe { &mut *(address as *mut _) })
        } else {
            None
        }
    }

    fn next_table_create(&mut self, index: usize) -> &'static mut PageTable<L::NextLevel> {
        debug_assert!(L::ID > 1);
        if let Some(address) = self.next_table_address(index) {
            return unsafe { &mut *(address as *mut _) }
        } else {
            let frame = frame_allocator::alloc::<Size4K>().expect("no framxes available");
            self.entries[index].set(frame, PageFlags::PRESENT | PageFlags::SMALL_PAGE | PageFlags::OUTER_SHARE | PageFlags::ACCESSED);
            let t = self.next_table_create(index);
            t.zero();
            t
        }
    }

    #[allow(mutable_transmutes)]
    fn get_entry(&self, address: Address<V>) -> Option<(usize, &'static mut PageTableEntry)> {
        debug_assert!(L::ID != 0);
        let index = Self::get_index(address);
        if L::ID == 2 && self.entries[index].is_block() {
            return Some((L::ID, unsafe { ::core::mem::transmute(&self.entries[index]) }));
        }
        if L::ID == 1 {
            return Some((L::ID, unsafe { ::core::mem::transmute(&self.entries[index]) }));
        }
        let next = self.next_table(index)?;
        next.get_entry(address)
    }

    fn get_entry_create<S: PageSize>(&mut self, address: Address<V>) -> (usize, &'static mut PageTableEntry) {
        debug_assert!(L::ID != 0);
        let index = Self::get_index(address);
        if L::ID == 2 && self.entries[index].present() && self.entries[index].is_block() {
            debug_assert!(S::LOG_SIZE != Size4K::LOG_SIZE);
            return (L::ID, unsafe { ::core::mem::transmute(&mut self.entries[index]) });
        }
        if S::LOG_SIZE == Size4K::LOG_SIZE && L::ID == 1 {
            return (L::ID, unsafe { ::core::mem::transmute(&mut self.entries[index]) });
        }
        if S::LOG_SIZE == Size2M::LOG_SIZE && L::ID == 2 {
            return (L::ID, unsafe { ::core::mem::transmute(&mut self.entries[index]) });
        }
        let next = self.next_table_create(index);
        next.get_entry_create::<S>(address)
    }
}

impl PageTable<L4> {
    pub const fn new() -> Self {
        Self {
            entries: unsafe { ::core::mem::transmute([0u64; 512]) },
            phantom: PhantomData,
        }
    }

    #[inline]
    pub fn get(high: bool) -> &'static mut Self {
        if high {
            unsafe { Address::<V>::new(0xffff_ffff_ffff_f000).as_ref_mut() }
        } else {
            unsafe { Address::<V>::new(0x0000_ffff_ffff_f000).as_ref_mut() }
        }
    }

    pub fn translate(&mut self, a: Address<V>) -> Option<(Address<P>, PageFlags)> {
        let (level, entry) = self.get_entry(a)?;
        if entry.present() {
            let page_offset = a.as_usize() & 0xfff;
            Some((entry.address() + page_offset, entry.flags()))
        } else {
            None
        }
    }

    pub fn identity_map<S: PageSize>(&mut self, frame: Frame<S>, flags: PageFlags) -> Page<S> {
        self.map(Page::new(frame.start().as_usize().into()), frame, flags)
    }

    pub fn map<S: PageSize>(&mut self, page: Page<S>, frame: Frame<S>, flags: PageFlags) -> Page<S> {
        let (level, entry) = self.get_entry_create::<S>(page.start());
        if cfg!(debug_assertions) {
            if S::LOG_SIZE == Size4K::LOG_SIZE {
                assert!(level == 1, "{:?} {:?} {}", page, frame, level);
            } else if S::LOG_SIZE == Size2M::LOG_SIZE {
                assert!(level == 2);
            }
        }
        if S::LOG_SIZE != Size4K::LOG_SIZE {
            debug_assert!(flags.bits() & 0b10 == 0);
        }
        // debug_assert!(!flags.contains(PageFlags::PRESENT));
        let flags = flags | PageFlags::PRESENT;
        entry.set(frame, flags);
        page
    }

    pub fn remap<S: PageSize>(&mut self, page: Page<S>, frame: Frame<S>, flags: PageFlags) -> Page<S> {
        let (level, entry) = self.get_entry_create::<S>(page.start());
        if cfg!(debug_assertions) {
            if S::LOG_SIZE == Size4K::LOG_SIZE {
                assert!(level == 1, "{:?} {:?} {}", page, frame, level);
            } else if S::LOG_SIZE == Size2M::LOG_SIZE {
                assert!(level == 2);
            }
        }
        if S::LOG_SIZE != Size4K::LOG_SIZE {
            debug_assert!(flags.bits() & 0b10 == 0);
        }
        let flags = flags | PageFlags::PRESENT;
        entry.set(frame, flags);
        page
    }

    pub fn update_flags<S: PageSize>(&mut self, page: Page<S>, flags: PageFlags) -> Page<S> {
        let (level, entry) = self.get_entry_create::<S>(page.start());
        if cfg!(debug_assertions) {
            if S::LOG_SIZE == Size4K::LOG_SIZE {
                assert!(level == 1, "{:?} {}", page, level);
            } else if S::LOG_SIZE == Size2M::LOG_SIZE {
                assert!(level == 2);
            }
        }
        if S::LOG_SIZE != Size4K::LOG_SIZE {
            debug_assert!(flags.bits() & 0b10 == 0);
        }
        let flags = flags | PageFlags::PRESENT;
        entry.update_flags(flags);
        page
    }

    pub fn unmap<S: PageSize>(&mut self, page: Page<S>) {
        let (level, entry) = self.get_entry(page.start()).unwrap();
        if cfg!(debug_assertions) {
            if S::LOG_SIZE == Size4K::LOG_SIZE {
                assert!(level == 1);
            } else if S::LOG_SIZE == Size2M::LOG_SIZE {
                assert!(level == 2);
            }
        }
        entry.clear();
    }

    pub fn with_temporary_low_table<R>(new_p4_frame: Frame, f: impl Fn(&'static mut PageTable<L4>) -> R) -> R {
        let old_p4_frame = TTBR0_EL1.get();
        TTBR0_EL1.set(new_p4_frame.start().as_usize() as u64);
        crate::mm::paging::invalidate_tlb();
        let r = f(Self::get(false));
        TTBR0_EL1.set(new_p4_frame.start().as_usize() as u64);
        crate::mm::paging::invalidate_tlb();
        r
    }
}

// 4096 / 8
impl <L: TableLevel> PageTable<L> {
    /// Fork a (user) page table hierarchy
    /// 
    /// This will copy all page tables and mark all (non-pagetable) pages as copy-on-write.
    /// 
    /// Special case for kernel stack pages:
    /// we simply redirect them to new frames, but not responsible for the copying
    pub fn fork(&mut self, stack_frames: &[(Frame, Frame); KERNEL_STACK_PAGES]) -> Frame {
        if L::ID == 0 { unreachable!() }

        // println!("Fork {}", L::ID);
        
        // Alloc a new table
        let new_table_frame = frame_allocator::alloc::<Size4K>().unwrap();
        
        // Copy entries & recursively fork children
        let limit = if L::ID == 4 { 511 } else { 512 };
        for i in 0..limit {
            if self.entries[i].present() {
                // println!("- {:?}", self.entries[i].address());
                if L::ID != 1 && self.entries[i].flags().contains(PageFlags::SMALL_PAGE) {
                    // println!("- {:?} page table {}", self.entries[i].address(), L::ID - 1);
                    // This entry is a page table
                    let table = self.next_table(i).unwrap();
                    let flags = self.entries[i].flags();
                    // println!("- {:?} page table {} recursive fork", self.entries[i].address(), L::ID - 1);
                    // println!("table = {:?}", table as *const _);
                    let frame = table.fork(stack_frames);
                    // println!("- {:?} page table {} recursive fork end", self.entries[i].address(), L::ID - 1);
                    let page = crate::mm::map_kernel_temporarily(new_table_frame, PAGE_TABLE_FLAGS);
                    let new_table = unsafe { page.start().as_ref_mut::<Self>() };
                    new_table.entries[i].set(frame, flags);
                    // println!("PT{}({:?})[{}] = T {:?}", L::ID, new_table_frame, i, new_table.entries[i].address());
                } else {
                    // This entry points to a page
                    // Mark as copy-on-write
                    let flags = self.entries[i].flags();
                    let address = self.entries[i].address();
                    let page = crate::mm::map_kernel_temporarily(new_table_frame, PAGE_TABLE_FLAGS);
                    let new_table = unsafe { page.start().as_ref_mut::<Self>() };
                    
                    if L::ID == 1 {
                        if let Some(pos) = stack_frames.iter().position(|x| x.0 == Frame::of(address)) {
                            // This is a kernel stack, remap it
                            let new_stack_frame = stack_frames[pos].1;
                            // println!("Remapped stack {:?} -> {:?}", address, new_stack_frame);
                            debug_assert!(flags.contains(PageFlags::ACCESSED));
                            new_table.entries[i].set(new_stack_frame, flags);
                            // println!("PT{}({:?})[{}] = P {:?}", L::ID, new_table_frame, i, new_table.entries[i].address());
                            continue;
                        }
                    }

                    // This is a normal user page, mark it as copy-on-write
                    new_table.entries[i] = PageTableEntry(self.entries[i].0);
                    // let page = crate::mm::map_kernel_temporarily(new_table_frame, PAGE_TABLE_FLAGS);
                    // let new_table = unsafe { page.start().as_ref_mut::<Self>() };
                    // let old_frame = 
                    // if L::ID == 2 && i == 1 {
                    //     // This is a kernel stack, copy it.
                    //     let new_stack_frame = frame_allocator::alloc().unwrap();
                    //     println!("Remapped stack {:?} -> {:?}", self.entries[i].address(), new_stack_frame);
                    //     debug_assert!(!flags.contains(PageFlags::SMALL_PAGE));
                    //     debug_assert!(flags.contains(PageFlags::ACCESSED));
                    //     new_table.entries[i].set(new_stack_frame, flags);
                    //     println!("PT{}({:?})[{}] = P {:?}", L::ID, new_table_frame, i, new_table.entries[i].address());
                    // } else {
                    //     new_table.entries[i] = PageTableEntry(self.entries[i].0);
                    // }
                }
            }
        }

        if L::ID == 4 {
            // Recursively reference P4 itself
            let page = crate::mm::map_kernel_temporarily(new_table_frame, PAGE_TABLE_FLAGS);
            let new_table = unsafe { page.start().as_ref_mut::<PageTable<L4>>() };
            new_table.entries[511].set(new_table_frame, super::page_table::PAGE_TABLE_FLAGS);
        }

        new_table_frame
    }
}