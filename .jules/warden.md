**2024-05-18 - [DoS Protection via Payload Capping]**
**Threat:** [Unbounded Deserialization/Memory Exhaustion via large API responses (e.g., calling `.json()`)]
**Defense:** [Replaced unbounded deserialization calls with a bounded `read_capped_body` stream that proactively rejects payloads exceeding predefined limits before allocations, preventing OOM crashes]