// MIT/Apache2 License

use super::{Connection, DisplayVariant, PendingRequest, PendingRequestFlags, RequestWorkaround};
use crate::{event::Event, util::cycled_zeroes, Fd, XID};
use alloc::{boxed::Box, vec, vec::Vec};
use core::iter;
use tinyvec::TinyVec;

#[cfg(feature = "async")]
use super::{AsyncConnection, SyncVariant};
#[cfg(feature = "async")]
use core::mem;

const TYPE_ERROR: u8 = 0;
const TYPE_REPLY: u8 = 1;
const GENERIC_EVENT: u8 = 35;
const GE_MASK: u8 = 0x7f;

struct Bomb;

impl Drop for Bomb {
    #[inline]
    fn drop(&mut self) {
        panic!("Future dropped mid-read!");
    }
}

impl<Conn, Variant: DisplayVariant> super::Display<Conn, Variant> {
    // process a set of 32 bytes into the system
    #[inline]
    fn process_bytes(&self, mut bytes: TinyVec<[u8; 32]>, fds: Box<[Fd]>) -> crate::Result {
        // get the sequence number
        let sequence = u16::from_ne_bytes([bytes[2], bytes[3]]);
        #[cfg(debug_assertions)]
        log::trace!("Found response bytes: {}", &bytes);

        if bytes[0] == TYPE_REPLY {
            log::debug!("Received bytes of type REPLY");

            let pereq = self
                .variant
                .remove_pending_request(sequence)
                .ok_or(crate::BreadError::NoMatchingRequest(sequence as _))?;

            // if we're discarding the reply, skip the conversion process
            if pereq.flags.discard_reply {
                log::debug!("Discarding input for request");
            } else {
                // convert bytes to a boxed slice
                bytes.move_to_the_heap();
                let bytes = match bytes {
                    TinyVec::Heap(v) => v.into_boxed_slice(),
                    TinyVec::Inline(_) => unreachable!(),
                };

                self.variant.insert_pending_reply(sequence, bytes, fds);
            }
        } else if bytes[0] == TYPE_ERROR {
            // if it's all zeroes, the X connection has closed and the programmer
            // forgot to check for the close message
            // we're fine to error out here
            if !bytes.iter().copied().any(|x| x != 0) {
                return Err(crate::BreadError::ClosedConnection);
            }

            let err = crate::BreadError::from_x_error(bytes);

            // if we have a pending request with the given sequence, remove that pending
            // request and put that in the pending requests
            match self.variant.remove_pending_request(sequence) {
                Some(_) => {
                    self.variant.insert_pending_error(sequence, err);
                }
                // if there is no pending request, we've probably done something
                // weird somewhere
                // default to returning the error from the request that's currently
                // calling wait()
                None => return Err(err),
            }
        } else {
            log::debug!("Received bytes of type EVENT");
            // this is an event
            let event = Event::from_bytes(bytes)?;
            // if it doesn't fit in any of the special event queues, put it in the main one
            if let Err(event) = self.filter_into_special_event(event) {
                self.variant.queue_event(event);
            }
        }

        Ok(())
    }

    // if necessary, fix the GLX FbConfigs bug
    // I already ranted about this in output.rs
    #[inline]
    fn fix_glx_workaround(&self, bytes: &mut TinyVec<[u8; 32]>) -> crate::Result<()> {
        // this will only ever apply to replies
        if bytes[0] == TYPE_REPLY {
            // grab the pending request
            let sequence = u16::from_ne_bytes([bytes[2], bytes[3]]);
            let pereq = self
                .variant
                .get_pending_request(sequence)
                .ok_or(crate::BreadError::NoMatchingRequest(sequence as _))?;

            if let RequestWorkaround::GlxFbconfigBug = pereq.flags.workaround {
                log::debug!("Applying GLX FbConfig workaround to reply");

                // length is the 1st u32, numVisuals is the 2nd u32, numProps in the 3rd u32
                // numVisuals is 8..12, numProps is 12..16, length is 4..8
                let (mut visuals, mut props): ([u8; 4], [u8; 4]) = ([0; 4], [0; 4]);
                visuals.copy_from_slice(&bytes[8..12]);
                props.copy_from_slice(&bytes[12..16]);

                let (visuals, props) = (u32::from_ne_bytes(visuals), u32::from_ne_bytes(props));

                let length = (visuals * props * 2).to_ne_bytes();
                (&mut bytes[4..8]).copy_from_slice(&length);
            }
        }

        Ok(())
    }

