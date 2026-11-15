# Strict Pulsar analytics oracles

Frozen from Aurora Client 04967aa38b1484622762b091e71f886dfb75086d.
Each deterministic gzip contains `raw` (the delivered source recording, including
legacy labels) and `oracle` (the format-7 envelope embedded in its delivered HTML).
The set contains 24 recordings and 26 envelopes including manual child exchanges.

Tests compare the complete derived payload and metric definitions recursively,
including unavailable values, and independently normalize original sources.
Only exported names and manual announcement redaction receive an explicit privacy
projection. Legacy normalization must be idempotent and never rewrites source files.

Scenarios 02100001 through 02100008 retain the original report coverage. 02100020
through 02100027 cover strict attacks, named damage causes, defensive shield
outcomes, deaths, leaving, a named killer and a proven Draw. Suffixes 40 through 45
cover each legacy attack label; 46 covers aggregate-only totals and 47 covers
legacy incoming damage. Missing per-hit evidence must not be manufactured.
