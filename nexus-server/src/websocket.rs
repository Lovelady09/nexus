//! WebSocket adapter for protocol compatibility
//!
//! This module provides a `WebSocketAdapter` that wraps a `WebSocketStream` and implements
//! `AsyncRead` and `AsyncWrite`, allowing WebSocket connections to be used with the existing
//! frame-based protocol handlers.
//!
//! The adapter buffers incoming WebSocket binary messages and presents them as a byte stream
//! for reading. For writing, it buffers bytes and sends them as WebSocket binary messages
//! when flushed.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::sink::Sink;
use futures_util::stream::Stream;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::connection::{ConnectionParams, handle_connection_inner};

/// Maximum size of a single WebSocket message (1 MB)
///
/// This prevents memory exhaustion from malicious clients sending huge messages.
/// The largest legitimate client messages are ~700KB (news/server image uploads).
/// File data chunks should be 64KB, matching TCP streaming behavior.
const MAX_WS_MESSAGE_SIZE: usize = 1024 * 1024;
use crate::transfers::{TransferParams, handle_transfer_connection_inner};

/// Adapter that makes a WebSocket stream behave like a byte stream
///
/// This allows WebSocket connections to use the same `FrameReader`/`FrameWriter`
/// infrastructure as TCP connections.
pub struct WebSocketAdapter<S> {
    /// The underlying WebSocket stream
    inner: S,
    /// Buffer for incoming data (from WebSocket messages)
    read_buffer: Vec<u8>,
    /// Current position in the read buffer
    read_pos: usize,
    /// Buffer for outgoing data (accumulated until flush)
    write_buffer: Vec<u8>,
    /// Whether the WebSocket has been closed
    closed: bool,
}

impl<S> WebSocketAdapter<S> {
    /// Create a new WebSocket adapter
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            read_buffer: Vec::new(),
            read_pos: 0,
            write_buffer: Vec::new(),
            closed: false,
        }
    }
}