    // add an entry to the pending elements linked list
    #[inline]
    pub(crate) fn expect_reply(&self, req: u16, flags: PendingRequestFlags) {
        let pereq = PendingRequest {
            request: req as _,
            flags,
        };
        self.variant.insert_pending_request(req, pereq);
    }

    #[inline]
    fn filter_into_special_event(&self, event: Event) -> Result<XID, Event> {
        // if the event's already differentiated, it's not a special event
        let evbytes = match event.as_byte_slice() {
            Some(evbytes) => evbytes,
            None => return Err(event),
        };

        // the first byte will always indicate an XGE event
        if evbytes[0] & 0x7F != GENERIC_EVENT as _ {
            return Err(event);
        }

        let mut eid_bytes: [u8; 4] = [0; 4];
        eid_bytes.copy_from_slice(&evbytes[12..16]);
        let my_eid = u32::from_ne_bytes(eid_bytes);

        self.variant.try_queue_special_event(event, my_eid)
    }
}

impl<Conn: Connection, Variant: DisplayVariant> super::Display<Conn, Variant> {
    // wait for bytes to appear
    #[inline]
    pub(crate) fn wait(&self) -> crate::Result {
        log::debug!("Running wait cycle");
        // replies, errors, and events are all in units of 32 bytes
        let mut bytes: TinyVec<[u8; 32]> = cycled_zeroes(32);
        let mut fds: Vec<Fd> = vec![];
        self.connection()?.read_packet(&mut bytes, &mut fds)?;

        self.fix_glx_workaround(&mut bytes)?;

        // in certain cases, we may have to read more bytes
        if let Some(ab) = additional_bytes(&bytes[..8]) {
            if ab != 0 {
                bytes.extend(iter::repeat(0).take(ab * 4));

                log::debug!("Waiting for {} additional bytes", ab * 4);
                self.connection()?.read_packet(&mut bytes[32..], &mut fds)?;
                log::debug!("Ending wait with {} additional bytes", ab * 4);
            }
        }

        self.process_bytes(bytes, fds.into_boxed_slice())
    }
}

#[cfg(feature = "async")]
impl<Conn: AsyncConnection> super::Display<Conn, SyncVariant> {
    // wait for bytes to appear, async redox
    #[inline]
    pub(crate) async fn wait_async(&self) -> crate::Result {
        #[cfg(debug_assertions)]
        log::debug!("Beginning wait cycle.");
        // see above function for more information
        let mut bytes: TinyVec<[u8; 32]> = cycled_zeroes(32);
        let mut fds: Vec<Fd> = vec![];

        let _bomb = Bomb;
        self.connection()?.read_packet(&mut bytes, &mut fds).await?;
        mem::forget(_bomb);

        self.fix_glx_workaround(&mut bytes)?;

        if let Some(ab) = additional_bytes(&bytes[..8]) {
            bytes.extend(iter::repeat(0).take(ab * 4));
            self.connection()?
                .read_packet(&mut bytes[32..], &mut fds)
                .await?;
        }

        self.process_bytes(bytes, fds.into_boxed_slice())
    }
}

#[inline]
fn additional_bytes(bytes: &[u8]) -> Option<usize> {
    if bytes[0] == TYPE_REPLY || bytes[0] & GE_MASK == GENERIC_EVENT {
        let mut len_bytes = [0; 4];
        len_bytes.copy_from_slice(&bytes[4..8]);
        Some(u32::from_ne_bytes(len_bytes) as usize)
    } else {
        None
    }
}
