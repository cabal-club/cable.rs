use std::{
    cmp,
    io::{Read, Write},
};

use futures_util::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{constants::PUBLIC_KEY_BYTES_LEN, Handshake, HandshakeComplete, Result};

impl Handshake<HandshakeComplete> {
    /// Read an encrypted message from the receive buffer, decrypt and write it
    /// to the message buffer - returning the byte size of the written payload.
    fn read_noise_message(&mut self, recv_buf: &[u8], msg: &mut [u8]) -> Result<usize> {
        let len = self.state.0.read_message(recv_buf, msg)?;

        Ok(len)
    }

    /// Read an encrypted message of the given length from the receive buffer,
    /// decrypt and return it as a byte vector.
    ///
    /// This method handles defragmentation of large (> 65519 byte) messages
    /// automatically.
    pub fn read_message(&mut self, recv_buf: &[u8], msg_len: u32) -> Result<Vec<u8>> {
        // Initialise the byte indexes.
        let mut bytes_read = 0;
        let mut bytes_remaining = msg_len;

        // Buffer to which the decrypted Noise transport message bytes for each
        // segment will be written.
        let mut segment_buf = vec![0u8; 65535];

        // Vector to which all decrypted message segments will be appended.
        let mut msg = Vec::new();

        while bytes_remaining > 0 {
            // Determine the byte length of the segment to be read.
            let segment_len = cmp::min(65535, bytes_remaining);

            let len = self.read_noise_message(
                &recv_buf[bytes_read..bytes_read + segment_len as usize],
                &mut segment_buf,
            )?;

            // Append the decrypted message segment to the complete message.
            msg.extend_from_slice(&segment_buf[..len]);

            // Update the byte indexes.
            bytes_remaining -= segment_len;
            bytes_read += segment_len as usize;
        }

        Ok(msg)
    }

    /// Read an encrypted message from the given stream and return it as a
    /// byte vector.
    ///
    /// This method essentially reads two messages from the stream: the
    /// unencrypted message length specifier (4 bytes) followed by the
    /// encrypted message payload.
    ///
    /// An `Ok` return value containing a `Vec` of zero length and capacity
    /// indicates receipt of an end-of-stream marker.
    pub fn read_message_from_stream<T: Read + Write>(&mut self, stream: &mut T) -> Result<Vec<u8>> {
        // Read four bytes describing the length of the incoming message.
        let mut len_buf = [0; 4];
        stream.read_exact(&mut len_buf)?;
        let msg_len = u32::from_le_bytes(len_buf);

        if msg_len == 0 {
            // Return a 0 capacity vector to indicate end-of-stream.
            //
            // This does not result in an allocation.
            return Ok(Vec::with_capacity(0));
        }

        // Read the encrypted bytes of the incoming message.
        let mut recv_buf = vec![0u8; msg_len as usize];
        stream.read_exact(&mut recv_buf[..])?;

        // Decrypt and return the entire message.
        let msg = self.read_message(&recv_buf, msg_len)?;

        Ok(msg)
    }

