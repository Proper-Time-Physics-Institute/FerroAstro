<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Research journal archive

These files mirror the "📓 Journal #N" comments on issue #1. They are the **historical record** of what was found and decided, and when.

## Annotation policy (settled 2026-10-19, red-team item I13)

1. **Append-only.** The original text of an entry is never rewritten or deleted.
2. **Every superseded or corrected passage gets an in-place annotation:**
   - italicised, in parentheses, placed directly after the affected text;
   - starting with `Corrected YYYY-MM-DD` (the original was wrong) or `Superseded YYYY-MM-DD` (a later decision replaced it);
   - naming the journal entry that made the change and the authoritative requirement or ADR.
3. **The requirement catalogue (`docs/requirements/`) and the ADRs are authoritative.** When a journal and a requirement disagree, the requirement wins. The journal annotation exists so readers can see why they differ.
4. **New findings go in a new entry**, never into an old one. Each new entry is posted on issue #1 first and then archived here.
5. **Evidence tags (`[V]`, `[P]`, `[C]`, `[S]`, UNVERIFIED) keep their original meaning.** A `[V]` that is later disproved is annotated as `Corrected`. It is not silently re-tagged.
