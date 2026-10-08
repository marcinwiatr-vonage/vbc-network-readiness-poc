use protocol::metrics::{
    MetricError, PacketObservation, calculate_directional_metrics, calculate_rtt_metrics,
};

#[test]
fn directional_metrics_count_unique_packets_and_impairments() {
    let observations = [
        PacketObservation::new(1, 0, 100),
        PacketObservation::new(3, 20_000_000, 100),
        PacketObservation::new(3, 21_000_000, 100),
        PacketObservation::new(2, 30_000_000, 100),
    ];

    let metrics = calculate_directional_metrics(4, 40_000_000, &observations)
        .expect("valid deterministic observations");

    assert_eq!(metrics.packets_sent, 4);
    assert_eq!(metrics.packets_received, 3);
    assert_eq!(metrics.duplicates, 1);
    assert_eq!(metrics.out_of_order, 1);
    assert_eq!(metrics.loss_pct, Some(25.0));
    assert_eq!(metrics.jitter_ms_p50, Some(10.0));
    assert_eq!(metrics.jitter_ms_p95, Some(10.0));
    assert_eq!(metrics.throughput_kbps, Some(60.0));
}

#[test]
fn rtt_metrics_use_arithmetic_mean_and_nearest_rank_p95() {
    let metrics = calculate_rtt_metrics(&[10_000_000, 20_000_000, 30_000_000, 100_000_000]);

    assert_eq!(metrics.min, Some(10.0));
    assert_eq!(metrics.mean, Some(40.0));
    assert_eq!(metrics.p95, Some(100.0));
}

#[test]
fn jitter_percentiles_use_absolute_interval_variation_and_nearest_rank() {
    let observations = [
        PacketObservation::new(1, 0, 172),
        PacketObservation::new(2, 10_000_000, 172),
        PacketObservation::new(3, 22_000_000, 172),
        PacketObservation::new(4, 52_000_000, 172),
        PacketObservation::new(5, 83_000_000, 172),
        PacketObservation::new(6, 124_000_000, 172),
    ];

    let metrics = calculate_directional_metrics(6, 124_000_000, &observations)
        .expect("valid deterministic observations");

    assert_eq!(metrics.jitter_ms_p50, Some(2.0));
    assert_eq!(metrics.jitter_ms_p95, Some(18.0));
}

#[test]
fn unavailable_inputs_produce_null_calculations_instead_of_zero() {
    let directional = calculate_directional_metrics(0, 0, &[])
        .expect("an unattempted direction has no packet observations");
    let rtt = calculate_rtt_metrics(&[]);

    assert_eq!(directional.packets_sent, 0);
    assert_eq!(directional.packets_received, 0);
    assert_eq!(directional.loss_pct, None);
    assert_eq!(directional.jitter_ms_p50, None);
    assert_eq!(directional.jitter_ms_p95, None);
    assert_eq!(directional.throughput_kbps, None);
    assert_eq!(rtt.min, None);
    assert_eq!(rtt.mean, None);
    assert_eq!(rtt.p95, None);
}

#[test]
fn directions_are_calculated_independently() {
    let uplink = calculate_directional_metrics(
        3,
        30_000_000,
        &[
            PacketObservation::new(1, 0, 100),
            PacketObservation::new(3, 20_000_000, 100),
        ],
    )
    .expect("valid uplink observations");
    let downlink = calculate_directional_metrics(
        3,
        30_000_000,
        &[
            PacketObservation::new(1, 0, 100),
            PacketObservation::new(2, 10_000_000, 100),
            PacketObservation::new(3, 20_000_000, 100),
        ],
    )
    .expect("valid downlink observations");

    assert_eq!(uplink.loss_pct, Some(100.0 / 3.0));
    assert_eq!(downlink.loss_pct, Some(0.0));
}

#[test]
fn invalid_sequences_and_receive_time_are_rejected() {
    assert_eq!(
        calculate_directional_metrics(2, 10, &[PacketObservation::new(0, 0, 100)]),
        Err(MetricError::SequenceOutsideExpectedRange {
            sequence_number: 0,
            packets_sent: 2,
        })
    );
    assert_eq!(
        calculate_directional_metrics(
            2,
            10,
            &[
                PacketObservation::new(1, 2, 100),
                PacketObservation::new(2, 1, 100),
            ],
        ),
        Err(MetricError::NonMonotonicReceiveTime)
    );
}
