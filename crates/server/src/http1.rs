//! A minimal HTTP/1.1 client, for talking to a session on loopback and nothing else.
//!
//! Hand-rolled for the reason [`crate::zip`] is: the whole requirement is one JSON POST to
//! `127.0.0.1` and one streamed response, and the alternative (`reqwest`) drags a TLS
//! stack, a connection pool, and a large slice of the async ecosystem into a binary that
//! currently has no HTTP client at all. `chromiumoxide` pulls `reqwest` in transitively,
//! but only behind the optional `headless-js` feature, so depending on it here would make
//! a core workflow contingent on an optional browser driver.
//!
//! Deliberately not a general client. It speaks to loopback, it does not do TLS,
//! redirects, keep-alive, compression, or proxies, and it should never grow them: the day
//! this needs any of that is the day the requirement has changed enough to warrant a real
//! dependency.

use std::io;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// A response whose body is still arriving.
pub(crate) struct Response {
    pub(crate) status: u16,
    reader: BufReader<TcpStream>,
    /// How the body is framed, decided from the headers.
    framing: Framing,
}

/// How to know where the body ends. HTTP/1.1 offers three answers and a streamed axum
/// response uses the first, but a small error response uses the second, so both are here.
enum Framing {
    /// At a chunk header: the next thing on the wire is `<hex>\r\n`.
    Chunked,
    /// Inside a chunk, with this many bytes left before its trailing CRLF.
    ChunkBody(u64),
    Length(u64),
    /// No length and no chunking: the body ends when the connection closes.
    UntilClose,
}

/// `POST <path>` with a JSON body to `127.0.0.1:<port>`.
///
/// `connect_timeout` bounds only the TCP connect and the response *head*; the body is
/// then streamed for as long as the server keeps writing, because a run legitimately takes
/// minutes and a client that timed out mid-cell would report a hang on healthy work.
pub(crate) async fn post_json(
    port: u16,
    path: &str,
    body: &str,
    connect_timeout: std::time::Duration,
) -> io::Result<Response> {
    let connect = TcpStream::connect(("127.0.0.1", port));
    let stream = tokio::time::timeout(connect_timeout, connect)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "connect timed out"))??;
    let mut stream = stream;
    // `Connection: close` keeps the framing simple and costs nothing: this client makes
    // exactly one request per process.
    let req = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).await?;
    stream.flush().await?;

    let mut reader = BufReader::new(stream);
    let mut head = String::new();
    let status = {
        reader.read_line(&mut head).await?;
        parse_status(&head).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("not an HTTP response: {}", head.trim()),
            )
        })?
    };
    let mut framing = Framing::UntilClose;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            break; // headers ended with the connection
        }
        let line = line.trim_end();
        if line.is_empty() {
            break; // end of headers
        }
        if let Some(f) = framing_from_header(line) {
            framing = f;
        }
    }
    Ok(Response {
        status,
        reader,
        framing,
    })
}

/// `HTTP/1.1 200 OK` -> `200`.
fn parse_status(line: &str) -> Option<u16> {
    line.split_whitespace().nth(1)?.parse().ok()
}

/// The framing a single header line implies, if any. Case-insensitive on the name, since
/// header names are.
fn framing_from_header(line: &str) -> Option<Framing> {
    let (name, value) = line.split_once(':')?;
    let value = value.trim();
    match name.trim().to_ascii_lowercase().as_str() {
        // A `Transfer-Encoding` list ends with the encoding actually applied.
        "transfer-encoding" if value.to_ascii_lowercase().contains("chunked") => {
            Some(Framing::Chunked)
        }
        "content-length" => value.parse().ok().map(Framing::Length),
        _ => None,
    }
}

impl Response {
    /// The next line of the body, without its trailing newline, or `None` at the end.
    ///
    /// Line-oriented because the only body this client reads is NDJSON. Chunk boundaries
    /// are invisible here: a chunk may split a line in half, so the chunk layer is decoded
    /// underneath and lines are assembled from the decoded bytes.
    pub(crate) async fn next_line(&mut self) -> io::Result<Option<String>> {
        let mut line = Vec::new();
        loop {
            match self.read_byte().await? {
                None => {
                    return Ok(
                        (!line.is_empty()).then(|| String::from_utf8_lossy(&line).into_owned())
                    );
                }
                Some(b'\n') => {
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    return Ok(Some(String::from_utf8_lossy(&line).into_owned()));
                }
                Some(b) => line.push(b),
            }
        }
    }

