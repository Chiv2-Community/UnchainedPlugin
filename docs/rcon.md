# RCON protocol v1

RCON is an authenticated, encrypted TCP command channel. There is no plaintext
fallback or protocol negotiation. Existing line-based clients must be updated.
The plugin no longer enables the legacy game-side RCON widget via `?rcon`.

## Cryptography

- Noise pattern: `Noise_NNpsk0_25519_AESGCM_SHA256`, implemented by `snow`.
- Prologue: the ASCII bytes `Unchained RCON v1`.
- PSK: 32 random bytes, configured as a file of 64 hexadecimal characters.
- Client is the initiator; server is the responder. Both handshake payloads are empty.

The two-message handshake authenticates the shared key before any command is
accepted. Ephemeral X25519 keys create fresh session keys. AES-256-GCM authenticates
each transport message; Noise maintains separate directional nonce counters.
Replayed, reordered, modified, or cross-session commands fail authentication.
Do not use Noise's stateless transport mode or reuse a session after an I/O or
authentication error.

Anyone with the PSK can impersonate a client or server. There are no individual
identities or revocation. Encryption does not protect a compromised host, command
lengths, traffic timing, or secrets deliberately written by game commands to logs.

## Framing

Every Noise handshake or transport message is prefixed with its length as an
unsigned 16-bit big-endian integer. The length excludes the two-byte prefix.

1. Client sends its initial Noise handshake message.
2. Server verifies it and sends its Noise handshake response.
3. Client verifies the response and both sides enter stateful transport mode.
4. Client sends one encrypted UTF-8 command per transport message.

Handshake messages are at most 128 bytes. Transport messages are at most 4112 bytes:
4096 bytes of plaintext plus the 16-byte authentication tag. Noise nonces are
implicit and are not additional bytes in the frame. Empty authenticated commands
are ignored. Newlines, carriage returns, NUL, and invalid UTF-8 are rejected.

A clean EOF between frames closes the session. A truncated frame, invalid size,
or authentication failure closes it without executing that frame. Commands from
earlier valid frames are not rolled back. Successful decryption authorizes a
command; it is then queued for the game thread. There are no command responses
or execution acknowledgments.

## Server limits

The listener binds only to `127.0.0.1`, accepts at most 32 simultaneous connections,
and applies ten-second socket read/write timeouts during the handshake. Authenticated
connections have a five-minute read timeout. These are per-I/O timeouts, not absolute
handshake deadlines. Use an SSH tunnel for remote access; no public bind option is added.

The PSK is loaded before binding. Missing or invalid key files disable RCON.
Key rotation requires a server restart. The RCON listener does not log key contents
or received command text.
