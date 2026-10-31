-- Maps a Salesforce record ID to the external ID that keys its record.
--
-- A Salesforce update or delete change event carries the changed fields
-- only, so it usually has no external ID. Its record ID is in the event
-- header. CDC capture writes this map from every event that has both
-- values, in the same transaction as the journal row. A later sparse event
-- for the record resolves its key here, even before the first event is
-- applied.
create table if not exists sync_salesforce_record (
    tenant text not null,
    object_name text not null,
    salesforce_id text not null,
    external_id text not null,
    updated_at timestamptz not null default now(),
    primary key (tenant, object_name, salesforce_id)
);

-- A record created from Postgres is known only by its link. CDC capture
-- looks the link up by its Salesforce ID.
create index if not exists sync_link_salesforce_id_idx
    on sync_link(tenant, object_name, salesforce_id)
    where salesforce_id is not null;

insert into force_sync_schema_migrations (version)
values (3)
on conflict (version) do nothing;
