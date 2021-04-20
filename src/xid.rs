// MIT/Apache2 License

use super::auto;
use core::cell::Cell;

#[cfg(feature = "thread-safe")]
use core::sync::atomic::{AtomicU32, Ordering};

/// An X11 ID.
#[allow(clippy::upper_case_acronyms)]
pub type XID = u32;

/// A type that acts as a wrapper around an XID.
pub trait XidType {
    fn xid(&self) -> XID;
    fn from_xid(xid: XID) -> Self;

    #[inline]
    fn count_ones(&self) -> usize {
        self.xid().count_ones() as usize
    }
}

impl<T: XidType> auto::AsByteSequence for T {
    #[inline]
    fn size(&self) -> usize {
        self.xid().size()
    }

    #[inline]
    fn as_bytes(&self, bytes: &mut [u8]) -> usize {
        self.xid().as_bytes(bytes)
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Option<(Self, usize)> {
        let (xid, len) = XID::from_bytes(bytes)?;
        Some((Self::from_xid(xid), len))
    }
}

/// XID Generator
#[derive(Debug, Default)]
pub(crate) struct XidGeneratorUnsync {
    pub last: Cell<XID>,
    pub max: Cell<XID>,
    pub inc: XID,
    pub base: XID,
    mask: XID,
}

impl XidGeneratorUnsync {
    #[inline]
    pub fn new(base: XID, mask: XID) -> Self {
        Self {
            last: Cell::new(0),
            max: Cell::new(0),
            base,
            inc: mask & mask.wrapping_neg(),
            mask,
        }
    }

    #[inline]
    pub fn eval_in_place(&self) -> XID {
        self.last.get() | self.base
    }

    #[inline]
    pub fn next(&self) -> Option<XID> {
        if self.last.get() >= self.max.get().wrapping_sub(self.inc).wrapping_add(1) {
            assert_eq!(self.last.get(), self.max.get());
            if self.last.get() == 0 {
                self.max.set(self.mask);
                self.last.set(self.inc);
            } else {
                return None;
            }
        } else {
            self.last.set(self.last.get().wrapping_add(self.inc));
        }

        Some(self.eval_in_place())
    }
}

#[cfg(feature = "thread-safe")]
#[derive(Debug, Default)]
pub(crate) struct XidGeneratorSync {
    last: AtomicU32,
    max: AtomicU32,
    inc: XID,
    base: XID,
    mask: XID,
}

#[cfg(feature = "thread-safe")]
impl XidGeneratorSync {
    #[inline]
    pub fn new(base: XID, mask: XID) -> Self {
        Self {
            last: AtomicU32::new(0),
            max: AtomicU32::new(0),
            base,
            inc: mask & mask.wrapping_neg(),
            mask,
        }
    }

    #[inline]
    pub fn eval_in_place(&self) -> XID {
        self.last.load(Ordering::SeqCst) | self.base
    }

    #[inline]
    pub fn next(&self) -> Option<XID> {
        let last = self.last.load(Ordering::SeqCst);
        let max = self.max.load(Ordering::SeqCst);
        if last >= max.wrapping_sub(self.inc).wrapping_add(1) {
            assert_eq!(last, max);
            if last == 0 {
                self.max.store(self.mask, Ordering::SeqCst);
                self.last.store(self.inc, Ordering::SeqCst);
            } else {
                return None;
            }
        } else {
            self.last
                .store(last.wrapping_add(self.inc), Ordering::SeqCst);
        }

        Some(self.eval_in_place())
    }
}
