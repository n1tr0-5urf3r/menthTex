# menthTex agent guide

This is a lab-specific BibTeX normalizer and literature workspace for the Menth
chair at the University of Tübingen. The application is a single Rust/Axum
server with a React/Vite frontend, PostgreSQL metadata, and on-disk PDF storage.
`README.md` is the user/deployment guide; this file records the code map,
invariants, and editing hazards that are easiest to miss.

## Start here

- `backend/src/main.rs` owns application startup, shared state, formatter and
  lookup endpoints, OAuth/OIDC, `conferences.bib` mutations, PDF extraction,
  and static frontend serving.
- `backend/src/bibtex.rs` is the self-contained BibTeX parser and normalizer.
  Its unit tests are the best executable specification of formatting behavior.
- `backend/src/workspace.rs` owns workspace routes, authorization, SQL, file
  persistence, document processing/enrichment, notes, tags, and sharing.
- `backend/migrations/` is the PostgreSQL schema history. Migrations are embedded
  with `sqlx::migrate!` and applied when the backend starts.
- `frontend/src/main.jsx` imports `App.jsx`. `App.jsx` is both the public
  formatter and the top-level route switch; it renders `WorkspaceApp.jsx` for
  relative paths beginning with `/app`. Do not replace it with
  `WorkspaceApp.jsx` or delete `App.css`.
- `frontend/src/pathUtils.js` implements base-path detection. There is no React
  Router; navigation uses `history.pushState` and a synthetic `popstate` event.
- `conferences.bib` is mutable application state, not merely sample data.
  Authenticated definition-management requests rewrite it.
- `sample_literature.bib` is a formatter regression fixture used by a Rust test.
- `skills/bib-hygiene/SKILL.md` is a published Claude Code skill that drives the
  public `POST /api/format-entry` endpoint from a paper repository. It is a
  consumer of this API, so any change to the `format-entry` request or response
  shape must be reflected there too. Keep it deployment-neutral: it resolves the
  instance from `MENTHTEX_URL` and must not hardcode a host or a local path.
- `docker-compose.yml` is the committed, portable default. `docker-compose.deploy.yml`
  is gitignored and holds deployment-local overrides (internal DNS pins, volume
  layout). Never commit deployment specifics back into `docker-compose.yml`.
- `.github/workflows/ci.yml` verifies only; it never pushes an image. `.gitlab-ci.yml`
  owns image publishing and the `github_sync` mirror job, which force-pushes a single
  squashed snapshot commit to GitHub. GitLab stays authoritative; GitHub has no history.

## Development and verification

The backend always requires a reachable PostgreSQL database, even when auth is
disabled and only the public formatter is being used. The simplest full stack
is:

```bash
docker compose up --build
```

It is then available at `http://localhost:4242`. For split development, start
PostgreSQL separately, set `DATABASE_URL`, then run:

```bash
cd backend
cargo run

cd frontend
npm ci
npm run dev
```

Vite proxies `/api` to `127.0.0.1:3000`. Before handing off code changes, run
the checks relevant to the touched area:

```bash
cd backend && cargo test && cargo build
cd frontend && npm run lint && npm run build
```

CI uses Rust 1.97 and Node 22, runs the same backend/frontend checks (with a
release backend build), and publishes the production image from the default
branch. There are no browser or database integration tests; most backend tests
are pure unit tests.

## Runtime architecture and configuration

Startup in `main.rs` performs these operations synchronously: build HTTP/auth
configuration, connect to PostgreSQL (up to 30 attempts), run migrations,
create the storage root, assemble routes, optionally mount the built frontend,
and listen on `0.0.0.0:$PORT` (default `3000`). The request body limit is 32 MiB.

Important configuration:

- `DATABASE_URL` is mandatory.
- `STORAGE_ROOT` defaults to `storage/` beside the repository in development.
- `CONFERENCES_BIB_PATH` defaults to the repository `conferences.bib`.
- `FRONTEND_DIST` defaults to `frontend/dist`; if absent, only API routes are
  served.
- `EXTERNAL_METADATA_LOOKUP_ENABLED` defaults to true.
- `EXTERNAL_METADATA_LOOKUP_TIMEOUT_SECS` defaults to 6 seconds per service.
- `VITE_BASE_PATH` is a frontend build-time value. It must match reverse-proxy
  subpath hosting, including the `/menthtex/` production case.

The production image installs `poppler-utils`: both the public PDF lookup and
workspace PDF processing shell out to `pdfinfo` and `pdftotext`. The final
container serves the Vite build through Axum's SPA fallback.

Compose persists PostgreSQL and PDFs in named volumes and bind-mounts
`conferences.bib` so definition edits survive container replacement. Any
institution-specific `extra_hosts` entries are deployment workarounds: verify
the addresses before keeping or changing them, and do not assume they are
portable. `.env` is ignored and may contain secrets; never commit or echo it.

## Formatter behavior

`format_bibtex_input` optionally loads the shared definitions, parses the
entire input with the balanced BibTeX scanner, and calls `format_entries`.

- `@inproceedings` requires usable author, title, year, and booktitle fields.
- Normal `@article` requires usable author, title, and year. An article without
  a journal is deliberately emitted as `@misc`; it is not left untouched.
- IEEE-standard-like articles can omit authors and retain their original key.
- Only RFC-like `@misc` entries are normalized. Other `@misc` entries remain
  byte-for-byte untouched.
- Unsupported entry types and supported entries lacking required data remain
  untouched, with warnings where appropriate.
- Input `@STRING` and `@comment` entries pass through; duplicate inline/shared
  definitions are suppressed in the assembled output.
- Supported entries use a strict field allowlist. Removed fields are reported
  in the response and are expected behavior, not parser loss.

