//! Deterministic long-run reliable transport simulation through the public endpoint API.
//!
//! A seeded PRNG injects packet drops, single-byte corruption, and reordering
//! on both link directions while simulated time advances, so retransmission,
//! duplicate handling, integrity rejection, and wire resynchronization are all
//! exercised end to end. The seed is fixed, so every run is reproducible.

use msrt::endpoint::{ClientEndpoint, EndpointPoll, EngineConfig, PassiveEndpoint, ReceiveReport};

const TX_BUF_BYTES: usize = 128;
const DROP_ONE_IN: u64 = 9;
const CORRUPT_ONE_IN: u64 = 8;
const HOLD_ONE_IN: u64 = 7;

#[test]
fn reliable_transport_survives_drops_corruption_reordering_and_duplex_load() {
    let mut mac = ClientEndpoint::new(sim_config(8));
    let mut mcu = PassiveEndpoint::new(sim_config(9));
    let mac_messages = [
        ExpectedMessage::new(b"mac nav message one"),
        ExpectedMessage::new(b"mac telemetry message two"),
        ExpectedMessage::new(b"mac command message three"),
    ];
    let mcu_messages = [
        ExpectedMessage::new(b"mcu telemetry response one"),
        ExpectedMessage::new(b"mcu nav response two"),
        ExpectedMessage::new(b"mcu status response three"),
    ];
    let mut link = SimLink::new(0x4d53_5254_5f76_3131);
    let mut clock = Clock::new();
    let mut mac_delivered = DeliveredMessages::new();
    let mut mcu_delivered = DeliveredMessages::new();

    mac.connect(clock.now()).expect("client connect");
    for message in mac_messages {
        mac.send(message.bytes).expect("queue mac message");
    }

    for _ in 0..256 {
        pump_until_idle(
            &mut mac,
            &mut mcu,
            &mut link,
            &mut clock,
            &mut mac_delivered,
            &mut mcu_delivered,
        );

        if mcu.peer().is_connected() {
            for message in mcu_messages {
                if !mcu_delivered.contains(message.bytes) {
                    mcu.send(message.bytes).expect("queue mcu message");
                }
            }
        }

        link.flush_reordered(&mut mac, &mut mcu, &clock);
        clock.tick();
        pump_until_idle(
            &mut mac,
            &mut mcu,
            &mut link,
            &mut clock,
            &mut mac_delivered,
            &mut mcu_delivered,
        );

        if mac_delivered.contains_all(&mcu_messages) && mcu_delivered.contains_all(&mac_messages) {
            link.assert_noise_was_injected();
            return;
        }
    }

    panic!(
        "simulation did not deliver all messages: mac={:?}, mcu={:?}, noise={:?}",
        mac_delivered,
        mcu_delivered,
        link.stats()
    );
}

fn sim_config(fragment_bytes: usize) -> EngineConfig {
    EngineConfig {
        fragment_bytes,
        retransmit_timeout_ms: 1,
        // The link is deliberately lossy; allow more retries than the default
        // so the test asserts delivery rather than failure reporting.
        max_retransmit_attempts: 30,
        ..EngineConfig::default()
    }
}

fn pump_until_idle(
    mac: &mut ClientEndpoint,
    mcu: &mut PassiveEndpoint,
    link: &mut SimLink,
    clock: &mut Clock,
    mac_delivered: &mut DeliveredMessages,
    mcu_delivered: &mut DeliveredMessages,
) {
    for _ in 0..256 {
        let mut progressed = false;
        clock.tick();

        progressed |= pump_mac(mac, mcu, &mut link.mac_to_mcu, clock, mac_delivered);
        progressed |= pump_mcu(mcu, mac, &mut link.mcu_to_mac, clock, mcu_delivered);

        if !progressed {
            break;
        }
    }
}