    /// Read an encrypted message from the given asynchronous stream and
    /// return it as a byte vector.
    ///
    /// This method essentially reads two messages from the stream: the
    /// unencrypted message length specifier (4 bytes) followed by the
    /// encrypted message payload.
    ///
    /// An `Ok` return value containing a `Vec` of zero length and capacity
    /// indicates receipt of an end-of-stream marker.
    pub async fn read_message_from_async_stream<T: AsyncRead + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut T,
    ) -> Result<Vec<u8>> {
        // Read four bytes describing the length of the incoming message.
        let mut len_buf = [0; 4];
        stream.read_exact(&mut len_buf).await?;
        let msg_len = u32::from_le_bytes(len_buf);

        if msg_len == 0 {
            // Return a 0 capacity vector to indicate end-of-stream.
            //
            // This does not result in an allocation.
            return Ok(Vec::with_capacity(0));
        }

        // Read the encrypted bytes of the incoming message.
        let mut recv_buf = vec![0u8; msg_len as usize];
        stream.read_exact(&mut recv_buf[..]).await?;

        // Decrypt and return the entire message.
        let msg = self.read_message(&recv_buf, msg_len)?;

        Ok(msg)
    }

    /// Encrypt and write a message to the send buffer, returning the byte size
    /// of the written payload.
    fn write_noise_message(&mut self, msg: &[u8], send_buf: &mut [u8]) -> Result<usize> {
        let len = self.state.0.write_message(msg, send_buf)?;

        Ok(len)
    }

    /// Encrypt and write a message to the send buffer, returning the total
    /// number of written bytes.
    ///
    /// This method handles message fragmentation in the case that the message
    /// byte length is greater that 65519 bytes.
    pub fn write_message(&mut self, msg: &[u8]) -> Result<(usize, Vec<u8>)> {
        // The length of the authentication tag included with each encrypted
        // Noise transport message.
        const AUTH_TAG_LEN: usize = 16;

        // Initialise the byte indexes.
        let mut bytes_written = 0;
        let mut encrypted_bytes_written = 0;

        // Buffer to which the encrypted Noise transport message bytes for each
        // segment will be written.
        let mut segment_buf = [0u8; 65535];

        // Vector to which all encrypted message segments will be appended.
        let mut encrypted_msg = Vec::new();

        let msg_len = msg.len();

        // Encrypt and append one or more message segments until the entire
        // message has been written.
        while bytes_written < msg_len {
            // Determine the byte length of the segment to be written.
            let segment_len = cmp::min(65535 - AUTH_TAG_LEN, msg_len - bytes_written);
            // Index the bytes of the segment to be written.
            let bytes = &msg[bytes_written..bytes_written + segment_len];

            // Write the message segment.
            let len = self.write_noise_message(bytes, &mut segment_buf)?;

            // Append the encrypted message segment to the complete message.
            encrypted_msg.extend_from_slice(&segment_buf[..len]);

            // Update the byte indexes.
            bytes_written += segment_len;
            encrypted_bytes_written += len;
        }

        Ok((encrypted_bytes_written, encrypted_msg))
    }

    /// Encrypt and write a message to the stream, returning the byte size
    /// of the written payload.
    ///
    /// This method essentially writes at least two messages to the stream:
    /// the unencrypted message length specifier (4 bytes) followed by the
    /// encrypted message payload. The encrypted message payload may be split
    /// into several fragments depending on total payload size.
    pub fn write_message_to_stream<T: Read + Write>(
        &mut self,
        stream: &mut T,
        msg: &[u8],
    ) -> Result<usize> {
        let (bytes_written, encrypted_msg) = self.write_message(msg)?;
        let encrypted_msg_len = &bytes_written.to_le_bytes()[..4];

        stream.write_all(encrypted_msg_len)?;
        stream.write_all(&encrypted_msg)?;

        Ok(bytes_written)
    }

    /// Write an end-of-stream marker (zero-length message) to the stream.
    pub fn write_eos_marker_to_stream<T: Read + Write>(&mut self, stream: &mut T) -> Result<()> {
        // End-of-stream marker (message with length of 0).
        stream.write_all(&[0, 0, 0, 0])?;

        Ok(())
    }

    /// Encrypt and write a message to the asynchronous stream, returning the
    /// byte size of the written payload.
    ///
    /// This method essentially writes at least two messages to the stream:
    /// the unencrypted message length specifier (4 bytes) followed by the
    /// encrypted message payload. The encrypted message payload may be split
    /// into several fragments depending on total payload size.
    pub async fn write_message_to_async_stream<T: AsyncRead + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut T,
        msg: &[u8],
    ) -> Result<usize> {
        let (bytes_written, encrypted_msg) = self.write_message(msg)?;
        let encrypted_msg_len = &bytes_written.to_le_bytes()[..4];

        stream.write_all(encrypted_msg_len).await?;
        stream.write_all(&encrypted_msg).await?;

        Ok(bytes_written)
    }

    /// Write an end-of-stream marker (zero-length message) to the asynchronous
    /// stream.
    pub async fn write_eos_marker_to_async_stream<T: AsyncRead + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut T,
    ) -> Result<()> {
        // End-of-stream marker (message with length of 0).
        stream.write_all(&[0, 0, 0, 0]).await?;

        Ok(())
    }

    /// Return the public key of the remote peer.
    pub fn get_remote_public_key(&self) -> Option<[u8; PUBLIC_KEY_BYTES_LEN]> {
        self.base.remote_public_key
    }
}
