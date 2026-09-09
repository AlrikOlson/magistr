# Authentication

`magistr login` verifies OIDC ID tokens against the configured issuer's public
signing keys before creating a cached identity. An unreadable token, failed
signature, invalid claim, or unavailable required key ends the login without
writing an identity. GitHub uses its own authenticated-user endpoint and never
interprets an ID token as proof of GitHub identity.

## Provider and transport boundary

An OIDC provider needs a nonblank client ID and a scope containing the `openid`
token. Its configured issuer must be an HTTPS URL without credentials, query or
fragment. Discovery appends `/.well-known/openid-configuration` to the issuer
path, removing a trailing slash only when constructing that URL. Comparisons
between the configured issuer, discovery's `issuer`, and the token's `iss` are
exact string comparisons; URL normalization does not make different issuers
equivalent.

Discovery must advertise the device authorization endpoint, token endpoint and
`jwks_uri`. Each endpoint must use HTTPS without credentials or a fragment.
Query parameters are permitted on endpoints. Requests do not follow redirects,
including redirects to another HTTPS URL. A provider must publish its final
endpoint locations.

Each request has a 30-second timeout. JSON responses are limited to 1 MiB.
Discovery, JWKS and GitHub user requests must return successful HTTP statuses;
device polling separately interprets the protocol's error responses, such as
`authorization_pending`. A non-successful discovery or JWKS response containing
plausible JSON is still a failure.

This binds discovery and key retrieval to the configured provider and follows
the [OpenID Provider metadata requirements](https://openid.net/specs/openid-connect-discovery-1_0.html#ProviderMetadata)
and [discovery validation rules](https://openid.net/specs/openid-connect-discovery-1_0.html#ProviderConfigurationValidation).
No key URL or embedded key from a token header can replace the discovered JWKS.

## Signing keys and rotation

The verifier supports **RS256 only**. It rejects unsigned tokens, HMAC algorithms
and other signature algorithms even if a provider advertises them. A provider
using another algorithm must register an RS256 client to use this implementation.
There is no TLS-only signature-verification fallback.
ID tokens are limited to 32 KiB. Unsupported critical, encryption, compression
and unencoded-payload JOSE header extensions are rejected.

A token must carry a nonblank `kid` selecting an unambiguous RSA verification
key. Missing or blank key IDs are rejected. Optional JWK `alg`
must match RS256; optional `use` must be `sig`; optional `key_ops` must contain
exactly `verify`. Keys declaring an incompatible purpose or algorithm are not usable.
Ambiguous keys are refused rather than choosing whichever appears first.
These restrictions apply the [JWT algorithm-verification guidance](https://www.rfc-editor.org/rfc/rfc8725.html#section-3.1)
and [JWK usage and algorithm fields](https://www.rfc-editor.org/rfc/rfc7517.html#section-4).

The in-memory JWKS cache lasts at most five minutes and accepts at most 64 keys.
Lower `Cache-Control: max-age` values shorten that lifetime; `Age` reduces the
remaining lifetime, and `no-store` or `no-cache` prevents reuse. An unfamiliar
key ID can cause one bounded refresh, with a 30-second cooldown limiting further
miss or outage requests. A missing key after that refresh, a malformed key set
or a failed fetch ends verification; it does not start an unbounded fetch loop
or authorize an identity from stale data. A signature failure with a known key
ID fails immediately without retry; keys with the same ID are fetched again
only through normal expiration. Key refresh supports the provider's signing-key
rotation without retaining provider tokens on disk.

## Claim policy

Claims are trusted only after signature verification. The following table is
Magistr's device-login policy; the freshness window and stricter audience policy
are deliberate client choices within the [OIDC ID-token validation rules](https://openid.net/specs/openid-connect-core-1_0.html#IDTokenValidation).
Timestamps use integer Unix seconds. Optional security claims may be absent;
present null values or values with the wrong type are rejected.

| Claim | Required behavior |
| --- | --- |
| `iss` | Required string exactly matching the configured and discovered issuer. |
| `sub` | Required nonblank ASCII string, at most 255 bytes; used unchanged as the subject. |
| `aud` | Required string or nonempty string array containing only the configured client ID. Additional audiences are not trusted. |
| `azp` | Optional; when present, must equal the configured client ID. |
| `exp` | Required timestamp strictly after the verification time. It becomes the cached identity's exact expiration. |
| `iat` | Required timestamp from 60 seconds before this flow started through 60 seconds after verification time; must precede `exp`. |
| `nbf` | Optional timestamp; must not be after verification time. |
| `nonce` | Must be absent for this device-flow policy. An unexpected nonce is rejected. |

Magistr does not request a nonce in the device authorization request.
[RFC 8628](https://www.rfc-editor.org/rfc/rfc8628.html#section-3.1) defines no
nonce parameter and permits providers to ignore unknown request parameters.
The fresh device grant, validated ID token and bounded issuance-time window form
this implementation's login boundary. This does not claim a nonce replay cache
or support for authorization-code, implicit or hybrid OIDC flows.

The cache retains the authenticated `sub` separately from optional name and
email claims. Existing team identity and roster resolution still use email or
name for member matching, so those fields remain relevant to authorization.
Member matching does not establish email verification or use an
issuer-and-subject roster identity. An expired or missing OIDC `exp` never falls back to the access
token's `expires_in` or a default session lifetime.

## GitHub and cached identities

The GitHub flow uses the opaque access token only to request the authenticated
user from GitHub. A successful response must contain the immutable numeric user
ID; a login name cannot replace a missing ID. An OIDC token without an ID token
fails locally and is never sent to GitHub.

The identity cache stores provider, subject, display fields and timestamps. It
does not store an access token, refresh token, ID token or JWKS. Cached identities
expire at their recorded time; a cache read does not contact the provider or
discover revocation. The local cache remains trusted local state, not a signed
credential suitable for transferring to another machine or accepting from an
untrusted caller.

An OIDC cache key binds both the exact issuer and registered client ID:
`oidc:<issuer>#client_id=<base64url-without-padding(client_id)>`. A login for one
client cannot satisfy a team configured for another client at the same issuer.
GitHub identities keep the `github` key because their identity proof comes from
GitHub's authenticated-user API rather than an ID-token audience.

The cache schema is version 2. Loading a version 1 cache, a document without a
version or an unknown version discards all its identities; the user must log in
again. GitHub entries follow the same invalidation rule. Adding a new login or saving
the migrated cache cannot promote old entries to version 2.
Migration happens during deserialization, including direct library callers, and
does not claim to protect a cache file from deliberate local modification.

Process ownership and native state are covered separately by the
[project authority](../architecture/project-authority.md).
