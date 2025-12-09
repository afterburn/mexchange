use std::net::{SocketAddr, UdpSocket};
use std::thread;
use std::time::Duration;

use crate::*;

// ============================================================================
// Protocol Tests
// ============================================================================

#[test]
fn test_packet_header_roundtrip() {
    let header = PacketHeader::new(42, 100, 1000, 5);
    let mut buf = [0u8; PACKET_HEADER_LEN];

    let written = header.write_to(&mut buf).unwrap();
    assert_eq!(written, PACKET_HEADER_LEN);

    let parsed = PacketHeader::read_from(&buf).unwrap();
    assert_eq!(header, parsed);
}

#[test]
fn test_packet_header_heartbeat() {
    let header = PacketHeader::heartbeat(42, 100, 1000);
    assert!(header.is_heartbeat());
    assert_eq!(header.msg_count, 0);

    let mut buf = [0u8; PACKET_HEADER_LEN];
    header.write_to(&mut buf).unwrap();
    let parsed = PacketHeader::read_from(&buf).unwrap();
    assert!(parsed.is_heartbeat());
}

#[test]
fn test_message_header_roundtrip() {
    let header = MessageHeader::new(MessageType::OrderNew, MessageFlags::NONE, 256);
    let mut buf = [0u8; MESSAGE_HEADER_LEN];

    let written = header.write_to(&mut buf).unwrap();
    assert_eq!(written, MESSAGE_HEADER_LEN);

    let parsed = MessageHeader::read_from(&buf).unwrap();
    assert_eq!(header, parsed);
    assert_eq!(parsed.message_type(), Some(MessageType::OrderNew));
}

#[test]
fn test_message_types() {
    let types = [
        (0x01, MessageType::OrderNew),
        (0x02, MessageType::OrderCancel),
        (0x03, MessageType::OrderReplace),
        (0x10, MessageType::MatchEvent),
        (0x11, MessageType::BookSnapshot),
        (0x12, MessageType::BookUpdate),
        (0x20, MessageType::PositionUpdate),
        (0x30, MessageType::Heartbeat),
        (0x40, MessageType::Control),
    ];

    for (byte, expected) in types {
        let parsed = MessageType::from_u8(byte);
        assert_eq!(parsed, Some(expected));
    }

    assert_eq!(MessageType::from_u8(0xFF), None);
}

#[test]
fn test_packet_builder_single_message() {
    let mut builder = PacketBuilder::new(1, 0, 0);
    let payload = b"test payload";

    assert!(builder.try_add_message(MessageType::OrderNew, payload));
    assert_eq!(builder.msg_count(), 1);

    let packet_bytes = builder.finish();
    let packet = Packet::parse(&packet_bytes).unwrap();

    assert_eq!(packet.header.stream_id, 1);
    assert_eq!(packet.header.packet_seq, 0);
    assert_eq!(packet.header.first_msg_seq, 0);
    assert_eq!(packet.header.msg_count, 1);
    assert_eq!(packet.messages.len(), 1);
    assert_eq!(packet.messages[0].payload, payload);
    assert_eq!(packet.messages[0].seq, 0);
}

#[test]
fn test_packet_builder_multiple_messages() {
    let mut builder = PacketBuilder::new(1, 5, 100);

    for i in 0..10 {
        let payload = format!("message {}", i);
        assert!(builder.try_add_message(MessageType::BookUpdate, payload.as_bytes()));
    }

    let packet_bytes = builder.finish();
    let packet = Packet::parse(&packet_bytes).unwrap();

    assert_eq!(packet.header.msg_count, 10);
    assert_eq!(packet.messages.len(), 10);

    for (i, msg) in packet.messages.iter().enumerate() {
        assert_eq!(msg.seq, 100 + i as u64);
    }
}

