//! std server endpoint example.

use std::time::{Duration, Instant};

use msrt::endpoint::{EndpointPoll, ServerEndpoint};

const TX_BUF_BYTES: usize = 256;
const MAX_PEERS: usize = 8;

fn main() -> msrt::error::Result<()> {
    let start = Instant::now();
    let mut endpoint = ServerEndpoint::<u32, MAX_PEERS>::default();
    let mut tx_buf = [0; TX_BUF_BYTES];

    loop {
        let now_ms = elapsed_ms(start);

        for (peer_id, packet) in read_transport() {
            if endpoint.peer_mut(peer_id).is_none() {
                let _ = endpoint.accept(peer_id, now_ms);
            }
            let _report = endpoint.receive(peer_id, now_ms, &packet);
        }

        let peer_ids: Vec<u32> = endpoint.peers().map(|peer| *peer.peer_id()).collect();
        for peer_id in peer_ids {
            while let Some(Ok(EndpointPoll::Transmit { bytes, .. })) =
                endpoint.poll(peer_id, now_ms, &mut tx_buf)
            {
                write_transport(peer_id, bytes);
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn read_transport() -> impl Iterator<Item = (u32, Vec<u8>)> {
    core::iter::empty()
}

fn write_transport(_peer_id: u32, _bytes: &[u8]) {
    // Replace with a socket, serial port, or other std transport write.
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}
