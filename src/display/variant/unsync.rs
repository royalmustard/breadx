// MIT/Apache2 License

use super::{
    super::{PendingRequest, EXT_KEY_SIZE},
    DisplayVariant,
};
use crate::{
    error::BreadError,
    event::Event,
    xid::{XidGeneratorUnsync, XID},
    Fd,
};
use alloc::{boxed::Box, collections::VecDeque};
use core::{
    cell::{Cell, RefCell},
    num::NonZeroU32,
};
use hashbrown::HashMap;

/// The non-thread-safe variant of the display.
#[derive(Debug)]
pub struct UnsyncVariant {
    state: RefCell<ConnectionState>,
    request_number: Cell<u16>,
    xid: XidGeneratorUnsync,
    wm_protocols_atom: Cell<Option<NonZeroU32>>,
    checked: Cell<bool>,
}

#[derive(Debug)]
struct ConnectionState {
    event_queue: VecDeque<Event>,
    pending_requests: HashMap<u16, PendingRequest>,
    pending_errors: HashMap<u16, BreadError>,
    pending_replies: HashMap<u16, PendingReply>,
    special_event_queues: HashMap<XID, VecDeque<Event>>,
    extensions: HashMap<[u8; EXT_KEY_SIZE], u8>,
}

#[derive(Debug)]
struct PendingReply(Box<[u8]>, Box<[Fd]>);

impl DisplayVariant for UnsyncVariant {
    #[inline]
    fn new() -> Self {
        Self {
            state: RefCell::new(ConnectionState {
                event_queue: VecDeque::new(),
                pending_requests: HashMap::new(),
                pending_errors: HashMap::new(),
                pending_replies: HashMap::new(),
                special_event_queues: HashMap::with_capacity(1),
                extensions: HashMap::new(),
            }),
            request_number: Cell::new(0),
            xid: Default::default(),
            wm_protocols_atom: Cell::new(None),
            checked: Cell::new(true),
        }
    }

    #[inline]
    fn set_xid_base(&mut self, base: XID, mask: XID) {
        self.xid = XidGeneratorUnsync::new(base, mask);
    }

    #[inline]
    fn queue_event(&self, ev: Event) {
        self.state.borrow_mut().event_queue.push_back(ev);
    }

    #[inline]
    fn pop_event(&self) -> Option<Event> {
        self.state.borrow_mut().event_queue.pop_front()
    }

    #[inline]
    fn register_special_event(&self, eid: XID) {
        self.state
            .borrow_mut()
            .special_event_queues
            .insert(eid, VecDeque::new());
    }

    #[inline]
    fn unregister_special_event(&self, eid: XID) {
        self.state.borrow_mut().special_event_queues.remove(&eid);
    }

    #[inline]
    fn is_queue_registered(&self, eid: XID) -> bool {
        self.state
            .borrow_mut()
            .special_event_queues
            .contains_key(&eid)
    }

    #[inline]
    fn get_special_event(&self, eid: XID) -> Option<Event> {
        self.state
            .borrow_mut()
            .special_event_queues
            .get_mut(&eid)
            .unwrap()
            .pop_front()
    }

    #[inline]
    fn generate_xid(&self) -> crate::Result<XID> {
        self.xid.next().ok_or(BreadError::NoXID)
    }

    #[inline]
    fn checked(&self) -> bool {
        self.checked.get()
    }

    #[inline]
    fn set_checked(&self, checked: bool) {
        self.checked.set(checked);

        if !checked {
            self.state
                .borrow_mut()
                .pending_requests
                .retain(|_, val| !val.flags.checked);
        }
    }

    #[inline]
    fn pending_request_contains(&self, sequence: u16) -> bool {
        self.state
            .borrow()
            .pending_requests
            .contains_key(&sequence)
    }

    #[inline]
    fn get_pending_request(&self, sequence: u16) -> Option<PendingRequest> {
        self.state
            .borrow()
            .pending_requests
            .get(&sequence)
            .cloned()
    }

    #[inline]
    fn remove_pending_request(&self, sequence: u16) -> Option<PendingRequest> {
        self.state
            .borrow_mut()
            .pending_requests
            .remove(&sequence)
    }

    #[inline]
    fn retrieve_pending_error(&self, sequence: u16) -> Option<BreadError> {
        self.state
            .borrow_mut()
            .pending_errors
            .remove(&sequence)
    }

    #[allow(clippy::type_complexity)]
    #[inline]
    fn retrieve_pending_reply(&self, sequence: u16) -> Option<(Box<[u8]>, Box<[Fd]>)> {
        match self
            .state
            .borrow_mut()
            .pending_replies
            .remove(&sequence)
        {
            None => None,
            Some(PendingReply(bytes, fds)) => Some((bytes, fds)),
        }
    }

    #[inline]
    fn insert_pending_request(&self, req: u16, pr: PendingRequest) {
        if self
            .state
            .borrow_mut()
            .pending_requests
            .insert(req, pr)
            .is_some()
            && cfg!(debug_assertions)
        {
            panic!("Too many requests! Two matching requests");
        }
    }

    #[inline]
    fn insert_pending_error(&self, sequence: u16, err: BreadError) {
        if self
            .state
            .borrow_mut()
            .pending_errors
            .insert(sequence, err)
            .is_some()
            && cfg!(debug_assertions)
        {
            panic!("Too many errors! Sequence overflow");
        }
    }

    #[inline]
    fn insert_pending_reply(&self, sequence: u16, bytes: Box<[u8]>, fds: Box<[Fd]>) {
        if self
            .state
            .borrow_mut()
            .pending_replies
            .insert(sequence, PendingReply(bytes, fds))
            .is_some()
            && cfg!(debug_assertions)
        {
            panic!("Too many replies! Sequence overflow");
        }
    }

    #[inline]
    fn try_queue_special_event(&self, event: Event, eid: XID) -> Result<XID, Event> {
        let mut event = Some(event);

        self.state
            .borrow_mut()
            .special_event_queues
            .iter_mut()
            .find_map(|(matching_eid, queue)| {
                if *matching_eid == eid {
                    queue.push_back(event.take().expect("Infallible"));
                    Some(eid)
                } else {
                    None
                }
            })
            .ok_or_else(|| event.unwrap())
    }

    #[inline]
    fn next_request_number(&self) -> u16 {
        let req = self.request_number.get();
        self.request_number.set(req.wrapping_add(1));
        req
    }

    #[inline]
    fn advance_request_number(&self) {}

    #[inline]
    fn get_ext_opcode(&self, sarr: &[u8; EXT_KEY_SIZE]) -> Option<u8> {
        self.state.borrow_mut().extensions.get(sarr).copied()
    }

    #[inline]
    fn insert_ext_opcode(&self, sarr: [u8; EXT_KEY_SIZE], code: u8) {
        self.state.borrow_mut().extensions.insert(sarr, code);
    }

    #[inline]
    fn wm_protocols_atom(&self) -> Option<NonZeroU32> {
        self.wm_protocols_atom.get()
    }

    #[inline]
    fn set_wm_protocols_atom(&self, n: NonZeroU32) {
        self.wm_protocols_atom.set(Some(n))
    }
}
