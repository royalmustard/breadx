// MIT/Apache2 License

use super::EstablishConnectionFuture;
use crate::{
    auth_info::AuthInfo,
    display::{ReadPacketFuture, SendPacketFuture},
};
use core::task::{Context, Poll};

#[cfg(all(feature = "std", unix))]
use super::unix;
use crate::Fd;
use alloc::vec::Vec;

#[cfg(not(unix))]
use core::pin::Pin;

#[cfg(all(feature = "std", not(unix)))]
use futures_lite::{AsyncRead, AsyncWrite};
#[cfg(all(feature = "std", not(unix)))]
use std::io;

#[cfg(feature = "std")]
use async_io::Async;
#[cfg(feature = "std")]
use std::net::TcpStream;
#[cfg(all(feature = "std", unix))]
use std::os::unix::net::UnixStream;

#[cfg(not(unix))]
use super::standard_fd_warning;

#[cfg(all(not(unix), feature = "std"))]
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(all(not(unix), feature = "tokio-support"))]
use tokio_util::compat::{TokioAsyncReadCompatExt as _, TokioAsyncWriteCompatExt as _};

/// Asynchronous breadx connection.
pub trait AsyncConnection {
    /// Send a packet across the connection in an async manner.
    fn poll_send_packet(
        &mut self,
        bytes: &[u8],
        fds: &mut Vec<Fd>,
        cx: &mut Context<'_>,
        bytes_written: &mut usize,
    ) -> Poll<crate::Result>;

    /// Read a packet from the connection in an async manner.
    fn poll_read_packet(
        &mut self,
        bytes: &mut [u8],
        fds: &mut Vec<Fd>,
        cx: &mut Context<'_>,
        bytes_read: &mut usize,
    ) -> Poll<crate::Result>;

    /// Establish a connection to the server.
    #[inline]
    fn establish_async(
        &mut self,
        auth_info: Option<AuthInfo>,
    ) -> EstablishConnectionFuture<'_, Self> {
        EstablishConnectionFuture::run(self, auth_info)
    }
}

impl<C: AsyncConnection + ?Sized> AsyncConnection for &mut C {
    #[inline]
    fn poll_send_packet(
        &mut self,
        bytes: &[u8],
        fds: &mut Vec<Fd>,
        cx: &mut Context<'_>,
        bytes_written: &mut usize,
    ) -> Poll<crate::Result> {
        (**self).poll_send_packet(bytes, fds, cx, bytes_written)
    }

    #[inline]
    fn poll_read_packet(
        &mut self,
        bytes: &mut [u8],
        fds: &mut Vec<Fd>,
        cx: &mut Context<'_>,
        bytes_read: &mut usize,
    ) -> Poll<crate::Result> {
        (**self).poll_read_packet(bytes, fds, cx, bytes_read)
    }
}

/// Extension trait for `AsyncConnection` that provides futures.
pub trait AsyncConnectionExt {
    fn read_packet_async<'a, 'b, 'c>(
        &'a mut self,
        bytes: &'b mut [u8],
        fds: &'c mut Vec<Fd>,
    ) -> ReadPacketFuture<'a, 'b, 'c, Self>;
    fn send_packet_async<'a, 'b, 'c>(
        &'a mut self,
        bytes: &'b [u8],
        fds: &'c mut Vec<Fd>,
    ) -> SendPacketFuture<'a, 'b, 'c, Self>;
}

impl<C: AsyncConnection + ?Sized> AsyncConnectionExt for C {
    #[inline]
    fn read_packet_async<'a, 'b, 'c>(
        &'a mut self,
        bytes: &'b mut [u8],
        fds: &'c mut Vec<Fd>,
    ) -> ReadPacketFuture<'a, 'b, 'c, Self> {
        ReadPacketFuture::run(self, bytes, fds)
    }
    #[inline]
    fn send_packet_async<'a, 'b, 'c>(
        &'a mut self,
        bytes: &'b [u8],
        fds: &'c mut Vec<Fd>,
    ) -> SendPacketFuture<'a, 'b, 'c, Self> {
        SendPacketFuture::run(self, bytes, fds)
    }
}

