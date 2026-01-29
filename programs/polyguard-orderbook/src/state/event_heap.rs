use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Maximum events in the heap
pub const MAX_EVENTS: usize = 256;

/// Sentinel for empty slots
pub const FREE_SLOT: u16 = u16::MAX;

/// Event types
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum EventType {
    /// Order was filled
    Fill = 0,
    /// Order was cancelled/removed
    Out = 1,
}

impl Default for EventType {
    fn default() -> Self {
        EventType::Fill
    }
}

/// Fill event - emitted when orders match
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
#[repr(C)]
pub struct FillEvent {
    /// Event type discriminator
    pub event_type: u8,

    /// Taker side (0 = buy, 1 = sell)
    pub taker_side: u8,

    /// Whether maker order is fully filled
    pub maker_out: u8,

    /// Maker's slot in their open orders
    pub maker_slot: u8,

    /// Padding
    pub _padding: [u8; 4],

    /// Event timestamp
    pub timestamp: i64,

    /// Market sequence number
    pub seq_num: u64,

    /// Maker's position account
    pub maker: Pubkey,

    /// Taker's position account
    pub taker: Pubkey,

    /// Fill price (in basis points)
    pub price: u64,

    /// Fill quantity
    pub quantity: u64,

    /// Maker's client order ID
    pub maker_client_order_id: u64,

    /// Taker's client order ID
    pub taker_client_order_id: u64,

    /// Outcome (0 = Yes, 1 = No)
    pub outcome: u8,

    /// Reserved
    pub _reserved: [u8; 7],
}

impl FillEvent {
    pub const SIZE: usize = 1 + 1 + 1 + 1 + 4 + 8 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 1 + 7; // 128 bytes

    pub fn new(
        taker_side: u8,
        maker_out: bool,
        maker_slot: u8,
        timestamp: i64,
        seq_num: u64,
        maker: Pubkey,
        taker: Pubkey,
        price: u64,
        quantity: u64,
        maker_client_order_id: u64,
        taker_client_order_id: u64,
        outcome: u8,
    ) -> Self {
        Self {
            event_type: EventType::Fill as u8,
            taker_side,
            maker_out: if maker_out { 1 } else { 0 },
            maker_slot,
            _padding: [0; 4],
            timestamp,
            seq_num,
            maker,
            taker,
            price,
            quantity,
            maker_client_order_id,
            taker_client_order_id,
            outcome,
            _reserved: [0; 7],
        }
    }

    pub fn is_maker_out(&self) -> bool {
        self.maker_out != 0
    }
}

unsafe impl Zeroable for FillEvent {}
unsafe impl Pod for FillEvent {}

/// Out event - emitted when order is removed
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
#[repr(C)]
pub struct OutEvent {
    /// Event type discriminator
    pub event_type: u8,

    /// Side of the cancelled order (0 = buy, 1 = sell)
    pub side: u8,

    /// Owner's slot in their open orders
    pub owner_slot: u8,

    /// Padding
    pub _padding: [u8; 5],

    /// Event timestamp
    pub timestamp: i64,

    /// Sequence number
    pub seq_num: u64,

    /// Owner's position account
    pub owner: Pubkey,

    /// Remaining quantity that was cancelled
    pub quantity: u64,

    /// Reserved padding (split into smaller arrays for Default)
    pub _reserved1: [u8; 32],
    pub _reserved2: [u8; 24],
}

impl Default for OutEvent {
    fn default() -> Self {
        Self {
            event_type: 0,
            side: 0,
            owner_slot: 0,
            _padding: [0; 5],
            timestamp: 0,
            seq_num: 0,
            owner: Pubkey::default(),
            quantity: 0,
            _reserved1: [0; 32],
            _reserved2: [0; 24],
        }
    }
}

unsafe impl Zeroable for OutEvent {}
unsafe impl Pod for OutEvent {}

