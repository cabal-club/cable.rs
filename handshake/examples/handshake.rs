use cable_handshake::{version_bytes_len, Handshake, Result, Version};

fn main() -> Result<()> {
    let hs_client = Handshake::new_client(Version::init(1, 0));
    let hs_server = Handshake::new_server(Version::init(3, 7));

    let mut buf = [0; 8];

    let (hs_client, hs_server) = {
        let mut client_buf = &mut buf[..version_bytes_len()];
        let hs_client = hs_client.send_client_version(&mut client_buf)?;
        let mut server_buf = &mut buf[..2];
        let hs_server = hs_server.recv_client_version(&mut server_buf)?;
        (hs_client, hs_server)
    };

    let mut server_buf = &mut buf[..2];
    let _hs_server = hs_server.send_server_version(&mut server_buf)?;
    let mut client_buf = &mut buf[..2];
    let _hs_client = hs_client.recv_server_version(&mut client_buf)?;

    Ok(())
}
