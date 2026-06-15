# Org Limits Prometheus Exporter

## User Story
As a DevOps Engineer, I want to export Salesforce API limits as Prometheus metrics, so that I can monitor org health in Grafana and trigger alerts before limits are exhausted.

## So What?
**What business problem does this solve?**
Exhausting Salesforce API limits causes immediate, cascading failures across all integrated systems. Monitoring these limits in a standard observability stack prevents costly outages.

## Metric Definition
- Success = Prometheus-compatible `/metrics` endpoint is exposed.
- Success = All limits from `/services/data/vXX.X/limits` are accurately represented as gauges.
- Success = Upstream Salesforce API calls are cached/rate-limited to avoid consuming the very limits being monitored.

## Gap Analysis
Currently, developers must write custom cron jobs or use heavy APM agent plugins to extract Salesforce limits. A lightweight, native Rust exporter integrated with the `force-rs` library provides a robust, low-overhead solution.

## Acceptance Criteria
- Must fetch all limits from the `/services/data/vXX.X/limits` endpoint.
- Must expose a standard HTTP `/metrics` endpoint serving Prometheus-formatted text.
- Must cache limit responses to avoid exhausting the API limits (configurable TTL).
- Must include metric labels for `limit_name` and `org_id`.

## Out of Scope
- OpenTelemetry gRPC exporting (Phase 2).
- Automatic scaling of downstream workers based on limits.
- AlertManager configuration generation.
