//! Integration tests for the public MSRT facade.

use msrt::{
    endpoint::{
        ClientEndpoint, EndpointPoll, EngineConfig, IntegrityConfig, MessageId, PassiveEndpoint,
        PeerState, ReceiveReport,
    },
    error::{Error, ErrorKind},
};

const TX_BUF_BYTES: usize = 128;

#[test]
fn facade_exposes_endpoint_config_api() {
    let mut client = ClientEndpoint::new(EngineConfig {
        initial_message_id: MessageId::new(7),
        integrity: IntegrityConfig::crc32(),
        ..EngineConfig::default()
    });
    let mut tx_buf = [0; TX_BUF_BYTES];
    let message_id = client.connect(0).unwrap();

    assert_ne!(message_id, MessageId::ZERO);

    let EndpointPoll::Transmit { bytes, .. } = client.poll(0, &mut tx_buf).unwrap() else {
        panic!("client should produce hello bytes");
    };

    assert!(!bytes.is_empty());
}

#[test]
fn facade_exposes_endpoint_receive_api() {
    let mut client = ClientEndpoint::default();
    let mut passive = PassiveEndpoint::default();
    let mut tx_buf = [0; TX_BUF_BYTES];

    client.connect(1).unwrap();
    let EndpointPoll::Transmit { bytes, .. } = client.poll(1, &mut tx_buf).unwrap() else {
        panic!("client should transmit hello");
    };

    assert!(matches!(
        passive.receive(2, bytes),
        ReceiveReport::Packet { .. }
    ));
    assert_eq!(passive.peer().state(), PeerState::Connected);
}

#[test]
fn facade_exposes_error_api() {
    let error = Error::new(ErrorKind::Engine);

    assert_eq!(error.kind(), ErrorKind::Engine);
}
