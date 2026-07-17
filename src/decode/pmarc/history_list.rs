//! History linked list for PMarc decoders
//!
//! Original C version: 2011, 2012, Simon Howard lhasa/lib/pma_common.c
//!
//! Rust version: 2026, Rafał Michalski
use bytemuck::{Zeroable, allocation::zeroed_box};

#[derive(Debug, Clone, Copy, Zeroable)]
pub struct HistoryNode {
    prev: u8,
    next: u8,
}

// Simon Howard:
// History linked list. In the decode stream, codes representing
// characters are not the character itself, but the number of
// nodes to count back in time in the linked list. Every time
// a character is output, it is moved to the front of the linked
// list. The entry point index into the list is the last output
// character, given by history_head;
#[derive(Debug, Clone, Copy, Zeroable)]
pub struct HistoryLinkedList {
    history: [HistoryNode; 256],
    history_head: u8
}

impl HistoryLinkedList {
    /// Return a new, initialized and boxed instance of history list
    pub fn new_boxed() -> Box<Self> {
        let mut history = zeroed_box::<Self>();
        history.initialize();
        history
    }
    /// Initialize the history buffer
    fn initialize(&mut self) {
        // History buffer is initialized to a linear chain
        for (node, i) in self.history.iter_mut().zip(0..=u8::MAX) {
            node.prev = i.wrapping_add(1);
            node.next = i.wrapping_sub(1)
        }
        // Simon Howard:
        // The chain is cut into groups and initially arranged so
        // that the ASCII characters are closest to the start of
        // the chain. This is followed by ASCII control characters,
        // then various other groups.

        self.history_head = 0x20;

        self.history[0x7f].prev = 0x00;  // 0x20 ... 0x7f -> 0x00
        self.history[0x00].next = 0x7f;

        self.history[0x1f].prev = 0xa0;  // 0x00 ... 0x1f -> 0xa0
        self.history[0xa0].next = 0x1f;

        self.history[0xdf].prev = 0x80;  // 0xa0 ... 0xdf -> 0x80
        self.history[0x80].next = 0xdf;

        self.history[0x9f].prev = 0xe0;  // 0x80 ... 0x9f -> 0xe0
        self.history[0xe0].next = 0x9f;

        self.history[0xff].prev = 0x20;  // 0xe0 ... 0xff -> 0x20
        self.history[0x20].next = 0xff;
    }

    /// Look up an entry in the history list, returning the code found
    #[inline]
    pub fn find_in_history_list(&self, count: u8) -> u8 {
        // Start from the last outputted byte.
        let mut code = self.history_head;

        // Simon Howard:
        // Walk along the history chain until we reach the desired
        // node.  If we will have to walk more than half the chain,
        // go the other way around.

        if count < 128 {
            for _ in 0..usize::from(count) {
                code = self.history[usize::from(code)].prev;
            }
        }
        else {
            for _ in 0..usize::from(0u8.wrapping_sub(count)) {
                code = self.history[usize::from(code)].next;
            }
        }

        code
    }

    /// Update history list by moving the specified byte to the head of the queue
    #[inline]
    pub fn update_history_list(&mut self, byte: u8) {
        // No update necessary?
        let head = self.history_head;
        if head == byte {
            return
        }

        // unlink the entry from its current position
        let mut node = self.history[usize::from(byte)];
        self.history[usize::from(node.next)].prev = node.prev;
        self.history[usize::from(node.prev)].next = node.next;

        // link in between the old head and old_head.next
        let old_head = self.history[usize::from(head)];
        node.prev = head;
        node.next = old_head.next;
        self.history[usize::from(byte)] = node;

        self.history[usize::from(old_head.next)].prev = byte;
        self.history[usize::from(head)].next = byte;

        // byte is now the head of the queue
        self.history_head = byte;
    }
}