#[test]
fn test_packet_builder_respects_mtu() {
    let mut builder = PacketBuilder::new(1, 0, 0);
    let large_payload = vec![0u8; MAX_PAYLOAD]; // This won't fit with header

    // First small message should fit
    assert!(builder.try_add_message(MessageType::OrderNew, b"small"));

    // Large message shouldn't fit in remaining space
    assert!(!builder.try_add_message(MessageType::OrderNew, &large_payload));
}

#[test]
fn test_packet_builder_heartbeat() {
    let heartbeat = PacketBuilder::heartbeat(42, 10, 500);
    let packet = Packet::parse(&heartbeat).unwrap();

    assert!(packet.header.is_heartbeat());
    assert_eq!(packet.header.stream_id, 42);
    assert_eq!(packet.header.packet_seq, 10);
    assert_eq!(packet.header.first_msg_seq, 500);
    assert_eq!(packet.messages.len(), 0);
}

#[test]
fn test_invalid_version_rejected() {
    let mut buf = [0u8; PACKET_HEADER_LEN];
    buf[0] = 99; // Invalid version
    buf[1] = PACKET_HEADER_LEN as u8;

    let result = PacketHeader::read_from(&buf);
    assert!(matches!(
        result,
        Err(ProtocolError::InvalidVersion { expected: 1, got: 99 })
    ));
}

#[test]
fn test_buffer_too_small() {
    let small_buf = [0u8; 10];
    let result = PacketHeader::read_from(&small_buf);
    assert!(matches!(result, Err(ProtocolError::BufferTooSmall { .. })));
}

// ============================================================================
// Integration Tests - Sender/Receiver
// ============================================================================

fn get_free_port() -> u16 {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.local_addr().unwrap().port()
}

#[test]
fn test_sender_receiver_single_message() {
    let recv_port = get_free_port();
    let send_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();
    let send_addr: SocketAddr = format!("127.0.0.1:{}", send_port).parse().unwrap();

    let recv_config = ReceiverConfig {
        stream_id: 1,
        ..Default::default()
    };

    let send_config = SenderConfig {
        stream_id: 1,
        target_addr: recv_addr,
        enable_heartbeats: false,
        max_batch_delay: Duration::from_millis(1),
        ..Default::default()
    };

    let receiver = UdpReceiver::new(recv_config, recv_addr).unwrap();
    let sender = UdpSender::new(send_config, send_addr).unwrap();

    // Send a message
    let payload = b"hello world";
    sender.send(MessageType::OrderNew, payload.to_vec()).unwrap();

    // Wait for message
    thread::sleep(Duration::from_millis(50));

    let msg = receiver.recv_timeout(Duration::from_millis(100)).unwrap();
    assert!(msg.is_some());
    let msg = msg.unwrap();
    assert_eq!(msg.msg_type, MessageType::OrderNew);
    assert_eq!(msg.payload, payload);
    assert_eq!(msg.stream_id, 1);

    // Check stats
    let send_stats = sender.stats();
    assert_eq!(send_stats.messages_sent, 1);
    assert!(send_stats.packets_sent >= 1);

    let recv_stats = receiver.stats();
    assert_eq!(recv_stats.messages_received, 1);
}

#[test]
fn test_sender_receiver_multiple_messages() {
    let recv_port = get_free_port();
    let send_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();
    let send_addr: SocketAddr = format!("127.0.0.1:{}", send_port).parse().unwrap();

    let recv_config = ReceiverConfig {
        stream_id: 1,
        ..Default::default()
    };

    let send_config = SenderConfig {
        stream_id: 1,
        target_addr: recv_addr,
        enable_heartbeats: false,
        max_batch_delay: Duration::from_millis(1),
        ..Default::default()
    };

    let receiver = UdpReceiver::new(recv_config, recv_addr).unwrap();
    let sender = UdpSender::new(send_config, send_addr).unwrap();

    // Send multiple messages
    let count = 100;
    for i in 0..count {
        let payload = format!("message {}", i);
        sender.send(MessageType::BookUpdate, payload.into_bytes()).unwrap();
    }

    // Wait for messages
    thread::sleep(Duration::from_millis(100));

    let mut received = 0;
    while let Ok(Some(_)) = receiver.recv_timeout(Duration::from_millis(10)) {
        received += 1;
    }

    assert_eq!(received, count);

    let recv_stats = receiver.stats();
    assert_eq!(recv_stats.messages_received, count as u64);
}

