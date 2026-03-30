create table if not exists force_sync_outbox (
    outbox_id bigserial primary key,
    tenant text not null,
    object_name text not null,
    external_id text not null,
    source_cursor text not null check (
        length(source_cursor) > 0
        and source_cursor not like 'postgres-lsn:%'
        and source_cursor not like 'salesforce-replay-id:%'
        and source_cursor not like 'snapshot:%'
    ),
    op text not null check (op in ('upsert', 'delete')),
    tombstone boolean not null default false check (
        (op = 'upsert' and tombstone = false)
        or (op = 'delete' and tombstone = true)
    ),
    payload jsonb not null,
    created_at timestamptz not null default now(),
    processed_at timestamptz
);

create index if not exists force_sync_outbox_pending_idx
    on force_sync_outbox(created_at, outbox_id)
    where processed_at is null;

insert into force_sync_schema_migrations (version)
values (2)
on conflict (version) do nothing;