When key rewriting is enabled, keys use two surname letters from each of the
first two authors plus the last two year digits; a single author omits the
second fragment, transliteration handles common accents, and collisions gain
`_2`, `_3`, etc. `rewrite_reference_key` defaults to **true** in backend request
deserialization and in a fresh formatter UI draft. Explicitly send/set false
when processing entries already referenced by `\cite{...}`.

Titles are title-cased and emitted double-braced. Unicode dashes in titles,
author names, venue names, and page ranges are rewritten for LaTeX (en/em
dashes become `--`, exotic hyphens become `-`); all other non-ASCII characters,
including accented letters, pass through untouched. Months and page ranges are
normalized. Trivial whole-paper conference ranges beginning at page 1 are
dropped; journal ranges are retained. RFC output has a separate strict shape
and title normalization.

Venue resolution canonicalizes journal/booktitle names and matches aliases in
`conferences.bib`. A known definition is rendered as its token. An unknown long
name produces a proposed definition and up to three similarity suggestions;
short strings that look like abbreviations remain literal. Formatting never
persists a proposal automatically. Persistence requires an authenticated call
to `/api/add-definition` or the definition CRUD endpoints. The formatter UI
also performs client-side proposal resolution when rebuilding its displayed
output, so check both Rust and React behavior when changing this contract.

Public formatter workflows are paste/uploaded BibTeX, DOI lookup, RFC lookup,
and PDF metadata lookup. DOI requests fetch BibTeX from `doi.org`; RFC requests
use Datatracker and may repair incomplete records from RFC Editor metadata.

## Authentication and API boundaries

Authentication is optional and may expose GitLab OAuth, Authentik OIDC, or
both. A provider is enabled only when all of its required variables are set.
GitLab requests `read_api`; Authentik uses discovery, authorization code + PKCE,
nonce validation, and configurable scopes/group claim.

Allowed-group membership is checked only during login. A successful login
upserts `users` plus `user_identities`, creates a PostgreSQL session, and sets
the `menthtex_session` HttpOnly/SameSite=Lax cookie. Sessions default to eight
hours. Removing a user from an allowed group does not revoke an existing
session; logout or session expiry does. OAuth pending state is in process
memory for 600 seconds, so a restart or an uncoordinated multi-replica callback
invalidates an in-flight login.

Public/no-session endpoints include health, auth initiation/callback/status,
formatter lookups, and read-only `/api/definition-options`. Listing or mutating
the managed definition file requires a valid session. `/api/auth/me` is the
formatter's compact auth status; `/api/me` is the workspace user status.

All other workspace handlers authenticate before checking project access.
`public` project visibility therefore means visible to any authenticated user,
not anonymous internet access. Roles order as viewer < editor < owner:

- viewer: read tree/documents/files/notes/tags/members and export BibTeX;
- editor: upload, move, edit, reprocess, or delete documents; manage folders,
  notes, and tags;
- owner: edit/delete the project and manage memberships/visibility.

Only users who have signed in at least once can be added to a project. The
owner membership cannot be reassigned or removed through the member endpoints.

## Workspace persistence and PDF pipeline

The model is `Project -> Folder tree -> Documents -> notes/tags`, with separate
project memberships. Project creation makes a hidden system `Root` folder and
a visible default folder currently also named `Root`; code that needs the
default upload folder selects the first visible folder. Documents always have
a folder (`ON DELETE RESTRICT`), and non-empty folders cannot be deleted.

Uploaded PDFs are stored at
`$STORAGE_ROOT/projects/<project_id>/<document_id>/<sanitized_filename>`; the
database stores the path, SHA-256, extracted metadata, source/formatted BibTeX,
processing status, and tags. Upload processing happens within the request, not
in a background worker. It:

1. extracts `pdfinfo`, text, front-matter, DOI, venue, and keyword hints;
2. attempts DOI BibTeX when a trustworthy DOI is present;
3. optionally searches Crossref, OpenAlex, and Semantic Scholar by title;
4. applies only high-confidence metadata, otherwise records suggestions and
   warnings in `source_metadata_json`;
5. runs the normal formatter and derives automatic tags.

Manual metadata edits rebuild and reformat the document BibTeX. Reprocessing
prefers saved manual fields over freshly extracted values and refreshes auto
tags. Deleting a document/project removes database rows first and then attempts
filesystem cleanup; keep the PostgreSQL database, PDF volume, and
`conferences.bib` together in backups.

## Change checklist

- Formatter rule change: update `bibtex.rs` tests, including the sample-file
  regression when relevant, and verify the formatter diff UI contract.
- API response/request change: search both `App.jsx` and `WorkspaceApp.jsx`;
  there is no generated client or shared schema.
- Schema change: add a new numbered migration; never edit an already-deployed
  migration as if it were mutable schema.
- Auth change: check both providers, both `/api/*me` responses, cookie/subpath
  behavior, login-time group enforcement, and legacy GitLab group env fallbacks.
- Workspace file change: keep DB/file failure ordering and cleanup behavior in
  mind; they are not one atomic transaction.
- Definition change: preserve valid `@STRING` rendering and remember the
  in-process mutex does not coordinate multiple backend replicas.
- Frontend route/base-path change: test `/`, `/app`, `/app/projects/<id>`, and
  the configured production subpath with a direct browser refresh.
- Version bump: `backend/Cargo.toml`, `backend/Cargo.lock`, `frontend/package.json`,
  `frontend/package-lock.json`, the `FRONTEND_VERSION` fallback in both `App.jsx` and
  `WorkspaceApp.jsx`, and the trailing version line in `README.md` must all agree.
- Anything user-visible: the repo is mirrored to a public GitHub repository, so treat
  every tracked file as published. Deployment secrets and host pins belong in `.env`
  or `docker-compose.deploy.yml`, both gitignored.
