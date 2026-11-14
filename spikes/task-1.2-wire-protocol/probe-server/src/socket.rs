//! Where the connections come from: a Unix socket (R5), and a pipe pair for gate 2.

use std::path::Path;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::UnixListener;

use crate::attach::{Server, serve};

/// Listen until the shutdown is asked for.
///
/// # Errors
/// When the socket cannot be bound.
pub async fn listen(server: Arc<Server>, path: &Path) -> std::io::Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    // R5: only the owning user may open it. The mode a bind leaves behind depends on the
    // process's umask, so it is set here rather than assumed - a finding for TASK-1.9, which
    // builds the real socket.
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    let shutdown = Arc::clone(&server.shutdown);
    let accepting = async {
        while let Ok((stream, _)) = listener.accept().await {
            let server = Arc::clone(&server);
            tokio::spawn(async move {
                let (reader, writer) = stream.into_split();
                serve(server, reader, writer).await;
            });
        }
    };
    tokio::select! {
        () = accepting => {}
        () = shutdown.notified() => {}
    }
    let _ = std::fs::remove_file(path);
    Ok(())
}

/// Serve exactly one connection over a reader and a writer that are not a socket. Used to show
/// that the framing survives a byte stream it does not control.
pub async fn one<R, W>(server: Arc<Server>, reader: R, writer: W)
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    serve(server, reader, writer).await;
}
