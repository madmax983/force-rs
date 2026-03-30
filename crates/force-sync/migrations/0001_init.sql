create table if not exists sync_journal (
    journal_id bigserial primary key,
    tenant text not null,
    object_name text not null,
    external_id text not null,
    source text not null check (source in ('salesforce', 'postgres')),
    source_cursor text not null,
    observed_at timestamptz not null,
    operation text not null check (operation in ('upsert', 'delete')),
    tombstone boolean not null default false,
    payload jsonb not null,
    payload_hash bytea not null,
    schema_version integer not null default 1
);

create unique index if not exists sync_journal_source_cursor_uidx
    on sync_journal(source, source_cursor);

create index if not exists sync_journal_object_observed_idx
    on sync_journal(object_name, observed_at);

create table if not exists sync_link (
    link_id bigserial primary key,
    tenant text not null,
    object_name text not null,
    external_id text not null,
    salesforce_id text,
    postgres_id text,
    last_source text,
    last_source_cursor text,
    last_payload_hash bytea,
    tombstone boolean not null default false,
    updated_at timestamptz not null default now()
);

create unique index if not exists sync_link_identity_uidx
    on sync_link(tenant, object_name, external_id);

create table if not exists sync_task (
    task_id bigserial primary key,
    status text not null,
    lease_owner text,
    lease_until timestamptz,
    priority integer not null default 0,
    next_attempt_at timestamptz not null default now(),
    task_kind text not null,
    target_key text,
    payload jsonb,
    attempt_count integer not null default 0,
    last_error text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists sync_task_ready_idx
    on sync_task(status, next_attempt_at, lease_until, priority desc);

create table if not exists sync_checkpoint (
    checkpoint_id bigserial primary key,
    stream_name text not null unique,
    cursor_position bigint not null default 0,
    cursor text,
    updated_at timestamptz not null default now()
);

create table if not exists sync_conflict (
    conflict_id bigserial primary key,
    tenant text not null,
    object_name text not null,
    external_id text not null,
    field_name text not null,
    left_value jsonb,
    right_value jsonb,
    resolution text,
    created_at timestamptz not null default now()
);

create table if not exists sync_dead_letter (
    dead_letter_id bigserial primary key,
    task_id bigint,
    tenant text,
    object_name text,
    external_id text,
    error_message text not null,
    payload jsonb,
    created_at timestamptz not null default now()
);

create table if not exists sync_export_watermark (
    export_name text primary key,
    watermark bigint not null default 0,
    updated_at timestamptz not null default now()
);

create table if not exists force_sync_schema_migrations (
    version integer primary key,
    applied_at timestamptz not null default now()
);

insert into force_sync_schema_migrations (version)
values (1)
on conflict (version) do nothing;
