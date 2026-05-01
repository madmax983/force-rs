# 🔭 Vantage: Spec for LLM Context Generator

## Overview
A utility to convert Salesforce `SObjectDescribe` metadata into a highly condensed, token-optimized textual representation specifically tailored for Large Language Models (LLMs).

## User Story
As an AI Developer, I want to automatically convert verbose Salesforce object metadata into a dense, token-efficient text format, so that I can inject schema context into LLM prompts for generating SOQL queries or Apex code without exceeding context windows or paying for unnecessary tokens.

## The "So What?"
**What business problem does this solve?**
Developers are building AI agents that need to understand custom Salesforce schemas to write accurate code or queries. Passing a full JSON `describe` object consumes thousands of tokens and includes irrelevant metadata (like internal URLs or UI layout flags). By stripping away the noise and generating a dense representation, we dramatically reduce LLM inference costs and latency while improving the model's accuracy by focusing its attention only on structure, fields, and relationships.

## Metric Definition
- **Success =** 90% reduction in token count compared to the raw JSON `SObjectDescribe` payload, while retaining 100% of the field name, type, and relationship data necessary for an LLM to write a valid SOQL query.

## Gap Analysis
We have tools to generate strictly typed schemas (like GraphQL or Protobuf) and human-readable documentation, but no existing utility is optimized purely for LLM context windows where density and token economy are the primary concerns.

## Acceptance Criteria
- Must output a plain text representation of the schema.
- Must include options to omit field labels to save tokens (`include_labels`).
- Must include options to omit relationships (`include_relationships`).
- Must include an option to only output custom fields (`custom_fields_only`).
- Must map Salesforce types to readable equivalents for the LLM.

## Out of Scope
- Actually calling an LLM API or managing prompts.
- Generating the SOQL or Apex code itself.
