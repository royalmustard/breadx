// MIT/Apache2 License

use crate::{
    auto::xproto::{Cursor, FreeCursorRequest},
    display::{Connection, Display, DisplayVariant},
    sr_request,
};

#[cfg(feature = "async")]
use crate::display::{AsyncConnection, SyncVariant};

impl Cursor {
    #[inline]
    pub fn free<Conn: Connection, Var: DisplayVariant>(
        self,
        dpy: &mut Display<Conn, Var>,
    ) -> crate::Result {
        sr_request!(
            dpy,
            FreeCursorRequest {
                cursor: self,
                ..Default::default()
            }
        )
    }

    #[cfg(feature = "async")]
    #[inline]
    pub async fn free_async<Conn: AsyncConnection>(
        self,
        dpy: &mut Display<Conn, SyncVariant>,
    ) -> crate::Result {
        sr_request!(
            dpy,
            FreeCursorRequest {
                cursor: self,
                ..Default::default()
            },
            async
        )
        .await
    }
}