impl OutEvent {
    pub const SIZE: usize = FillEvent::SIZE; // Same size for union storage

    pub fn new(
        side: u8,
        owner_slot: u8,
        timestamp: i64,
        seq_num: u64,
        owner: Pubkey,
        quantity: u64,
    ) -> Self {
        Self {
            event_type: EventType::Out as u8,
            side,
            owner_slot,
            _padding: [0; 5],
            timestamp,
            seq_num,
            owner,
            quantity,
            _reserved1: [0; 32],
            _reserved2: [0; 24],
        }
    }
}

/// Event node in the heap (linked list node)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
#[repr(C)]
pub struct EventNode {
    /// Next node in list
    pub next: u16,

    /// Previous node in list
    pub prev: u16,

    /// Padding
    pub _padding: [u8; 4],

    /// Event data (Fill or Out) - split for Default compatibility
    pub data1: [u8; 32],
    pub data2: [u8; 32],
    pub data3: [u8; 32],
    pub data4: [u8; 32],
}

impl Default for EventNode {
    fn default() -> Self {
        Self {
            next: FREE_SLOT,
            prev: FREE_SLOT,
            _padding: [0; 4],
            data1: [0; 32],
            data2: [0; 32],
            data3: [0; 32],
            data4: [0; 32],
        }
    }
}

unsafe impl Zeroable for EventNode {}
unsafe impl Pod for EventNode {}

impl EventNode {
    pub const SIZE: usize = 2 + 2 + 4 + FillEvent::SIZE;

    fn data_ptr(&self) -> *const u8 {
        self.data1.as_ptr()
    }

    fn data_mut_ptr(&mut self) -> *mut u8 {
        self.data1.as_mut_ptr()
    }

    pub fn is_free(&self) -> bool {
        self.data1[0] == 0 && self.next == FREE_SLOT && self.prev == FREE_SLOT
    }

    pub fn event_type(&self) -> EventType {
        match self.data1[0] {
            0 => EventType::Fill,
            1 => EventType::Out,
            _ => EventType::Fill,
        }
    }

    pub fn as_fill(&self) -> Option<FillEvent> {
        if self.data1[0] == EventType::Fill as u8 {
            Some(unsafe { std::ptr::read(self.data_ptr() as *const FillEvent) })
        } else {
            None
        }
    }

    pub fn as_out(&self) -> Option<OutEvent> {
        if self.data1[0] == EventType::Out as u8 {
            Some(unsafe { std::ptr::read(self.data_ptr() as *const OutEvent) })
        } else {
            None
        }
    }

    pub fn set_fill(&mut self, event: &FillEvent) {
        unsafe {
            std::ptr::write(self.data_mut_ptr() as *mut FillEvent, *event);
        }
    }

    pub fn set_out(&mut self, event: &OutEvent) {
        unsafe {
            std::ptr::write(self.data_mut_ptr() as *mut OutEvent, *event);
        }
    }
}

/// Event heap for storing fill and out events
#[account(zero_copy)]
#[repr(C)]
pub struct EventHeap {
    /// Head of used list (oldest event)
    pub used_head: u16,

    /// Tail of used list (newest event)
    pub used_tail: u16,

    /// Head of free list
    pub free_head: u16,

    /// Number of events in the heap
    pub count: u16,

    /// Sequence number for events
    pub seq_num: u64,

    /// Reserved (split for Zeroable)
    pub _reserved1: [u8; 32],
    pub _reserved2: [u8; 24],

    /// Event nodes
    pub nodes: [EventNode; MAX_EVENTS],
}

#[cfg(test)]
impl Default for EventHeap {
    fn default() -> Self {
        Self {
            used_head: FREE_SLOT,
            used_tail: FREE_SLOT,
            free_head: 0,
            count: 0,
            seq_num: 0,
            _reserved1: [0; 32],
            _reserved2: [0; 24],
            nodes: core::array::from_fn(|_| EventNode::default()),
        }
