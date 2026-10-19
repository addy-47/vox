# Vox remote control: ticket plan

LAN-only v1, screen-on foreground use, built for you first. Off-LAN, Opus and background use are scheduled, not designed out.

## Rubric

## Extension guard rails

These cost little now and are what let off-LAN arrive later as one new Connector instead of a refactor.

1. A peer is a device ID plus key. Addresses are replaceable hints held as a candidate list, never the identity.
2. Every caller dials through a Connector. Only LanConnector exists in v1.
3. Authentication is a signed device credential at the app layer, so it rides unchanged over any future transport.
4. The wire protocol is versioned. A Hello message negotiates version, codec and capabilities.
5. One conformance suite (T1.6) must pass for every Link and Connector.

## Dependency waves

Everything inside one wave can be built in parallel once earlier waves land. Click any ticket to see what blocks it (yellow) and what it unblocks (green outline).

Select a ticket.

## Tickets

| ID | Ticket | Tag | Milestone | Size | Blocked by | Unblocks |
| --- | --- | --- | --- | --- | --- | --- |

Sizes: S about 1 day, M 2 to 3, L 4 to 5. Milestones: M1 proof without a phone, M2 phone works on Wi-Fi (dev token), M3 usable v1, M4 later. ★ marks the critical path through M1 to M3.