macro_rules! unix_aware_async_connection_impl {
    ($name: ty) => {
        impl AsyncConnection for $name {
            #[inline]
            fn poll_send_packet(
                &mut self,
                bytes: &[u8],
                fds: &mut Vec<Fd>,
                cx: &mut Context<'_>,
                bytes_written: &mut usize,
            ) -> Poll<crate::Result> {
                cfg_if::cfg_if! {
                    if #[cfg(unix)] {
                        unix::poll_send_packet_unix(self, bytes, fds, cx, bytes_written)
                    } else {
                        standard_fd_warning(fds);
                        let mut bytes = bytes;
                        while !bytes.is_empty() {
                            match Pin::new(&mut *self).poll_write(cx, bytes) {
                                Poll::Pending => return Poll::Pending,
                                Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                                Poll::Ready(Ok(0)) => {
                                    let err: io::Error = io::ErrorKind::WriteZero.into();
                                    return Poll::Ready(Err(err.into()));
                                }
                                Poll::Ready(Ok(n)) => {
                                    bytes = &bytes[n..];
                                    *bytes_written += n;
                                }
                            }
                        }

                        Poll::Ready(Ok(()))
                    }
                }
            }

            #[inline]
            fn poll_read_packet(
                &mut self,
                bytes: &mut [u8],
                fds: &mut Vec<Fd>,
                cx: &mut Context<'_>,
                bytes_read: &mut usize,
            ) -> Poll<crate::Result> {
                cfg_if::cfg_if! {
                    if #[cfg(unix)] {
                        unix::poll_read_packet_unix(self, bytes, fds, cx, bytes_read)
                    } else {
                        let _ = fds;
                        let mut bytes = bytes;
                        while !bytes.is_empty() {
                            match Pin::new(&mut *self).poll_read(cx, bytes) {
                                Poll::Pending => return Poll::Pending,
                                Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                                Poll::Ready(Ok(0)) => {
                                    let err: io::Error = io::ErrorKind::UnexpectedEof.into();
                                    return Poll::Ready(Err(err.into()));
                                }
                                Poll::Ready(Ok(n)) => {
                                    bytes = &mut bytes[n..];
                                    *bytes_read += n;
                                }
                            }
                        }

                        Poll::Ready(Ok(()))
                    }
                }
            }
        }
    };
}

// NOTE: In the past, these were "async_net::TcpStream" and "async_net::os::unix::UnixStream".
//       However, neither implement AsyncRead or AsyncWrite for immutable access. The underlying
//       "Async" primitive, however, does.

#[cfg(feature = "std")]
unix_aware_async_connection_impl! { Async<TcpStream> }
#[cfg(all(feature = "std", unix))]
unix_aware_async_connection_impl! { Async<UnixStream> }
#[cfg(feature = "std")]
unix_aware_async_connection_impl! { &Async<TcpStream> }
#[cfg(all(feature = "std", unix))]
unix_aware_async_connection_impl! { &Async<UnixStream> }
#[cfg(all(feature = "tokio-support", unix))]
unix_aware_async_connection_impl! { tokio::net::UnixStream }

#[cfg(feature = "tokio-support")]
impl AsyncConnection for tokio::net::TcpStream {
    #[inline]
    fn poll_send_packet(
        &mut self,
        bytes: &[u8],
        fds: &mut Vec<Fd>,
        cx: &mut Context<'_>,
        bytes_written: &mut usize,
    ) -> Poll<crate::Result> {
        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                unix::poll_send_packet_unix(self, bytes, fds, cx, bytes_written)
            } else {
                standard_fd_warning(fds);
                let mut bytes = bytes;
                let mut this = self.compat();
                while !bytes.is_empty() {
                    match Pin::new(&mut this).poll_write(cx, bytes) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Ready(Ok(0)) => {
                            let err: io::Error = io::ErrorKind::WriteZero.into();
                            return Poll::Ready(Err(err.into()));
                        }
                        Poll::Ready(Ok(n)) => {
                            bytes = &bytes[n..];
                            *bytes_written += n;
                        }
                    }
                }

                Poll::Ready(Ok(()))
            }
        }
    }

    #[inline]
    fn poll_read_packet(
        &mut self,
        bytes: &mut [u8],
        fds: &mut Vec<Fd>,
        cx: &mut Context<'_>,
        bytes_read: &mut usize,
    ) -> Poll<crate::Result> {
        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                unix::poll_read_packet_unix(self, bytes, fds, cx, bytes_read)
            } else {
                let _ = fds;
                let mut bytes = bytes;
                let mut this = self.compat();
                while !bytes.is_empty() {
                    match Pin::new(&mut this).poll_read(cx, bytes) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Ready(Ok(0)) => {
                            let err: io::Error = io::ErrorKind::UnexpectedEof.into();
                            return Poll::Ready(Err(err.into()));
                        }
                        Poll::Ready(Ok(n)) => {
                            bytes = &mut bytes[n..];
                            *bytes_read += n;
                        }
                    }
                }

                Poll::Ready(Ok(()))
            }
        }
    }
}
