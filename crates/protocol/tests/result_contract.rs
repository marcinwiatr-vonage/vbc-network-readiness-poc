use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const DIRECTIONAL_METRIC_FIELDS: [&str; 8] = [
    "packets_sent",
    "packets_received",
    "duplicates",
    "out_of_order",
    "loss_pct",
    "jitter_ms_p50",
    "jitter_ms_p95",
    "throughput_kbps",
];

fn proto_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("proto")
        .join(relative)
}

fn read_json(relative: &str) -> Value {
    let path = proto_path(relative);
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse {} as JSON: {error}", path.display()))
}

#[test]
fn schema_v1_defines_directional_metrics_without_mos_or_capacity() {
    let schema = read_json("result.schema.json");

    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["properties"]["report_schema_version"]["const"], 1);
    assert_eq!(schema["properties"]["protocol_version"]["const"], 2);
    assert!(schema["properties"].get("mos").is_none());
    assert!(schema["properties"].get("capacity").is_none());
    assert!(
        schema["properties"]
            .get("estimated_supported_calls")
            .is_none()
    );

    let required = schema["$defs"]["directional_metrics"]["required"]
        .as_array()
        .expect("directional metric required list");
    for field in DIRECTIONAL_METRIC_FIELDS {
        assert!(
            required.contains(&Value::String(field.to_owned())),
            "missing {field}"
        );
    }

    let unavailable = schema["$defs"]["unavailable_directional_metrics"]["properties"]
        .as_object()
        .expect("unavailable directional metric properties");
    assert_eq!(
        unavailable["observer"]["enum"],
        Value::Array(vec![
            Value::String("probe".into()),
            Value::String("agent".into())
        ])
    );
    for field in DIRECTIONAL_METRIC_FIELDS {
        assert_eq!(
            unavailable[field]["const"],
            Value::Null,
            "{field} is not null"
        );
    }
}

#[test]
fn completed_fixtures_keep_uplink_and_downlink_independent() {
    let clean = read_json("fixtures/clean-link.json");
    let loss = read_json("fixtures/directional-loss.json");
    let jitter = read_json("fixtures/high-jitter.json");

    for fixture in [&clean, &loss, &jitter] {
        assert_eq!(fixture["report_schema_version"], 1);
        assert_eq!(fixture["status"], "COMPLETED");
        assert_eq!(fixture["upstream"]["observer"], "probe");
        assert_eq!(fixture["downstream"]["observer"], "agent");
        for direction in ["upstream", "downstream"] {
            for field in DIRECTIONAL_METRIC_FIELDS {
                assert!(
                    !fixture[direction][field].is_null(),
                    "{direction}.{field} is null"
                );
            }
        }
    }

    assert_eq!(clean["upstream"]["loss_pct"], 0.0);
    assert_eq!(clean["downstream"]["loss_pct"], 0.0);
    assert!(loss["upstream"]["loss_pct"].as_f64().unwrap() > 0.0);
    assert_eq!(loss["downstream"]["loss_pct"], 0.0);
    assert!(
        jitter["downstream"]["jitter_ms_p95"].as_f64().unwrap()
            > clean["downstream"]["jitter_ms_p95"].as_f64().unwrap()
    );
}

#[test]
fn unavailable_and_not_tested_fixtures_use_null_not_zero() {
    let unreachable = read_json("fixtures/no-probe-response.json");
    let rejected = read_json("fixtures/session-rejected.json");

    assert_eq!(unreachable["status"], "UDP_UNREACHABLE_OR_BLOCKED");
    assert_eq!(
        unreachable["reachability"]["status"],
        "no_validated_response"
    );
    assert_eq!(rejected["status"], "SESSION_REJECTED");
    assert_eq!(rejected["reachability"]["status"], "not_tested");

    for fixture in [&unreachable, &rejected] {
        for direction in ["upstream", "downstream"] {
            for field in DIRECTIONAL_METRIC_FIELDS {
                assert!(
                    fixture[direction][field].is_null(),
                    "{direction}.{field} is not null"
                );
            }
        }
        for field in ["min", "mean", "p95"] {
            assert!(
                fixture["rtt_ms"][field].is_null(),
                "rtt_ms.{field} is not null"
            );
        }
    }
}
