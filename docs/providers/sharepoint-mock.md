# SharePoint Mock Provider

PR-05 introduces a deterministic SharePoint-style external reference provider for local development and demo flows.

## Current Scope

The provider does not integrate with Microsoft Graph or live SharePoint APIs.

Instead it implements the shared external-reference contract and returns deterministic fake payloads for:

- BTG documents
- RFI records
- site visit notes

The payload model is stable for the same input request and config seed, including:

- stable record IDs
- stable ordering
- stable source URLs
- stable dates
- stable nested content

## Supported Families

### BTG

- `document_id`
- `building_id`
- `floor_id`
- `title`
- `version`
- `sections[]`
- `source_url`
- `last_updated`

### RFI

- `rfi_id`
- `building_id`
- `floor_id`
- `question`
- `answer`
- `status`
- `date`
- `source_url`

### Site Visit

- `visit_id`
- `building_id`
- `floor_id`
- `summary`
- `findings[]`
- `date`
- `source_url`

## Config Shape

Current config fields:

- `seed`
- `tenant_id`

The `seed` anchors deterministic payload generation. `tenant_id` is used to generate stable source URLs and tenant-scoped demo records.