#[test]
fn test_batching() {
    let recv_port = get_free_port();
    let send_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();
    let send_addr: SocketAddr = format!("127.0.0.1:{}", send_port).parse().unwrap();

    let recv_config = ReceiverConfig {
        stream_id: 1,
        ..Default::default()
    };

    // Very long batch delay - messages will only be sent when packet is full or on shutdown
    let send_config = SenderConfig {
        stream_id: 1,
        target_addr: recv_addr,
        enable_heartbeats: false,
        max_batch_delay: Duration::from_secs(60),
        ..Default::default()
    };

    let receiver = UdpReceiver::new(recv_config, recv_addr).unwrap();
    let mut sender = UdpSender::new(send_config, send_addr).unwrap();

    // Send 100 small messages (each ~10 bytes payload + 4 byte header = ~14 bytes)
    // MAX_PAYLOAD is 1376 bytes, so we should fit many messages per packet
    for i in 0..100 {
        let payload = format!("msg{:04}", i); // 7 bytes each
        sender.send(MessageType::OrderNew, payload.into_bytes()).unwrap();
    }

    // Shutdown flushes all pending messages
    sender.shutdown();

    // Wait for receiver to process
    thread::sleep(Duration::from_millis(50));

    let recv_stats = receiver.stats();
    let send_stats = sender.stats();

    // 100 messages of ~11 bytes each (7 payload + 4 header) = ~1100 bytes total
    // This should fit in 1-2 packets (MAX_PAYLOAD is 1376 bytes)
    assert!(send_stats.packets_sent <= 2, "Expected <= 2 packets, got {}", send_stats.packets_sent);
    assert_eq!(send_stats.messages_sent, 100);
    assert!(recv_stats.messages_received >= 90, "Expected >= 90 messages received, got {}", recv_stats.messages_received);
}

#[test]
fn test_heartbeats() {
    let recv_port = get_free_port();
    let send_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();
    let send_addr: SocketAddr = format!("127.0.0.1:{}", send_port).parse().unwrap();

    let recv_config = ReceiverConfig {
        stream_id: 1,
        ..Default::default()
    };

    let send_config = SenderConfig {
        stream_id: 1,
        target_addr: recv_addr,
        enable_heartbeats: true,
        max_batch_delay: Duration::from_millis(10),
        ..Default::default()
    };

    let receiver = UdpReceiver::new(recv_config, recv_addr).unwrap();
    let sender = UdpSender::new(send_config, send_addr).unwrap();

    // Wait for heartbeats (HEARTBEAT_INTERVAL_MS is 100ms, so 350ms should give us 3)
    thread::sleep(Duration::from_millis(400));

    let recv_stats = receiver.stats();
    let send_stats = sender.stats();

    // Should have received some heartbeats
    assert!(send_stats.heartbeats_sent >= 1);
    assert!(recv_stats.heartbeats_received >= 1);

    // Receiver should be active
    assert_eq!(receiver.state(), StreamState::Active);
}

#[test]
fn test_stream_state_transitions() {
    let recv_port = get_free_port();
    let send_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();
    let send_addr: SocketAddr = format!("127.0.0.1:{}", send_port).parse().unwrap();

    let recv_config = ReceiverConfig {
        stream_id: 1,
        stream_timeout: Duration::from_millis(100),
        ..Default::default()
    };

    let send_config = SenderConfig {
        stream_id: 1,
        target_addr: recv_addr,
        enable_heartbeats: false,
        max_batch_delay: Duration::from_millis(1),
        ..Default::default()
    };

    let receiver = UdpReceiver::new(recv_config, recv_addr).unwrap();
    let mut sender = UdpSender::new(send_config, send_addr).unwrap();

    // Initially in Initializing state
    assert_eq!(receiver.state(), StreamState::Initializing);

    // Send a message
    sender.send(MessageType::OrderNew, b"test".to_vec()).unwrap();
    thread::sleep(Duration::from_millis(50));

    // Should be Active after receiving
    assert_eq!(receiver.state(), StreamState::Active);

    // Shutdown sender
    sender.shutdown();

    // Wait for timeout
    thread::sleep(Duration::from_millis(200));

    // Should be Down after timeout
    assert_eq!(receiver.state(), StreamState::Down);
}

