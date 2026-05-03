# Spec: LLM Context Generator

## 👤 User Story
As a Developer, I want to extract an SObject's schema into a token-optimized format, so that I can provide exactly the necessary context to a Large Language Model (LLM) for generating SOQL or Apex without exceeding token limits or incurring unnecessary costs.

## 🤔 So What? (Business Value)
Passing full JSON `describe` objects to LLMs wastes thousands of tokens on irrelevant metadata. By filtering down to only what the LLM needs (structure, fields, and relationships), we reduce API costs and improve the LLM's response quality (less noise = better code generation).

## 📊 Metric Definition
- Success = Context string size is at least 80% smaller than the raw JSON `describe` output.
- Token consumption per prompt is reduced.

## 🕳️ Gap Analysis
Currently, we have schema parsers and visualizers, but no way to safely pass org-specific metadata to an LLM without overwhelming its context window. Standard library outputs (JSON) are too verbose for this specific job.

## ✅ Acceptance Criteria
- Must generate a highly condensed textual representation of an SObject.
- Must provide configuration options to include/exclude field labels to save tokens.
- Must provide configuration options to include/exclude relationship information.
- Must provide configuration options to filter for custom fields only.
- Must be deterministically sortable to ensure caching and consistent LLM behavior.

## 🚫 Out of Scope
- Direct LLM integration (e.g., calling OpenAI/Anthropic APIs). We only generate the context string.
- Reverse generation (e.g., parsing LLM output back into Salesforce metadata).
