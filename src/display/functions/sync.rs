// MIT/Apache2 License

use crate::{
    auto::sync::{DestroyFenceRequest, Fence, TriggerFenceRequest},
    display::{Connection, Display, DisplayVariant},
    sr_request,
};

#[cfg(feature = "async")]
use crate::display::{AsyncConnection, SyncVariant};

impl<Conn: Connection, Var: DisplayVariant> Display<Conn, Var> {
    #[inline]
    pub fn trigger_fence(&self, fence: Fence) -> crate::Result {
        sr_request!(
            self,
            TriggerFenceRequest {
                fence,
                ..Default::default()
            }
        )
    }

    #[inline]
    pub fn free_sync_fence(&self, fence: Fence) -> crate::Result {
        sr_request!(
            self,
            DestroyFenceRequest {
                fence,
                ..Default::default()
            }
        )
    }
}

#[cfg(feature = "async")]
impl<Conn: AsyncConnection> Display<Conn, SyncVariant> {
    #[inline]
    pub async fn trigger_fence_async(&self, fence: Fence) -> crate::Result {
        sr_request!(
            self,
            TriggerFenceRequest {
                fence,
                ..Default::default()
            },
            async
        )
        .await
    }

    #[inline]
    pub async fn free_sync_fence_async(&self, fence: Fence) -> crate::Result {
        sr_request!(
            self,
            DestroyFenceRequest {
                fence,
                ..Default::default()
            },
            async
        )
        .await
    }
}