impl<S> AsyncRead for WebSocketAdapter<S>
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // If we have buffered data, return it
        if self.read_pos < self.read_buffer.len() {
            let remaining = &self.read_buffer[self.read_pos..];
            let to_copy = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..to_copy]);
            self.read_pos += to_copy;

            // If we've consumed the entire buffer, reset it
            if self.read_pos >= self.read_buffer.len() {
                self.read_buffer.clear();
                self.read_pos = 0;
            }

            return Poll::Ready(Ok(()));
        }

        // If closed, return EOF
        if self.closed {
            return Poll::Ready(Ok(()));
        }

        // Need to read from the WebSocket
        let inner = Pin::new(&mut self.inner);
        match inner.poll_next(cx) {
            Poll::Ready(Some(Ok(msg))) => {
                match msg {
                    Message::Binary(data) => {
                        // Reject oversized messages to prevent memory exhaustion
                        if data.len() > MAX_WS_MESSAGE_SIZE {
                            return Poll::Ready(Err(io::Error::other(format!(
                                "WebSocket message too large: {} bytes (max {})",
                                data.len(),
                                MAX_WS_MESSAGE_SIZE
                            ))));
                        }

                        // Store the binary data in our buffer
                        self.read_buffer = data.to_vec();
                        self.read_pos = 0;

                        // Return as much as we can
                        let to_copy = self.read_buffer.len().min(buf.remaining());
                        buf.put_slice(&self.read_buffer[..to_copy]);
                        self.read_pos = to_copy;

                        // If we've consumed the entire buffer, reset it
                        if self.read_pos >= self.read_buffer.len() {
                            self.read_buffer.clear();
                            self.read_pos = 0;
                        }

                        Poll::Ready(Ok(()))
                    }
                    Message::Close(_) => {
                        self.closed = true;
                        Poll::Ready(Ok(()))
                    }
                    Message::Ping(_) | Message::Pong(_) | Message::Text(_) | Message::Frame(_) => {
                        // Ignore non-binary messages, try again
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
            }
            Poll::Ready(Some(Err(e))) => {
                Poll::Ready(Err(io::Error::other(format!("WebSocket error: {}", e))))
            }
            Poll::Ready(None) => {
                // Stream ended
                self.closed = true;
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S> AsyncWrite for WebSocketAdapter<S>
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // Buffer the data for sending on flush
        self.write_buffer.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // If there's nothing to send, we're done
        if self.write_buffer.is_empty() {
            // Still need to flush the underlying sink
            let inner = Pin::new(&mut self.inner);
            return match inner.poll_flush(cx) {
                Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
                Poll::Ready(Err(e)) => Poll::Ready(Err(io::Error::other(format!(
                    "WebSocket flush error: {}",
                    e
                )))),
                Poll::Pending => Poll::Pending,
            };
        }

        // First, make sure the sink is ready to receive
        {
            let inner = Pin::new(&mut self.inner);
            match inner.poll_ready(cx) {
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Err(io::Error::other(format!(
                        "WebSocket ready error: {}",
                        e
                    ))));
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        // Send the buffered data as a binary message
        let data = std::mem::take(&mut self.write_buffer);
        let msg = Message::Binary(data.into());

        {
            let inner = Pin::new(&mut self.inner);
            if let Err(e) = inner.start_send(msg) {
                return Poll::Ready(Err(io::Error::other(format!(
                    "WebSocket send error: {}",
                    e
                ))));
            }
        }

        // Flush the sink
        let inner = Pin::new(&mut self.inner);
        match inner.poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(io::Error::other(format!(
                "WebSocket flush error: {}",
                e
            )))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Send a close message
        {
            let inner = Pin::new(&mut self.inner);
            match inner.poll_ready(cx) {
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Err(io::Error::other(format!(
                        "WebSocket ready error: {}",
                        e
                    ))));
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        {
            let inner = Pin::new(&mut self.inner);
            if let Err(e) = inner.start_send(Message::Close(None)) {
                return Poll::Ready(Err(io::Error::other(format!(
                    "WebSocket close error: {}",
                    e
                ))));
            }
        }

        // Close the sink
        let inner = Pin::new(&mut self.inner);
        match inner.poll_close(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(io::Error::other(format!(
                "WebSocket close error: {}",
                e
            )))),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Handle a WebSocket BBS connection
///
/// Performs TLS handshake, then WebSocket handshake, then wraps in adapter
/// and delegates to the standard connection handler.
pub async fn handle_websocket_connection(
    socket: TcpStream,
    tls_acceptor: TlsAcceptor,
    params: ConnectionParams,
) -> io::Result<()> {
    // Perform TLS handshake (mandatory, same as TCP)
    let tls_stream = tls_acceptor
        .accept(socket)
        .await
        .map_err(|e| io::Error::other(format!("TLS handshake failed: {}", e)))?;

    // Perform WebSocket handshake over TLS
    let ws_stream = tokio_tungstenite::accept_async(tls_stream)
        .await
        .map_err(|e| io::Error::other(format!("WebSocket handshake failed: {}", e)))?;

    // Wrap in adapter and delegate to standard handler
    let adapter = WebSocketAdapter::new(ws_stream);
    handle_connection_inner(adapter, params).await
}

/// Handle a WebSocket transfer connection
///
/// Performs TLS handshake, then WebSocket handshake, then wraps in adapter
/// and delegates to the standard transfer handler.
pub async fn handle_websocket_transfer_connection(
    socket: TcpStream,
    tls_acceptor: TlsAcceptor,
    params: TransferParams,
) -> io::Result<()> {
    // Perform TLS handshake (mandatory, same as TCP)
    let tls_stream = tls_acceptor
        .accept(socket)
        .await
        .map_err(|e| io::Error::other(format!("TLS handshake failed: {}", e)))?;

    // Perform WebSocket handshake over TLS
    let ws_stream = tokio_tungstenite::accept_async(tls_stream)
        .await
        .map_err(|e| io::Error::other(format!("WebSocket handshake failed: {}", e)))?;

    // Wrap in adapter and delegate to standard transfer handler
    let adapter = WebSocketAdapter::new(ws_stream);
    handle_transfer_connection_inner(adapter, params).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Mock WebSocket stream for testing
    struct MockWebSocket {
        incoming: VecDeque<Result<Message, tokio_tungstenite::tungstenite::Error>>,
        outgoing: Vec<Message>,
        closed: bool,
    }

    impl MockWebSocket {
        fn new(messages: Vec<Message>) -> Self {
            Self {
                incoming: messages.into_iter().map(Ok).collect(),
                outgoing: Vec::new(),
                closed: false,
            }
        }
    }

    impl Stream for MockWebSocket {
        type Item = Result<Message, tokio_tungstenite::tungstenite::Error>;

        fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Poll::Ready(self.incoming.pop_front())
        }
    }

    impl Sink<Message> for MockWebSocket {
        type Error = tokio_tungstenite::tungstenite::Error;

        fn poll_ready(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            self.outgoing.push(item);
            Ok(())
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            self.closed = true;
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn test_read_single_message() {
        let mock = MockWebSocket::new(vec![Message::Binary(b"hello".to_vec().into())]);
        let mut adapter = WebSocketAdapter::new(mock);

        let mut buf = [0u8; 10];
        let n = adapter.read(&mut buf).await.unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf[..n], b"hello");
    }

    #[tokio::test]
    async fn test_read_multiple_messages() {
        let mock = MockWebSocket::new(vec![
            Message::Binary(b"hello".to_vec().into()),
            Message::Binary(b"world".to_vec().into()),
        ]);
        let mut adapter = WebSocketAdapter::new(mock);

        let mut buf = [0u8; 10];

        let n = adapter.read(&mut buf).await.unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf[..n], b"hello");

        let n = adapter.read(&mut buf).await.unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf[..n], b"world");
    }

    #[tokio::test]
    async fn test_read_partial_buffer() {
        let mock = MockWebSocket::new(vec![Message::Binary(b"hello world".to_vec().into())]);
        let mut adapter = WebSocketAdapter::new(mock);

        let mut buf = [0u8; 5];

        let n = adapter.read(&mut buf).await.unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf[..n], b"hello");

        let n = adapter.read(&mut buf).await.unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf[..n], b" worl");

        let n = adapter.read(&mut buf).await.unwrap();
        assert_eq!(n, 1);
        assert_eq!(&buf[..n], b"d");
    }

    #[tokio::test]
    async fn test_read_eof_on_close() {
        let mock = MockWebSocket::new(vec![
            Message::Binary(b"hello".to_vec().into()),
            Message::Close(None),
        ]);
        let mut adapter = WebSocketAdapter::new(mock);

        let mut buf = [0u8; 10];

        let n = adapter.read(&mut buf).await.unwrap();
        assert_eq!(n, 5);

        // Should get EOF after close
        let n = adapter.read(&mut buf).await.unwrap();
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn test_write_and_flush() {
        let mock = MockWebSocket::new(vec![]);
        let mut adapter = WebSocketAdapter::new(mock);

        adapter.write_all(b"hello").await.unwrap();
        adapter.flush().await.unwrap();

        assert_eq!(adapter.inner.outgoing.len(), 1);
        assert!(
            matches!(&adapter.inner.outgoing[0], Message::Binary(data) if data.as_ref() == b"hello")
        );
    }

    #[tokio::test]
    async fn test_write_accumulates_before_flush() {
        let mock = MockWebSocket::new(vec![]);
        let mut adapter = WebSocketAdapter::new(mock);

        adapter.write_all(b"hello").await.unwrap();
        adapter.write_all(b" world").await.unwrap();
        adapter.flush().await.unwrap();

        // Should be a single message with both writes
        assert_eq!(adapter.inner.outgoing.len(), 1);
        assert!(
            matches!(&adapter.inner.outgoing[0], Message::Binary(data) if data.as_ref() == b"hello world")
        );
    }

    #[tokio::test]
    async fn test_shutdown_sends_close() {
        let mock = MockWebSocket::new(vec![]);
        let mut adapter = WebSocketAdapter::new(mock);

        adapter.shutdown().await.unwrap();

        assert!(adapter.inner.closed);
        assert!(
            adapter
                .inner
                .outgoing
                .iter()
                .any(|m| matches!(m, Message::Close(_)))
        );
    }

    #[tokio::test]
    async fn test_oversized_message_rejected() {
        // Create a message larger than MAX_WS_MESSAGE_SIZE (1MB)
        let oversized_data = vec![0u8; 2 * 1024 * 1024]; // 2MB
        let mock = MockWebSocket::new(vec![Message::Binary(oversized_data.into())]);
        let mut adapter = WebSocketAdapter::new(mock);

        let mut buf = [0u8; 10];
        let result = adapter.read(&mut buf).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("too large"));
    }

    #[tokio::test]
    async fn test_empty_binary_message() {
        let mock = MockWebSocket::new(vec![Message::Binary(vec![].into())]);
        let mut adapter = WebSocketAdapter::new(mock);

        let mut buf = [0u8; 10];
        let n = adapter.read(&mut buf).await.unwrap();

        // Empty message should return 0 bytes read (but not EOF)
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn test_text_message_ignored() {
        // Text messages should be ignored, adapter should wait for binary
        let mock = MockWebSocket::new(vec![
            Message::Text("ignored".to_string().into()),
            Message::Binary(b"hello".to_vec().into()),
        ]);
        let mut adapter = WebSocketAdapter::new(mock);

        let mut buf = [0u8; 10];
        let n = adapter.read(&mut buf).await.unwrap();

        // Should skip text and read binary
        assert_eq!(n, 5);
        assert_eq!(&buf[..n], b"hello");
    }

    #[tokio::test]
    async fn test_stream_end_returns_eof() {
        // Empty stream (no messages)
        let mock = MockWebSocket::new(vec![]);
        let mut adapter = WebSocketAdapter::new(mock);

        let mut buf = [0u8; 10];
        let n = adapter.read(&mut buf).await.unwrap();

        // Should return 0 (EOF)
        assert_eq!(n, 0);
    }
}
