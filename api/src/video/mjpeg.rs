//! MJPEG stream parsing: extract JPEG frames from FFmpeg stdout.
//! JPEG frames start with FF D8 and end with FF D9.

use bytes::Bytes;
use std::io::Read;
use std::process::ChildStdout;
use tokio::sync::broadcast;

/// Parse MJPEG stream from FFmpeg stdout into individual JPEG frames.
pub fn extract_jpeg_frames(mut reader: ChildStdout, tx: broadcast::Sender<Bytes>) {
    let mut buf = [0u8; 65536];
    let mut frame = Vec::new();
    let mut in_frame = false;

    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = &buf[..n];
                let mut i = 0;
                while i < chunk.len() {
                    if !in_frame {
                        if i + 1 < chunk.len() && chunk[i] == 0xFF && chunk[i + 1] == 0xD8 {
                            in_frame = true;
                            frame.clear();
                            frame.extend_from_slice(&chunk[i..]);
                            i = chunk.len();
                        } else {
                            i += 1;
                        }
                    } else {
                        frame.extend_from_slice(&chunk[i..]);
                        if let Some(pos) =
                            frame.windows(2).rposition(|w| w[0] == 0xFF && w[1] == 0xD9)
                        {
                            let end = pos + 2;
                            let jpeg = frame.drain(..end).collect::<Vec<_>>();
                            let _ = tx.send(Bytes::from(jpeg));
                            in_frame = false;
                            i = chunk.len();
                        } else {
                            i = chunk.len();
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
}