#[test]
fn test_gap_detection() {
    let recv_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();

    let recv_config = ReceiverConfig {
        stream_id: 1,
        ..Default::default()
    };

    let receiver = UdpReceiver::new(recv_config, recv_addr).unwrap();

    // Create a raw socket to send packets with gaps
    let send_socket = UdpSocket::bind("127.0.0.1:0").unwrap();

    // Send packet with seq 0
    let mut builder = PacketBuilder::new(1, 0, 0);
    builder.try_add_message(MessageType::OrderNew, b"msg0");
    send_socket.send_to(&builder.finish(), recv_addr).unwrap();

    thread::sleep(Duration::from_millis(20));

    // Skip seq 1 and send seq 2 (creates gap)
    let mut builder = PacketBuilder::new(1, 2, 2);
    builder.try_add_message(MessageType::OrderNew, b"msg2");
    send_socket.send_to(&builder.finish(), recv_addr).unwrap();

    thread::sleep(Duration::from_millis(50));

    // Check for gap detection
    let recv_stats = receiver.stats();
    assert_eq!(recv_stats.gaps_detected, 1);

    let gaps = receiver.gaps();
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].expected_seq, 1);
    assert_eq!(gaps[0].received_seq, 2);

    // State should be degraded
    assert_eq!(receiver.state(), StreamState::Degraded);

    // Clear gaps
    receiver.clear_gaps();
    assert!(receiver.gaps().is_empty());
    assert_eq!(receiver.state(), StreamState::Active);
}

#[test]
fn test_duplicate_detection() {
    let recv_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();

    let recv_config = ReceiverConfig {
        stream_id: 1,
        ..Default::default()
    };

    let receiver = UdpReceiver::new(recv_config, recv_addr).unwrap();

    let send_socket = UdpSocket::bind("127.0.0.1:0").unwrap();

    // Send packet seq 0
    let mut builder = PacketBuilder::new(1, 0, 0);
    builder.try_add_message(MessageType::OrderNew, b"msg0");
    let packet = builder.finish();
    send_socket.send_to(&packet, recv_addr).unwrap();

    thread::sleep(Duration::from_millis(20));

    // Send packet seq 1
    let mut builder = PacketBuilder::new(1, 1, 1);
    builder.try_add_message(MessageType::OrderNew, b"msg1");
    send_socket.send_to(&builder.finish(), recv_addr).unwrap();

    thread::sleep(Duration::from_millis(20));

    // Re-send packet seq 0 (duplicate)
    send_socket.send_to(&packet, recv_addr).unwrap();

    thread::sleep(Duration::from_millis(50));

    let recv_stats = receiver.stats();
    assert_eq!(recv_stats.duplicates_dropped, 1);
    assert_eq!(recv_stats.messages_received, 2); // Only 2 unique messages
}

#[test]
fn test_stream_id_filtering() {
    let recv_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();

    // Only accept stream_id 1
    let recv_config = ReceiverConfig {
        stream_id: 1,
        ..Default::default()
    };

    let receiver = UdpReceiver::new(recv_config, recv_addr).unwrap();

    let send_socket = UdpSocket::bind("127.0.0.1:0").unwrap();

    // Send from stream 1 (should be accepted)
    let mut builder = PacketBuilder::new(1, 0, 0);
    builder.try_add_message(MessageType::OrderNew, b"stream1");
    send_socket.send_to(&builder.finish(), recv_addr).unwrap();

    // Send from stream 2 (should be filtered)
    let mut builder = PacketBuilder::new(2, 0, 0);
    builder.try_add_message(MessageType::OrderNew, b"stream2");
    send_socket.send_to(&builder.finish(), recv_addr).unwrap();

    thread::sleep(Duration::from_millis(50));

    let recv_stats = receiver.stats();
    // Packets received counts all packets
    assert_eq!(recv_stats.packets_received, 1);
    assert_eq!(recv_stats.messages_received, 1);
}

