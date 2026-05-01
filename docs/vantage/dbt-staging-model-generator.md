# 🔭 Vantage: Spec for dbt Staging Model Generator

## Overview
A utility to generate dbt (data build tool) staging model SQL queries from Salesforce `SObjectDescribe` metadata.

## User Story
As an Analytics Engineer, I want to automatically generate dbt staging models for Salesforce objects, so that I can quickly standardize and transform raw synced Salesforce data in my data warehouse without writing boilerplate SQL.

## The "So What?"
**What business problem does this solve?**
When companies sync Salesforce data to a warehouse (like Snowflake or BigQuery), analytics engineers must write "staging models" in dbt to clean up field names, cast types, and establish the base layer for downstream modeling. Writing these models manually for objects with hundreds of fields is tedious and error-prone. By generating the boilerplate `SELECT` statements and renaming fields to a standard casing (e.g., snake_case), we save hours of manual data engineering work and enforce naming conventions automatically.

## Metric Definition
- **Success =** Generated SQL must compile in dbt without syntax errors and correctly rename all Salesforce API names (e.g., `CustomField__c`) to the configured staging convention (e.g., `custom_field`).

## Gap Analysis
We can generate the DDL to create tables, but dbt requires SELECT statements (views) that map the raw source tables to internal staging formats. This bridges the gap between raw data replication and analytical data modeling.

## Acceptance Criteria
- Must generate a `WITH source AS (SELECT * FROM {{ source('salesforce', 'table_name') }})` CTE block.
- Must generate a `renamed AS` CTE block that selects all fields.
- Must convert Salesforce `CamelCase` or `Custom__c` field names into standard `snake_case` aliases.
- Must support generating configuration blocks (e.g., `{{ config(...) }}`).

## Out of Scope
- Executing the dbt models or interacting with the dbt CLI.
- Generating complex downstream dimensional models or aggregations.
