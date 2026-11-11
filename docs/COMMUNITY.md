# The shared library and the Cecilia Community

Churches share music and organ setups with each other. Three pieces fit
together:

| | |
| --- | --- |
| **Cecilia** (this repository) | Browses the shared library (Library → Shared library, Organs → Find my organ), adds pieces and setups, and shares back (Share with other churches) |
| **[cecilia-community](https://github.com/garethcheyne/cecilia-community)** | The service: accounts, the review queue, the moderation page and the catalogue. Cloudflare Workers with D1 and R2; for now it runs on this computer |
| **[cecilia-library](https://github.com/garethcheyne/cecilia-library)** | The same library as plain files on GitHub: an open, permanent mirror written only by the service |

Cecilia reads whichever `CECILIA_LIBRARY_URL` points at: the service, the
GitHub mirror, or a local folder. All three serve the same `index.json`
catalogue and file paths.

## How things get in

1. **Shared from Cecilia.** Someone signs in (Settings → Cecilia Community,
   six-digit code by email) and presses *Share with other churches* on an
   organ setup or a piece. Cecilia sends it to the service with the sign-in
   token, which never leaves this machine for a browser.
2. **Automatic checks.** Shape and size, licence (music must be public domain
   or openly licensed), plain text only, rate limits.
3. **Review.** A moderator approves or rejects it (with a reason the sender
   sees) at `/moderate`. Trusted contributors skip the queue; banned ones can't
   sign in.
4. **Published.** It appears in the catalogue at once and is copied to the
   GitHub mirror.

Bulk additions are made by maintainers:

- **Open Hymnal Project hymns:** `cecilia library import openhymnal <abc zip or
  folder> <library folder>` converts ABC files to Cecilia scores, keeps only
  hymns whose tune and harmony are out of copyright worldwide (life + 70), and
  credits the project and source hymnal. Every conversion was checked against
  the Open Hymnal's own MIDI files.
- **Organ templates:** built from manufacturers' published stop lists, with
  sources in each setup's notes. They need *Learn every stop*, because makers
  don't publish per-stop MIDI numbers.
- **Checking a library folder:** `cecilia library check <folder>`; `cecilia
  library index <folder>` prints its catalogue.

## Organists and credit

Organists are kept in Cecilia's settings, on this computer. *Who's playing*
(beside Record) fills in "Played by" on new recordings. When a recording is
shared, the organist is named only if their profile says they've agreed;
otherwise it's credited to "A Cecilia organist".

## Running the service locally

```sh
cd ../cecilia-community
npm install
npm run db:migrate:local
npx wrangler dev --ip 127.0.0.1 --port 8787
```

Then start Cecilia with `CECILIA_LIBRARY_URL=http://127.0.0.1:8787/`.
Moderators are the addresses in `MODERATOR_EMAILS` in `.dev.vars` (not
committed); locally, sign-in codes are shown on screen instead of emailed.
Load the library into a fresh service with `scripts/import-library.mjs` (see
its README).
