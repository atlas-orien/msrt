//! Internal engine state machinery.

use crate::core::{MessageId, PacketNumber};
use crate::engine::{MAX_IN_FLIGHT_PACKETS, MAX_INGRESS_BYTES};
use crate::reliability::PacketDedup;
use crate::wire::StreamingDecoder;

use self::{
    ack::AckRanges, inflight::InFlightPackets, queue::EventQueue, reassembly::ReassemblyBuffer,
};

pub(crate) mod ack;
pub(crate) mod inflight;
pub(crate) mod ingress;
pub(crate) mod outgoing;
pub(crate) mod packet;
pub(crate) mod queue;
pub(crate) mod reassembly;
pub(crate) mod retransmit;

/// Internal protocol state owned by [`crate::engine::Engine`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Machine {
    pub(crate) next_packet_number: PacketNumber,
    pub(crate) next_message_id: MessageId,
    pub(crate) now_ms: u64,
    pub(crate) events: EventQueue,
    pub(crate) in_flight: InFlightPackets,
    pub(crate) ack_ranges: AckRanges,
    pub(crate) ingress: StreamingDecoder<MAX_INGRESS_BYTES>,
    pub(crate) dedup: PacketDedup<MAX_IN_FLIGHT_PACKETS>,
    pub(crate) reassembly: ReassemblyBuffer,
}

impl Machine {
    pub(crate) const fn new(
        initial_packet_number: PacketNumber,
        initial_message_id: MessageId,
    ) -> Self {
        Self {
            next_packet_number: initial_packet_number,
            next_message_id: initial_message_id,
            now_ms: 0,
            events: EventQueue::new(),
            in_flight: InFlightPackets::new(),
            ack_ranges: AckRanges::new(),
            ingress: StreamingDecoder::new(),
            dedup: PacketDedup::new(),
            reassembly: ReassemblyBuffer::new(),
        }
    }
}
