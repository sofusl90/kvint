# Kvint — Privacy Policy

**Effective date:** 2026-05-03

## What Kvint collects

Kvint does not collect, store, or transmit any personal data to the developer or any third-party server.

### Display name

When you use Kvint, the app generates a random display name for you (e.g. "brave-otter"). You can change this name in Settings. Your display name is:

- **Stored locally** on your device in the app's private Documents folder.
- **Visible to nearby players** on your local Wi-Fi network via Bluetooth/mDNS while you are in the lobby. It is never sent to a server or retained by anyone.

### Networking

Kvint uses [iroh](https://github.com/n0-computer/iroh) for peer-to-peer connectivity. When you use the "Invite a friend" feature to play across different networks, iroh connects through number0's relay infrastructure (`relay.iroh.network`) for NAT traversal. In this process:

- Your **IP address** and an **ephemeral, randomly-generated session ID** are shared with number0's relay servers to establish the connection. This session ID is discarded when you close the app; it is not linked to your identity.
- Your **display name** and **game moves** are transmitted end-to-end encrypted directly between you and your opponent and are not visible to the relay.
- number0's privacy policy applies to their relay infrastructure: [https://www.n0.computer/privacy](https://www.n0.computer/privacy)

Game moves (board coordinates) are transmitted directly to your opponent over an encrypted connection. No move history is stored or uploaded anywhere.

## What Kvint does not collect

- No account, email address, or Apple ID
- No device identifiers
- No analytics or crash reporting
- No advertising data

## Contact

If you have questions about this policy, contact: sofus@laubel.net
