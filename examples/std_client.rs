//! std client endpoint example.

use std::time::{Duration, Instant};

use msrt::endpoint::{ClientEndpoint, EndpointPoll};

const TX_BUF_BYTES: usize = 256;

fn main() -> msrt::error::Result<()> {
    let start = Instant::now();
    let mut endpoint = ClientEndpoint::default();
    let mut tx_buf = [0; TX_BUF_BYTES];

    endpoint.connect(0)?;

    loop {
        let now_ms = elapsed_ms(start);

        for packet in read_transport() {
            let _report = endpoint.receive(now_ms, &packet);
        }

        if endpoint.peer().is_connected() {
            let _ = endpoint.send(b"hello from std client")?;
        }

        while let EndpointPoll::Transmit { bytes, .. } = endpoint.poll(now_ms, &mut tx_buf)? {
            write_transport(bytes);
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn read_transport() -> impl Iterator<Item = Vec<u8>> {
    core::iter::empty()
}

fn write_transport(_bytes: &[u8]) {
    // Replace with a socket, serial port, or other std transport write.
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}