fn pump_mac(
    src: &mut ClientEndpoint,
    dst: &mut PassiveEndpoint,
    direction: &mut SimDirection,
    clock: &Clock,
    delivered: &mut DeliveredMessages,
) -> bool {
    let mut tx_buf = [0; TX_BUF_BYTES];

    match src.poll(clock.now(), &mut tx_buf).expect("poll client") {
        EndpointPoll::Transmit { bytes, .. } => {
            direction.deliver_to_passive(dst, clock, SimWrite::from_bytes(bytes));
            true
        }
        EndpointPoll::Message(message) => {
            delivered.push(message.as_bytes());
            true
        }
        EndpointPoll::SendFailed(failed) => {
            panic!("reliable simulation should not fail sends: {failed:?}");
        }
        EndpointPoll::Idle => false,
    }
}

fn pump_mcu(
    src: &mut PassiveEndpoint,
    dst: &mut ClientEndpoint,
    direction: &mut SimDirection,
    clock: &Clock,
    delivered: &mut DeliveredMessages,
) -> bool {
    let mut tx_buf = [0; TX_BUF_BYTES];

    match src.poll(clock.now(), &mut tx_buf).expect("poll passive") {
        EndpointPoll::Transmit { bytes, .. } => {
            direction.deliver_to_client(dst, clock, SimWrite::from_bytes(bytes));
            true
        }
        EndpointPoll::Message(message) => {
            delivered.push(message.as_bytes());
            true
        }
        EndpointPoll::SendFailed(failed) => {
            panic!("reliable simulation should not fail sends: {failed:?}");
        }
        EndpointPoll::Idle => false,
    }
}

/// Simulated milliseconds driving retransmission and reassembly timers.
#[derive(Clone, Copy, Debug)]
struct Clock {
    now_ms: u64,
}

impl Clock {
    const fn new() -> Self {
        Self { now_ms: 0 }
    }

    const fn now(&self) -> u64 {
        self.now_ms
    }

    fn tick(&mut self) {
        self.now_ms += 1;
    }
}

/// Deterministic xorshift64* PRNG; the fixed seed keeps every run identical.
#[derive(Clone, Copy, Debug)]
struct Rng {
    state: u64,
}

