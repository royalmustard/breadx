// MIT/Apache2 License

use crate::{
    auto::xproto::{FreePixmapRequest, Pixmap},
    display::{Connection, Display, DisplayVariant},
    sr_request,
};

#[cfg(feature = "async")]
use crate::display::{AsyncConnection, SyncVariant};

impl Pixmap {
    /// Free the memory used by a pixmap.
    #[inline]
    pub fn free<Conn: Connection, Var: DisplayVariant>(
        self,
        dpy: &Display<Conn, Var>,
    ) -> crate::Result {
        sr_request!(
            dpy,
            FreePixmapRequest {
                pixmap: self,
                ..Default::default()
            }
        )
    }

    /// Free the memory used by a pixmap, async redox.
    #[cfg(feature = "async")]
    #[inline]
    pub async fn free_async<Conn: AsyncConnection>(
        self,
        dpy: &Display<Conn, SyncVariant>,
    ) -> crate::Result {
        sr_request!(
            dpy,
            FreePixmapRequest {
                pixmap: self,
                ..Default::default()
            },
            async
        )
        .await
    }
}
