// MIT/Apache2 License

use super::{
    super::{PendingRequest, EXT_KEY_SIZE},
    DisplayVariant,
};
use crate::{
    error::BreadError,
    event::Event,
    xid::{XidGeneratorSync, XID},
    Fd,
};
use alloc::{boxed::Box, collections::VecDeque};
use concurrent_queue::ConcurrentQueue;
use core::{
    num::NonZeroU32,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU16, Ordering},
};
use dashmap::DashMap;

#[derive(Debug)]
pub struct SyncVariant {
    event_queue: ConcurrentQueue<Event>,
    pending_requests: DashMap<u16, PendingRequest>,
    pending_errors: DashMap<u16, BreadError>,
    pending_replies: DashMap<u16, PendingReply>,
    special_event_queues: DashMap<XID, VecDeque<Event>>,
    extensions: DashMap<[u8; EXT_KEY_SIZE], u8>,
    request_number: AtomicU16,
    xid: XidGeneratorSync,
    checked: AtomicBool,
    wm_protocols_atom: AtomicU32,
}

#[derive(Debug)]
struct PendingReply(Box<[u8]>, Box<[Fd]>);

impl DisplayVariant for SyncVariant {
    #[inline]
    fn new() -> Self {
        Self {
            event_queue: ConcurrentQueue::unbounded(),
            pending_requests: DashMap::new(),
            pending_errors: DashMap::new(),
            pending_replies: DashMap::new(),
            special_event_queues: DashMap::with_capacity(1),
            extensions: DashMap::new(),
            request_number: AtomicU16::new(0),
            xid: Default::default(),
            checked: AtomicBool::new(true),
            wm_protocols_atom: AtomicU32::new(0),
        }
    }

    #[inline]
    fn set_xid_base(&mut self, base: XID, mask: XID) {
        self.xid = XidGeneratorSync::new(base, mask);
    }

    #[inline]
    fn queue_event(&self, ev: Event) {
        self.event_queue
            .push(ev)
            .expect("Queue should not be closed at this point");
    }

    #[inline]
    fn pop_event(&self) -> Option<Event> {
        self.event_queue.pop().ok()
    }

    #[inline]
    fn register_special_event(&self, eid: XID) {
        self.special_event_queues.insert(eid, VecDeque::new());
    }

    #[inline]
    fn unregister_special_event(&self, eid: XID) {
        self.special_event_queues.remove(&eid);
    }

    #[inline]
    fn is_queue_registered(&self, eid: XID) -> bool {
        self.special_event_queues.contains_key(&eid)
    }

    #[inline]
    fn get_special_event(&self, eid: XID) -> Option<Event> {
        self.special_event_queues.get_mut(&eid).unwrap().pop_front()
    }

    #[inline]
    fn generate_xid(&self) -> crate::Result<XID> {
        self.xid.next().ok_or(BreadError::NoXID)
    }

    #[inline]
    fn checked(&self) -> bool {
        self.checked.load(Ordering::SeqCst)
    }

    #[inline]
    fn set_checked(&self, checked: bool) {
        self.checked.store(checked, Ordering::SeqCst);

        if !checked {
            self.pending_requests.retain(|_, val| !val.flags.checked);
        }
    }

    #[inline]
    fn pending_request_contains(&self, sequence: u16) -> bool {
        self.pending_requests.contains_key(&sequence)
    }

    #[inline]
    fn get_pending_request(&self, sequence: u16) -> Option<PendingRequest> {
        match self.pending_requests.get(&sequence) {
            Some(d) => Some(*d),
            None => None,
        }
    }

    #[inline]
    fn remove_pending_request(&self, sequence: u16) -> Option<PendingRequest> {
        self.pending_requests.remove(&sequence).map(|d| d.1)
    }

    #[inline]
    fn retrieve_pending_error(&self, sequence: u16) -> Option<BreadError> {
        self.pending_errors.remove(&sequence).map(|d| d.1)
    }

    #[allow(clippy::type_complexity)]
    #[inline]
    fn retrieve_pending_reply(&self, sequence: u16) -> Option<(Box<[u8]>, Box<[Fd]>)> {
        match self.pending_replies.remove(&sequence) {
            None => None,
            Some((_, PendingReply(bytes, fds))) => Some((bytes, fds)),
        }
    }

    #[inline]
    fn insert_pending_request(&self, req: u16, pr: PendingRequest) {
        if self.pending_requests.insert(req, pr).is_some() && cfg!(debug_assertions) {
            panic!("Too many request! Two matching requests");
        }
    }

    #[inline]
    fn insert_pending_error(&self, sequence: u16, err: BreadError) {
        if self.pending_errors.insert(sequence, err).is_some() && cfg!(debug_assertions) {
            panic!("Too many errors! Two matching errors");
        }
    }

    #[inline]
    fn insert_pending_reply(&self, sequence: u16, bytes: Box<[u8]>, fds: Box<[Fd]>) {
        if self
            .pending_replies
            .insert(sequence, PendingReply(bytes, fds))
            .is_some()
            && cfg!(debug_assertions)
        {
            panic!("Too many replies! Two matching replies");
        }
    }

    #[inline]
    fn try_queue_special_event(&self, event: Event, eid: XID) -> Result<XID, Event> {
        let mut event = Some(event);

        self.special_event_queues
            .iter_mut()
            .find_map(|mut p| {
                let (matching_eid, queue) = p.pair_mut();
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
        let next_rn = self.request_number.fetch_add(1, Ordering::SeqCst);
        next_rn 
    }

    #[inline]
    fn advance_request_number(&self) {}

    #[inline]
    fn get_ext_opcode(&self, sarr: &[u8; EXT_KEY_SIZE]) -> Option<u8> {
        match self.extensions.get(sarr) {
            Some(d) => Some(*d),
            None => None,
        }
    }

    #[inline]
    fn insert_ext_opcode(&self, sarr: [u8; EXT_KEY_SIZE], code: u8) {
        self.extensions.insert(sarr, code);
    }

    #[inline]
    fn wm_protocols_atom(&self) -> Option<NonZeroU32> {
        NonZeroU32::new(self.wm_protocols_atom.load(Ordering::SeqCst))
    }

    #[inline]
    fn set_wm_protocols_atom(&self, n: NonZeroU32) {
        self.wm_protocols_atom.store(n.get(), Ordering::SeqCst);
    }
}