impl Rng {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn one_in(&mut self, chance: u64) -> bool {
        self.next().is_multiple_of(chance)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NoiseStats {
    dropped: usize,
    corrupted: usize,
    reordered: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedMessage {
    bytes: &'static [u8],
}

impl ExpectedMessage {
    const fn new(bytes: &'static [u8]) -> Self {
        Self { bytes }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeliveredMessages {
    messages: [([u8; 64], usize); 8],
    len: usize,
}

impl DeliveredMessages {
    const fn new() -> Self {
        Self {
            messages: [([0; 64], 0); 8],
            len: 0,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        assert!(
            self.len < self.messages.len(),
            "delivered message buffer full"
        );
        assert!(bytes.len() <= 64, "delivered message too large for test");

        if self.contains(bytes) {
            return;
        }

        let mut stored = [0; 64];
        stored[..bytes.len()].copy_from_slice(bytes);
        self.messages[self.len] = (stored, bytes.len());
        self.len += 1;
    }

    fn contains_all(&self, expected: &[ExpectedMessage]) -> bool {
        expected.iter().all(|message| self.contains(message.bytes))
    }

    fn contains(&self, bytes: &[u8]) -> bool {
        self.messages[..self.len]
            .iter()
            .any(|(current_bytes, current_len)| &current_bytes[..*current_len] == bytes)
    }
}

#[derive(Debug)]
struct SimLink {
    mac_to_mcu: SimDirection,
    mcu_to_mac: SimDirection,
}

impl SimLink {
    const fn new(seed: u64) -> Self {
        Self {
            mac_to_mcu: SimDirection::new(seed),
            mcu_to_mac: SimDirection::new(seed ^ 0x9e37_79b9_7f4a_7c15),
        }
    }

    fn flush_reordered(
        &mut self,
        mac: &mut ClientEndpoint,
        mcu: &mut PassiveEndpoint,
        clock: &Clock,
    ) {
        self.mac_to_mcu.flush_to_passive(mcu, clock);
        self.mcu_to_mac.flush_to_client(mac, clock);
    }

    fn stats(&self) -> (NoiseStats, NoiseStats) {
        (self.mac_to_mcu.stats, self.mcu_to_mac.stats)
    }

    fn assert_noise_was_injected(&self) {
        for stats in [self.mac_to_mcu.stats, self.mcu_to_mac.stats] {
            assert!(stats.dropped > 0, "simulation never dropped a packet");
            assert!(stats.corrupted > 0, "simulation never corrupted a packet");
            assert!(stats.reordered > 0, "simulation never reordered a packet");
        }
    }
}

#[derive(Debug)]
struct SimDirection {
    rng: Rng,
    held: [Option<SimWrite>; 8],
    held_len: usize,
    stats: NoiseStats,
}

/// What the noisy link decides to do with one transmitted envelope.
enum LinkFate {
    Deliver(SimWrite),
    Dropped,
    Held,
}

impl SimDirection {
    const fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            held: [None; 8],
            held_len: 0,
            stats: NoiseStats {
                dropped: 0,
                corrupted: 0,
                reordered: 0,
            },
        }
    }

    fn decide(&mut self, mut write: SimWrite) -> LinkFate {
        if self.rng.one_in(DROP_ONE_IN) {
            self.stats.dropped += 1;
            return LinkFate::Dropped;
        }

        if self.held_len < self.held.len() && self.rng.one_in(HOLD_ONE_IN) {
            self.stats.reordered += 1;
            self.held[self.held_len] = Some(write);
            self.held_len += 1;
            return LinkFate::Held;
        }

        if self.rng.one_in(CORRUPT_ONE_IN) {
            self.stats.corrupted += 1;
            let index = (self.rng.next() as usize) % write.len;
            let mask = (self.rng.next() % 255 + 1) as u8;
            write.bytes[index] ^= mask;
        }

        LinkFate::Deliver(write)
    }

    fn deliver_to_passive(&mut self, dst: &mut PassiveEndpoint, clock: &Clock, write: SimWrite) {
        if let LinkFate::Deliver(write) = self.decide(write) {
            receive_ok(dst.receive(clock.now(), write.as_bytes()));
        }
    }

    fn deliver_to_client(&mut self, dst: &mut ClientEndpoint, clock: &Clock, write: SimWrite) {
        if let LinkFate::Deliver(write) = self.decide(write) {
            receive_ok(dst.receive(clock.now(), write.as_bytes()));
        }
    }

    fn flush_to_passive(&mut self, dst: &mut PassiveEndpoint, clock: &Clock) {
        while self.held_len > 0 {
            self.held_len -= 1;
            let write = self.held[self.held_len].take().expect("held packet");

            receive_ok(dst.receive(clock.now(), write.as_bytes()));
        }
    }

    fn flush_to_client(&mut self, dst: &mut ClientEndpoint, clock: &Clock) {
        while self.held_len > 0 {
            self.held_len -= 1;
            let write = self.held[self.held_len].take().expect("held packet");

            receive_ok(dst.receive(clock.now(), write.as_bytes()));
        }
    }
}

fn receive_ok(report: ReceiveReport) {
    // Corrupted input legitimately produces Noise / Corrupted / Incomplete
    // while the decoder resynchronizes; only a session-level error is fatal.
    if let ReceiveReport::Error(error) = report {
        panic!("unexpected receive error in simulation: {error:?}");
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SimWrite {
    bytes: [u8; TX_BUF_BYTES],
    len: usize,
}

impl SimWrite {
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut stored = [0; TX_BUF_BYTES];
        stored[..bytes.len()].copy_from_slice(bytes);

        Self {
            bytes: stored,
            len: bytes.len(),
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}
