# 🔭 Vantage: Spec for Org Limits Prometheus Exporter

**Business problem:**
Enterprise Salesforce organizations have strict resource limits (API calls, storage, async executions, etc.). Exceeding these limits can cause critical business processes to fail, integrations to break, and require emergency purchases of additional capacity. Currently, administrators must manually check limits or rely on reactive email alerts when limits are nearly exhausted. There is no automated, continuous observability pipeline to monitor these limits proactively in modern infrastructure dashboards.

**Gap Analysis:**
While Salesforce provides a REST API endpoint to retrieve organization limits, there is no standardized, out-of-the-box solution to continuously ingest this data into industry-standard observability platforms like Prometheus and Grafana. Engineering teams often have to build and maintain custom polling scripts, dealing with authentication, error handling, and metric formatting from scratch.

**Success metric:**
Success = A standalone exporter that can authenticate with Salesforce, poll the limits endpoint, and expose the maximum, remaining, and usage percentages for all limits in the Prometheus OpenMetrics format, with configurable polling intervals to minimize API consumption overhead.

👤 **User Story:**
As a Site Reliability Engineer, I want an out-of-the-box exporter for Salesforce limits, so that I can ingest quotas into Prometheus and trigger Grafana alerts before we exhaust our daily allocations.

✅ **Acceptance Criteria:**
- Must expose a standard metrics endpoint that serves current limit data in the OpenMetrics format.
- Must translate limit maximums, remaining values, and usage percentages into distinct gauge metrics.
- Must support customizable polling intervals to prevent the exporter itself from exhausting API request limits.
- Must handle authentication failures or network disruptions gracefully without crashing the exporter process.
- Must support all standard limits and dynamically discover any additional limits returned by the API.

🚫 **Out of Scope:**
- Providing built-in Grafana dashboards or alerting rules (the exporter only provides the metrics).
- Remediating limit exhaustion automatically (e.g., stopping integrations).
