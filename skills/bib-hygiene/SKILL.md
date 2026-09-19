---
name: bib-hygiene
description: Normalize a paper's BibTeX (references.bib / *.bib) through a menthTex formatter instance, then report and optionally apply the changes. Cleans citation keys, title case, and venue abbreviations, and strips fields the house style does not keep (abstract, url, keywords, ...). Use when the user asks to clean up, normalize, format, lint, or check the bibliography / .bib file.
---

# BibTeX hygiene via menthTex

Normalize a paper's BibTeX by sending it through a [menthTex](https://github.com/n1tr0-5urf3r/menthTex)
instance — the authoritative style engine for the group's BibTeX — then report
what changed and, on confirmation, apply it.

Do **not** reimplement the formatting rules here. The API is the single source
of truth for the style, and it is deliberately opinionated. Hand-formatting
defeats the point and produces entries that drift from everyone else's.

## Finding the service

Resolve the base URL in this order, and stop at the first hit:

1. The `MENTHTEX_URL` environment variable.
2. A `menthtex_url` key in the repository's Claude settings, or a base URL the
   user has previously given you in this project.
3. Ask the user for their instance URL.

The only endpoint this skill needs — `POST {base}/api/format-entry` — is public
and requires no authentication.

If the host is unreachable, say so and offer to run one locally:

```sh
git clone https://github.com/n1tr0-5urf3r/menthTex && cd menthTex
cp .env.example .env
docker compose up --build   # then use http://localhost:4242 as the base
```

Do **not** fall back to hand-formatting.

A local instance ships with its own `conferences.bib`, so venue abbreviations
will differ from a shared deployment. If the user's group runs a shared
instance, prefer it — that is where the agreed abbreviations live.

## Request and response contract

`POST {base}/api/format-entry`, JSON body:

```json
{
  "input": "<one or many raw BibTeX entries; a whole .bib file is fine>",
  "include_string_definitions": false,
  "use_conferences_bib": true,
  "rewrite_reference_key": false,
  "keep_doi": false
}
```

| Field | Notes |
| ----- | ----- |
| `input` | Raw BibTeX. Multi-entry input is expected and handled. |
| `use_conferences_bib` | Resolve venues against the instance's shared `@STRING` definitions. Keep `true`. |
| `include_string_definitions` | Emit the `@STRING{...}` definitions alongside the entries. Set `true` only if the user wants them inlined in the `.bib`. |
| `rewrite_reference_key` | **Server default is `true`.** Always send it explicitly. See the key-rewrite policy below. |
| `keep_doi` | Retain `doi` fields, which the style otherwise strips. Set `true` only on request. |

Response fields that matter:

- `output` — the full normalized BibTeX, all entries concatenated.
- `entry_count`, `formatted_count`, `untouched_count`.
- `warnings` — global warnings, e.g. a venue kept literal because nothing matched.
- `proposed_definitions[]` — new venue abbreviations the formatter suggests, each
  with `token`, `raw_value`, `expanded_value`, `definition_text`, and up to three
  ranked `suggestions[]` for existing definitions that look similar. These are
  **never** added automatically. Always surface them as FLAG items.
- `output_definitions[]` — the definitions actually referenced by the output.
- `entries[]` — per-entry detail: `key`, `entry_type`, `status`
  (`formatted` | `untouched`), `original_text`, `output_text`, `warnings`,
  `removed_fields`, `normalized_fields`.

Entries the formatter does not support come back with `status: "untouched"` and
their original text. Never rewrite those yourself.

## Key-rewrite policy (the important safety rule)

The formatter can regenerate citation keys (`foo2023` → `DoRo23`). On an
existing paper that **breaks every `\cite{oldkey}`** in the source.

The server defaults `rewrite_reference_key` to `true`, so omitting it silently
renames keys. Always send it explicitly, and default to `false`: preserve
existing keys, normalize everything else.

If the user does want canonical keys (`rewrite_reference_key: true`):

1. Run the format and build the old→new key map from `entries[]`.
2. Grep the LaTeX sources for every citation command using each old key —
   `\cite`, `\citep`, `\citet`, `\citeauthor`, `\autocite`, `\parencite`,
   `\textcite`, and friends. Search the whole tree, not just one directory;
   layouts vary (`sections/`, `chapters/`, `content/`, or a flat root).
3. Report the rename map and the exact citation sites that would change.
4. Only after the user confirms, apply **both** the new `.bib` and the citation
   edits together, so no dangling references remain.

Never rewrite keys in the `.bib` without updating the citations in the same step.

## Workflow

1. **Find the bibliography.** Read the main LaTeX file for
   `\bibliography{...}` or `\addbibresource{...}`. Otherwise use `*.bib` at the
   repository root, commonly `references.bib`. If the user names a file, use it.
2. **Read the file(s).** Send the whole file as one `input` per request. For very
   large bibliographies, chunk on entry boundaries (a line starting with `@`) —
   never mid-entry.
3. **Call the API** with the flags above and parse the JSON.
4. **Report** (format below). Do not write anything yet.
5. **Confirm, then apply.** On confirmation write `output` back to the `.bib`
   file, plus any citation edits if keys were rewritten. Suggest committing or
   branching first so the change is auditable with `git diff`.

```sh
jq -n --rawfile input references.bib '{
  input: $input,
  include_string_definitions: false,
  use_conferences_bib: true,
  rewrite_reference_key: false,
  keep_doi: false
}' | curl -sS -X POST "$MENTHTEX_URL/api/format-entry" \
       -H 'Content-Type: application/json' --data @- > /tmp/bib-hygiene.json
```

Build the body with `jq --rawfile` rather than interpolating the file into a
shell string: BibTeX is full of braces, backslashes, and quotes that will
otherwise corrupt the JSON.

## Report format

- **Summary** — `entry_count` total, `formatted_count` changed,
  `untouched_count` left as-is.
- **Per changed entry** — for each `entries[]` item with `status == "formatted"`:
  the `key`, a one-line note of `normalized_fields` and `removed_fields`, and any
  per-entry `warnings`. Show before→after only where the change is non-obvious.
- **FLAG — needs human review**, never auto-applied:
  - `proposed_definitions` — list each `token` and `expanded_value`, plus the
    top `suggestions[]`. A close suggestion usually means the venue name has a
    typo or a variant spelling, and the existing abbreviation should be used
    instead of creating a near-duplicate. Adding a definition for real is done
    by a signed-in user in the menthTex UI, under **Manage abbreviations**.
  - Global `warnings`, especially venues kept literal because nothing matched.
  - `untouched` entries the user might have expected to be formatted — name
    them, so nothing is silently skipped.
  - If key-rewrite is on: the full old→new map and the affected citation sites.
- **Close** by naming the fields the style strips (`abstract`, `url`,
  `keywords`, `doi`, `publisher`, `issn`, ...), so the user is not surprised
  that metadata is gone.

## Notes

- This is a best-effort tool. Tell the user to eyeball the diff before
  committing.
- Never invent BibTeX or citation keys.
- Never add venue abbreviations yourself. That is gated behind an authenticated
  UI on purpose: the shared `conferences.bib` is group-wide state, and a
  careless addition becomes everyone's near-duplicate abbreviation.
