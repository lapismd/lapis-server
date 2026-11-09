use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::AsyncWrite;
use tokio_util::sync::CancellationToken;

/// The SDK reports send errors to requests, but a broken stdout must also end
/// the subprocess service even while its stdin read remains pending.
pub struct Output<W> {
    pub inner: W,
    pub closed: CancellationToken,
}
impl<W: AsyncWrite + Unpin> AsyncWrite for Output<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, bytes);
        if matches!(result, Poll::Ready(Err(_)))
            || !bytes.is_empty() && matches!(result, Poll::Ready(Ok(0)))
        {
            self.closed.cancel();
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let result = Pin::new(&mut self.inner).poll_flush(cx);
        if matches!(result, Poll::Ready(Err(_))) {
            self.closed.cancel();
        }
        result
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