#[test]
fn test_high_throughput() {
    let recv_port = get_free_port();
    let send_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();
    let send_addr: SocketAddr = format!("127.0.0.1:{}", send_port).parse().unwrap();

    let recv_config = ReceiverConfig {
        stream_id: 1,
        channel_capacity: 100_000,
        ..Default::default()
    };

    let send_config = SenderConfig {
        stream_id: 1,
        target_addr: recv_addr,
        enable_heartbeats: false,
        max_batch_delay: Duration::from_micros(100),
        channel_capacity: 100_000,
        ..Default::default()
    };

    let receiver = UdpReceiver::new(recv_config, recv_addr).unwrap();
    let sender = UdpSender::new(send_config, send_addr).unwrap();

    let message_count = 10_000;
    let payload = vec![0u8; 100];

    let start = std::time::Instant::now();

    // Send messages
    for _ in 0..message_count {
        sender.try_send(MessageType::BookUpdate, payload.clone()).unwrap();
    }

    // Wait for processing
    thread::sleep(Duration::from_millis(500));

    let elapsed = start.elapsed();
    let send_stats = sender.stats();
    let recv_stats = receiver.stats();

    println!(
        "Sent {} messages in {:?} ({:.0} msg/sec)",
        send_stats.messages_sent,
        elapsed,
        send_stats.messages_sent as f64 / elapsed.as_secs_f64()
    );
    println!(
        "Received {} messages, {} packets",
        recv_stats.messages_received,
        recv_stats.packets_received
    );
    println!(
        "Batching ratio: {:.2} messages/packet",
        send_stats.messages_sent as f64 / send_stats.packets_sent as f64
    );

    // Should have received most messages (allow some loss in test environment)
    assert!(recv_stats.messages_received >= (message_count as u64 * 9) / 10);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_max_size_message() {
    let max_payload_size = MAX_PAYLOAD - MESSAGE_HEADER_LEN;
    let large_payload = vec![0xAB; max_payload_size];

    let mut builder = PacketBuilder::new(1, 0, 0);
    assert!(builder.try_add_message(MessageType::BookSnapshot, &large_payload));

    let packet_bytes = builder.finish();
    let packet = Packet::parse(&packet_bytes).unwrap();

    assert_eq!(packet.messages.len(), 1);
    assert_eq!(packet.messages[0].payload.len(), max_payload_size);
}

#[test]
fn test_empty_payload() {
    let mut builder = PacketBuilder::new(1, 0, 0);
    assert!(builder.try_add_message(MessageType::Heartbeat, &[]));

    let packet_bytes = builder.finish();
    let packet = Packet::parse(&packet_bytes).unwrap();

    assert_eq!(packet.messages.len(), 1);
    assert!(packet.messages[0].payload.is_empty());
}

#[test]
fn test_message_too_large() {
    let recv_port = get_free_port();
    let send_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();
    let send_addr: SocketAddr = format!("127.0.0.1:{}", send_port).parse().unwrap();

    let send_config = SenderConfig {
        stream_id: 1,
        target_addr: recv_addr,
        enable_heartbeats: false,
        ..Default::default()
    };

    let sender = UdpSender::new(send_config, send_addr).unwrap();

    // Try to send message larger than MTU
    let huge_payload = vec![0u8; MAX_MTU * 2];
    let result = sender.send(MessageType::BookSnapshot, huge_payload);

    assert!(matches!(result, Err(ProtocolError::MessageTooLarge { .. })));
}
