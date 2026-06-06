#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]
//! no_std MCU-style passive endpoint example.
//!
//! Host builds keep a tiny `main` so `cargo check --examples` still works on a
//! desktop. The actual endpoint driver below uses only `core` and MSRT no_std
//! APIs, so it can be copied into a Cortex-M project with a real interrupt,
//! DMA, UART, or USB CDC adapter.

use msrt::endpoint::{EndpointPoll, MessageEvent, PassiveEndpoint, ReceiveReport};

const TX_BUF_BYTES: usize = 256;
const POLLS_PER_TICK: usize = 16;

/// Small no_std endpoint wrapper owned by the MCU main loop.
pub struct McuMsrt {
    endpoint: PassiveEndpoint,
    tx_buf: [u8; TX_BUF_BYTES],
}

impl McuMsrt {
    /// Creates an idle MCU-side MSRT endpoint.
    #[must_use]
    pub fn new() -> Self {
        Self {
            endpoint: PassiveEndpoint::new(msrt::endpoint::EngineConfig::default()),
            tx_buf: [0; TX_BUF_BYTES],
        }
    }

    /// Feeds one byte received by an interrupt, DMA ring, or polling driver.
    pub fn on_rx_byte(&mut self, now_ms: u64, byte: u8) -> ReceiveReport {
        self.endpoint.receive(now_ms, core::slice::from_ref(&byte))
    }

    /// Runs a bounded amount of endpoint work.
    pub fn poll<W>(&mut self, now_ms: u64, mut write: W) -> msrt::error::Result<()>
    where
        W: FnMut(&[u8]),
    {
        for _ in 0..POLLS_PER_TICK {
            match self.endpoint.poll(now_ms, &mut self.tx_buf)? {
                EndpointPoll::Transmit { bytes, .. } => write(bytes),
                EndpointPoll::Message(message) => self.on_message(message)?,
                EndpointPoll::SendFailed(_) | EndpointPoll::Idle => break,
            }
        }

        Ok(())
    }

    fn on_message(&mut self, _message: MessageEvent) -> msrt::error::Result<()> {
        let _ = self.endpoint.send(b"mcu msrt received host message")?;
        Ok(())
    }
}

impl Default for McuMsrt {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("build this example for a no_std MCU target to use the embedded entry point");
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    let mut msrt = McuMsrt::new();
    let mut now_ms = 0u64;

    loop {
        while let Some(byte) = read_transport_byte() {
            let _ = msrt.on_rx_byte(now_ms, byte);
        }

        let _ = msrt.poll(now_ms, write_transport);
        now_ms = now_ms.wrapping_add(1);
    }
}

#[cfg(target_os = "none")]
fn read_transport_byte() -> Option<u8> {
    // Replace with a non-blocking read from a DMA ring, UART ISR queue, or USB CDC buffer.
    None
}

#[cfg(target_os = "none")]
fn write_transport(_bytes: &[u8]) {
    // Replace with a non-blocking platform transmit function.
}
