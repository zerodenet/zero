//! Cross-platform IPC client.
//!
//! On Unix this connects to a Unix domain socket.
//! On Windows this connects to a named pipe.

#[cfg(unix)]
mod imp {
    use std::io::{self, BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    use crate::ipc::protocol::{serialize_frame, IpcRequest, IpcResponse};

    pub fn send_request(socket_path: &str, request: &IpcRequest) -> io::Result<IpcResponse> {
        let stream = UnixStream::connect(socket_path)?;
        send_impl(&stream, request)?;
        read_impl(&stream)
    }

    pub fn stream_events(
        socket_path: &str,
        request: &IpcRequest,
        on_event: impl FnMut(serde_json::Value),
    ) -> io::Result<()> {
        let stream = UnixStream::connect(socket_path)?;
        stream_impl(&stream, request, on_event)
    }

    fn send_impl(mut stream: &UnixStream, request: &IpcRequest) -> io::Result<()> {
        let frame = serialize_frame(request).map_err(io::Error::other)?;
        stream.write_all(&frame)?;
        Ok(())
    }

    fn read_impl(stream: &UnixStream) -> io::Result<IpcResponse> {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            return serde_json::from_str(&line).map_err(io::Error::other);
        }
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "no response from control socket",
        ))
    }

    fn stream_impl(
        stream: &UnixStream,
        request: &IpcRequest,
        mut on_event: impl FnMut(serde_json::Value),
    ) -> io::Result<()> {
        send_impl(stream, request)?;
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let value: serde_json::Value =
                serde_json::from_str(&line).map_err(io::Error::other)?;
            on_event(value);
        }
        Ok(())
    }
}

#[cfg(windows)]
mod imp {
    use std::io::{self, BufRead, BufReader, Write};

    use crate::ipc::protocol::{serialize_frame, IpcRequest, IpcResponse};

    pub fn send_request(pipe_name: &str, request: &IpcRequest) -> io::Result<IpcResponse> {
        let stream = open_pipe(pipe_name)?;
        send_impl(&stream, request)?;
        read_impl(&stream)
    }

    pub fn stream_events(
        pipe_name: &str,
        request: &IpcRequest,
        on_event: impl FnMut(serde_json::Value),
    ) -> io::Result<()> {
        let stream = open_pipe(pipe_name)?;
        stream_impl(&stream, request, on_event)
    }

    fn open_pipe(name: &str) -> io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(name)
    }

    fn send_impl(mut stream: &std::fs::File, request: &IpcRequest) -> io::Result<()> {
        let frame = serialize_frame(request).map_err(io::Error::other)?;
        stream.write_all(&frame)?;
        Ok(())
    }

    fn read_impl(stream: &std::fs::File) -> io::Result<IpcResponse> {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            return serde_json::from_str(&line).map_err(io::Error::other);
        }
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "no response from ipc pipe",
        ))
    }

    fn stream_impl(
        stream: &std::fs::File,
        request: &IpcRequest,
        mut on_event: impl FnMut(serde_json::Value),
    ) -> io::Result<()> {
        send_impl(stream, request)?;
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let value: serde_json::Value =
                serde_json::from_str(&line).map_err(io::Error::other)?;
            on_event(value);
        }
        Ok(())
    }
}

// Re-export the platform-specific implementations.
pub use imp::{send_request, stream_events};