    /// The whole remaining body as a string. For error responses, which are small.
    pub(crate) async fn text(&mut self) -> io::Result<String> {
        let mut out = String::new();
        while let Some(line) = self.next_line().await? {
            out.push_str(&line);
            out.push('\n');
        }
        Ok(out.trim_end().to_string())
    }

    /// One decoded body byte, or `None` at the end of the body.
    async fn read_byte(&mut self) -> io::Result<Option<u8>> {
        match &mut self.framing {
            Framing::Length(0) => Ok(None),
            Framing::Length(n) => {
                let mut b = [0u8; 1];
                match self.reader.read_exact(&mut b).await {
                    Ok(_) => {
                        *n -= 1;
                        Ok(Some(b[0]))
                    }
                    // A short body is the server's problem, not a parse error: report the
                    // end rather than an error the caller cannot act on.
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
                    Err(e) => Err(e),
                }
            }
            Framing::UntilClose => {
                let mut b = [0u8; 1];
                match self.reader.read_exact(&mut b).await {
                    Ok(_) => Ok(Some(b[0])),
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
                    Err(e) => Err(e),
                }
            }
            // Both chunk states are driven from one loop rather than by recursing, so a
            // run of empty chunks (legal, and a plausible keep-alive artefact) cannot grow
            // the stack.
            Framing::Chunked | Framing::ChunkBody(_) => loop {
                match self.framing {
                    Framing::ChunkBody(0) => {
                        // End of a chunk: consume its trailing CRLF, then read the next
                        // chunk header.
                        let mut crlf = [0u8; 2];
                        self.reader.read_exact(&mut crlf).await?;
                        self.framing = Framing::Chunked;
                    }
                    Framing::ChunkBody(ref mut n) => {
                        let mut b = [0u8; 1];
                        self.reader.read_exact(&mut b).await?;
                        *n -= 1;
                        return Ok(Some(b[0]));
                    }
                    _ => {
                        let size = self.read_chunk_size().await?;
                        if size == 0 {
                            // The terminating chunk. Nothing here reads trailers, and
                            // `Connection: close` means the socket ends next anyway.
                            return Ok(None);
                        }
                        self.framing = Framing::ChunkBody(size);
                    }
                }
            },
        }
    }

    /// The size of the next chunk, from its `<hex>[;ext]\r\n` header line.
    async fn read_chunk_size(&mut self) -> io::Result<u64> {
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        let hex = line.trim();
        // Chunk extensions (`1a;foo=bar`) are legal and ignored.
        let hex = hex.split(';').next().unwrap_or("").trim();
        if hex.is_empty() {
            return Ok(0);
        }
        u64::from_str_radix(hex, 16)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, format!("bad chunk: {hex}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_status_line() {
        assert_eq!(parse_status("HTTP/1.1 200 OK\r\n"), Some(200));
        assert_eq!(parse_status("HTTP/1.1 403 Forbidden\r\n"), Some(403));
        assert_eq!(
            parse_status("HTTP/1.0 500 Internal Server Error"),
            Some(500)
        );
        assert_eq!(parse_status("garbage"), None, "must not invent a status");
        assert_eq!(parse_status(""), None);
    }

    #[test]
    fn framing_is_read_case_insensitively() {
        // Header names are case-insensitive on the wire, and hyper does not promise a
        // casing. Reading `Transfer-Encoding` but not `transfer-encoding` would decode a
        // chunked body as raw bytes and emit the chunk-size lines as if they were output.
        assert!(matches!(
            framing_from_header("Transfer-Encoding: chunked"),
            Some(Framing::Chunked)
        ));
        assert!(matches!(
            framing_from_header("transfer-encoding: CHUNKED"),
            Some(Framing::Chunked)
        ));
        assert!(matches!(
            framing_from_header("Content-Length: 42"),
            Some(Framing::Length(42))
        ));
        assert!(framing_from_header("Content-Type: application/json").is_none());
        assert!(framing_from_header("no-colon-here").is_none());
    }
}
