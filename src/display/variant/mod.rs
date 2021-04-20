// MIT/Apache2 License

use super::PendingRequest;
use crate::{display::EXT_KEY_SIZE, error::BreadError, event::Event, xid::XID, Fd};
use alloc::boxed::Box;
use core::num::NonZeroU32;

mod unsync;
pub use unsync::*;

#[cfg(feature = "thread-safe")]
mod sync;
#[cfg(feature = "thread-safe")]
pub use sync::*;

/// Whether or not the display should use thread-safe sync mechanisms or thread-unsync mechanisms.
pub trait DisplayVariant: Send {
    fn new() -> Self;
    fn set_xid_base(&mut self, base: XID, mask: XID);
    fn queue_event(&self, ev: Event);
    fn pop_event(&self) -> Option<Event>;
    fn register_special_event(&self, eid: XID);
    fn unregister_special_event(&self, eid: XID);
    fn is_queue_registered(&self, eid: XID) -> bool;
    fn get_special_event(&self, eid: XID) -> Option<Event>;
    fn generate_xid(&self) -> crate::Result<XID>;
    fn checked(&self) -> bool;
    fn set_checked(&self, checked: bool);
    fn pending_request_contains(&self, sequence: u16) -> bool;
    fn get_pending_request(&self, sequence: u16) -> Option<PendingRequest>;
    fn remove_pending_request(&self, sequence: u16) -> Option<PendingRequest>;
    fn retrieve_pending_error(&self, sequence: u16) -> Option<BreadError>;
    #[allow(clippy::type_complexity)]
    fn retrieve_pending_reply(&self, sequence: u16) -> Option<(Box<[u8]>, Box<[Fd]>)>;
    fn insert_pending_request(&self, sequence: u16, pr: PendingRequest);
    fn insert_pending_error(&self, sequence: u16, err: BreadError);
    fn insert_pending_reply(&self, sequence: u16, bytes: Box<[u8]>, fds: Box<[Fd]>);
    fn try_queue_special_event(&self, event: Event, eid: XID) -> Result<XID, Event>;
    fn next_request_number(&self) -> u16;
    fn advance_request_number(&self);
    fn get_ext_opcode(&self, sarr: &[u8; EXT_KEY_SIZE]) -> Option<u8>;
    fn insert_ext_opcode(&self, sarr: [u8; EXT_KEY_SIZE], code: u8);
    fn wm_protocols_atom(&self) -> Option<NonZeroU32>;
    fn set_wm_protocols_atom(&self, n: NonZeroU32);
}
