//! Pure deterministic calculators for directional packet and RTT evidence.

use std::collections::BTreeSet;

/// One valid authenticated packet observed by the authoritative receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketObservation {
    pub sequence_number: u32,
    pub received_at_ns: u64,
    pub payload_bytes: u16,
}

impl PacketObservation {
    pub const fn new(sequence_number: u32, received_at_ns: u64, payload_bytes: u16) -> Self {
        Self {
            sequence_number,
            received_at_ns,
            payload_bytes,
        }
    }
}

/// Aggregate evidence for one independently observed packet direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionalMetrics {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub duplicates: u64,
    pub out_of_order: u64,
    pub loss_pct: Option<f64>,
    pub jitter_ms_p50: Option<f64>,
    pub jitter_ms_p95: Option<f64>,
    pub throughput_kbps: Option<f64>,
}

/// Aggregate authenticated round-trip-time evidence in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RttMetrics {
    pub min: Option<f64>,
    pub mean: Option<f64>,
    pub p95: Option<f64>,
}

/// Invalid calculator input that must be rejected before reporting metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricError {
    SequenceOutsideExpectedRange {
        sequence_number: u32,
        packets_sent: u32,
    },
    NonMonotonicReceiveTime,
}

impl core::fmt::Display for MetricError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SequenceOutsideExpectedRange {
                sequence_number,
                packets_sent,
            } => write!(
                formatter,
                "sequence {sequence_number} is outside expected range 1..={packets_sent}"
            ),
            Self::NonMonotonicReceiveTime => {
                formatter.write_str("receive timestamps are not monotonic")
            }
        }
    }
}

impl std::error::Error for MetricError {}

/// Calculates one direction from receiver-ordered, valid authenticated observations.
pub fn calculate_directional_metrics(
    packets_sent: u32,
    measurement_duration_ns: u64,
    observations: &[PacketObservation],
) -> Result<DirectionalMetrics, MetricError> {
    let mut seen = BTreeSet::new();
    let mut greatest_sequence = None;
    let mut duplicates = 0_u64;
    let mut out_of_order = 0_u64;
    let mut received_payload_bytes = 0_u64;
    let mut unique_arrival_times = Vec::with_capacity(observations.len());
    let mut previous_receive_time = None;

    for observation in observations {
        if observation.sequence_number == 0 || observation.sequence_number > packets_sent {
            return Err(MetricError::SequenceOutsideExpectedRange {
                sequence_number: observation.sequence_number,
                packets_sent,
            });
        }
        if previous_receive_time.is_some_and(|previous| observation.received_at_ns < previous) {
            return Err(MetricError::NonMonotonicReceiveTime);
        }
        previous_receive_time = Some(observation.received_at_ns);

        if !seen.insert(observation.sequence_number) {
            duplicates += 1;
            continue;
        }
        if greatest_sequence.is_some_and(|greatest| observation.sequence_number < greatest) {
            out_of_order += 1;
        }
        greatest_sequence = Some(
            greatest_sequence.map_or(observation.sequence_number, |greatest: u32| {
                greatest.max(observation.sequence_number)
            }),
        );
        received_payload_bytes += u64::from(observation.payload_bytes);
        unique_arrival_times.push(observation.received_at_ns);
    }

    let packets_received = u64::try_from(seen.len()).expect("usize packet count fits in u64");
    let loss_pct = (packets_sent != 0).then(|| {
        100.0 * f64::from(packets_sent - packets_received as u32) / f64::from(packets_sent)
    });
    let mut jitter_samples_ms: Vec<f64> = unique_arrival_times
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect::<Vec<_>>()
        .windows(2)
        .map(|pair| pair[1].abs_diff(pair[0]) as f64 / 1_000_000.0)
        .collect();
    jitter_samples_ms.sort_by(f64::total_cmp);

    let throughput_kbps = (measurement_duration_ns != 0)
        .then(|| received_payload_bytes as f64 * 8_000_000.0 / measurement_duration_ns as f64);

    Ok(DirectionalMetrics {
        packets_sent: u64::from(packets_sent),
        packets_received,
        duplicates,
        out_of_order,
        loss_pct,
        jitter_ms_p50: nearest_rank(&jitter_samples_ms, 50),
        jitter_ms_p95: nearest_rank(&jitter_samples_ms, 95),
        throughput_kbps,
    })
}

/// Calculates RTT aggregates from elapsed monotonic nanosecond samples.
pub fn calculate_rtt_metrics(samples_ns: &[u64]) -> RttMetrics {
    if samples_ns.is_empty() {
        return RttMetrics {
            min: None,
            mean: None,
            p95: None,
        };
    }

    let mut sorted_ms: Vec<f64> = samples_ns
        .iter()
        .map(|sample| *sample as f64 / 1_000_000.0)
        .collect();
    sorted_ms.sort_by(f64::total_cmp);
    let total_ns: u128 = samples_ns.iter().map(|sample| u128::from(*sample)).sum();

    RttMetrics {
        min: sorted_ms.first().copied(),
        mean: Some(total_ns as f64 / samples_ns.len() as f64 / 1_000_000.0),
        p95: nearest_rank(&sorted_ms, 95),
    }
}

fn nearest_rank(sorted_samples: &[f64], percentile: usize) -> Option<f64> {
    if sorted_samples.is_empty() {
        return None;
    }
    let rank = (percentile * sorted_samples.len()).div_ceil(100);
    Some(sorted_samples[rank - 1])
}
