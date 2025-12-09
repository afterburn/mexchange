use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput, BenchmarkId};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use udp_proto::*;

static PORT_COUNTER: AtomicU16 = AtomicU16::new(23000);

fn get_free_port() -> u16 {
    PORT_COUNTER.fetch_add(1, Ordering::SeqCst)
}

fn bench_packet_header_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_header");

    let header = PacketHeader::new(1, 100, 1000, 5);
    let mut buf = vec![0u8; PACKET_HEADER_LEN];

    group.throughput(Throughput::Bytes(PACKET_HEADER_LEN as u64));
    group.bench_function("write", |b| {
        b.iter(|| {
            header.write_to(black_box(&mut buf)).unwrap()
        })
    });

    group.finish();
}

fn bench_packet_header_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_header");

    let header = PacketHeader::new(1, 100, 1000, 5);
    let mut buf = vec![0u8; PACKET_HEADER_LEN];
    header.write_to(&mut buf).unwrap();

    group.throughput(Throughput::Bytes(PACKET_HEADER_LEN as u64));
    group.bench_function("read", |b| {
        b.iter(|| {
            PacketHeader::read_from(black_box(&buf)).unwrap()
        })
    });

    group.finish();
}

fn bench_message_header_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_header");

    let header = MessageHeader::new(MessageType::OrderNew, MessageFlags::NONE, 100);
    let mut buf = vec![0u8; MESSAGE_HEADER_LEN];

    group.throughput(Throughput::Bytes(MESSAGE_HEADER_LEN as u64));
    group.bench_function("write", |b| {
        b.iter(|| {
            header.write_to(black_box(&mut buf)).unwrap()
        })
    });

    group.finish();
}

fn bench_message_header_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_header");

    let header = MessageHeader::new(MessageType::OrderNew, MessageFlags::NONE, 100);
    let mut buf = vec![0u8; MESSAGE_HEADER_LEN];
    header.write_to(&mut buf).unwrap();

    group.throughput(Throughput::Bytes(MESSAGE_HEADER_LEN as u64));
    group.bench_function("read", |b| {
        b.iter(|| {
            MessageHeader::read_from(black_box(&buf)).unwrap()
        })
    });

    group.finish();
}

fn bench_packet_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_builder");

    let payload_sizes = [32, 64, 128, 256, 512];

    for size in payload_sizes {
        let payload = vec![0xABu8; size];

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::new("add_message", size), &payload, |b, payload| {
            b.iter(|| {
                let mut builder = PacketBuilder::new(1, 0, 0);
                builder.try_add_message(MessageType::OrderNew, black_box(payload));
                builder.finish()
            })
        });
    }

    group.finish();
}

fn bench_packet_builder_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_builder_batch");

    let message_counts = [1, 10, 50, 100];
    let payload = vec![0xABu8; 32]; // Small messages for batching

    for count in message_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("messages", count), &count, |b, &count| {
            b.iter(|| {
                let mut builder = PacketBuilder::new(1, 0, 0);
                for _ in 0..count {
                    if !builder.try_add_message(MessageType::OrderNew, black_box(&payload)) {
                        break;
                    }
                }
                builder.finish()
            })
        });
    }

    group.finish();
}

fn bench_packet_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_parse");

    let message_counts = [1, 5, 10, 20];
    let payload = vec![0xABu8; 32];

    for count in message_counts {
        // Build a packet with 'count' messages
        let mut builder = PacketBuilder::new(1, 0, 0);
        for _ in 0..count {
            builder.try_add_message(MessageType::OrderNew, &payload);
        }
        let data = builder.finish();

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("messages", count), &data, |b, data| {
            b.iter(|| {
                Packet::parse(black_box(data)).unwrap()
            })
        });
    }

    group.finish();
}

fn bench_end_to_end_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    let recv_port = get_free_port();
    let send_port = get_free_port();

    let recv_addr: SocketAddr = format!("127.0.0.1:{}", recv_port).parse().unwrap();
    let send_addr: SocketAddr = format!("127.0.0.1:{}", send_port).parse().unwrap();

    let recv_config = ReceiverConfig {
        stream_id: 1,
        recv_timeout: Duration::from_micros(100),
        ..Default::default()
    };

    let send_config = SenderConfig {
        stream_id: 1,
        target_addr: recv_addr,
        enable_heartbeats: false,
        max_batch_delay: Duration::from_micros(50),
        ..Default::default()
    };

    let receiver = UdpReceiver::new(recv_config, recv_addr).unwrap();
    let sender = UdpSender::new(send_config, send_addr).unwrap();

    let payload = vec![0xABu8; 64];

    group.throughput(Throughput::Elements(1));
    group.bench_function("send_message", |b| {
        b.iter(|| {
            sender.try_send(MessageType::OrderNew, black_box(payload.clone())).ok()
        })
    });

    // Clean up
    drop(sender);
    drop(receiver);

    group.finish();
}

fn bench_serialization_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization_throughput");

    // Simulate a full order message payload
    let order_payload = vec![0u8; 64]; // Typical order size

    group.throughput(Throughput::Bytes(64));
    group.bench_function("order_serialize", |b| {
        b.iter(|| {
            let mut builder = PacketBuilder::new(1, 0, 0);
            builder.try_add_message(MessageType::OrderNew, black_box(&order_payload));
            builder.finish()
        })
    });

    // Build a packet to parse
    let mut builder = PacketBuilder::new(1, 0, 0);
    builder.try_add_message(MessageType::OrderNew, &order_payload);
    let packet_data = builder.finish();

    group.throughput(Throughput::Bytes(packet_data.len() as u64));
    group.bench_function("order_deserialize", |b| {
        b.iter(|| {
            Packet::parse(black_box(&packet_data)).unwrap()
        })
    });

    group.finish();
}

fn bench_high_throughput_scenario(c: &mut Criterion) {
    let mut group = c.benchmark_group("high_throughput");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    // Benchmark building 1000 messages worth of packets
    let payload = vec![0xABu8; 32];
    let message_count = 1000;

    group.throughput(Throughput::Elements(message_count as u64));
    group.bench_function("build_1000_messages", |b| {
        b.iter(|| {
            let mut packets = Vec::new();
            let mut builder = PacketBuilder::new(1, 0, 0);
            let mut packet_seq = 0u64;
            let mut msg_seq = 0u64;

            for _ in 0..message_count {
                if !builder.try_add_message(MessageType::OrderNew, black_box(&payload)) {
                    let msg_count = builder.msg_count();
                    packets.push(builder.finish());
                    packet_seq += 1;
                    msg_seq += msg_count as u64;
                    builder = PacketBuilder::new(1, packet_seq, msg_seq);
                    builder.try_add_message(MessageType::OrderNew, &payload);
                }
            }
            if !builder.is_empty() {
                packets.push(builder.finish());
            }
            packets
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_packet_header_write,
    bench_packet_header_read,
    bench_message_header_write,
    bench_message_header_read,
    bench_packet_builder,
    bench_packet_builder_batch,
    bench_packet_parse,
    bench_serialization_throughput,
    bench_end_to_end_throughput,
    bench_high_throughput_scenario,
);
criterion_main!(benches);
