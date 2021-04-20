// MIT/Apache2 License

use crate::{
    auto::{
        xfixes::{CreateRegionRequest, DestroyRegionRequest, Region},
        xproto::Rectangle,
    },
    display::{Connection, Display, DisplayVariant},
    sr_request,
};
use alloc::vec::Vec;

#[cfg(feature = "async")]
use crate::display::{AsyncConnection, SyncVariant};

impl<Conn: Connection, Var: DisplayVariant> Display<Conn, Var> {
    #[inline]
    pub fn create_region(&self, rectangles: Vec<Rectangle>) -> crate::Result<Region> {
        let xid = Region::const_from_xid(self.generate_xid()?);
        sr_request!(
            self,
            CreateRegionRequest {
                region: xid,
                rectangles,
                ..Default::default()
            }
        )?;
        Ok(xid)
    }
}

#[cfg(feature = "async")]
impl<Conn: AsyncConnection> Display<Conn, SyncVariant> {
    #[inline]
    pub async fn create_region_async(&self, rectangles: Vec<Rectangle>) -> crate::Result<Region> {
        let xid = Region::const_from_xid(self.generate_xid()?);
        sr_request!(
            self,
            CreateRegionRequest {
                region: xid,
                rectangles,
                ..Default::default()
            },
            async
        )
        .await?;
        Ok(xid)
    }
}

impl Region {
    #[inline]
    pub fn destroy<Conn: Connection, Var: DisplayVariant>(self, dpy: &mut Display<Conn, Var>) -> crate::Result {
        sr_request!(
            dpy,
            DestroyRegionRequest {
                region: self,
                ..Default::default()
            }
        )
    }

    #[cfg(feature = "async")]
    #[inline]
    pub async fn destroy_async<Conn: AsyncConnection>(
        self,
        dpy: &mut Display<Conn, SyncVariant>,
    ) -> crate::Result {
        sr_request!(
            dpy,
            DestroyRegionRequest {
                region: self,
                ..Default::default()
            },
            async
        )
        .await
    }
}
