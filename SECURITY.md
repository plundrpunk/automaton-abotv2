# ABot v3 Trust Boundary

## The Core Rule

**The body cannot grant itself privileges. Only AMS can.**

ABot v3 follows an open-core model where the Rust runtime (the "body") is open-source and AMS (the "head") is proprietary. Since anyone can fork the body, we enforce a strict trust boundary: the body sends **claims** about who it is, and AMS returns **grants** defining what it's allowed to do.

## Claims vs Grants

### Claims (body → AMS)

The body sends identity metadata during birth ritual. These are descriptive, not authoritative:

| Field | Example | Source |
|---|---|---|
| `hand_name` | `backend-engineer` | HAND.toml |
| `archetype` | `engineer` | HAND.toml |
| `domain` | `Backend Engineering` | HAND.toml |
| `description` | `Writes backend code...` | HAND.toml |
| `persona_role` | `Backend Engineer` | HAND.toml |
| `persona_style` | `precise, test-aware...` | HAND.toml |
| `default_model` | `kimi-k2.5` | HAND.toml |
| `goals` | `[...]` | HAND.toml |
| `tags` | `[...]` | HAND.toml |
| `version` | `3.0.0` | Binary |
| `runtime` | `rust` | Binary |
| `sandbox` | `wasmtime` | Binary |

AMS may use claims for display, logging, and routing decisions, but never for access control.

### Grants (AMS → body)

AMS looks up the seeded agent record and returns authoritative operating limits:

| Field | Example | Authority |
|---|---|---|
| `trust_tier` | `1` | `agents.trust_tier` in DB |
| `agent_class` | `worker` | `agents.agent_class` in DB |
| `enable_tools` | `true` | `agents.config->enable_tools` |
| `max_iterations` | `8` | `agents.config->max_iterations` |
| `warn_threshold` | `85` | `agents.warn_threshold` |
| `critical_threshold` | `98` | `agents.critical_threshold` |
| `nanny_managed` | `false` | `agents.nanny_managed` |
| `default_model` | `kimi-k2.5` | `agents.config->default_model` |

The body MUST operate within these limits. A forked body that ignores grants will be flagged by AMS governance.

## What Happens When a Body Is Unrecognized

If a body registers with an `agent_id` that has no matching seeded agent in the database, AMS returns restrictive default grants:

```json
{
  "trust_tier": 3,
  "agent_class": "untrusted",
  "enable_tools": false,
  "max_iterations": 3,
  "nanny_managed": true
}
```

This means an unknown body gets minimum privileges, no tool access, and mandatory nanny oversight.

## Attack Scenarios and Mitigations

### Forked body claims higher trust

A malicious fork could add `"trust_tier": 0` to its birth metadata. This has no effect because AMS ignores trust claims from the body entirely — grants come from the seeded agent database.

### Forked body ignores grants

A malicious fork could locally override `max_iterations` or `enable_tools`. AMS mitigates this through:

1. **Heartbeat monitoring**: AMS tracks execution patterns and can detect bodies operating outside their grants
2. **Governance FSM**: The containment system can escalate from MONITOR → WARN → PAUSE → KILL
3. **Nanny oversight**: High-risk bodies can be flagged for mandatory nanny review

### Body spoofs another agent's name

A malicious fork could claim to be `backend-engineer` to get its grants. Mitigations:

1. **API key authentication**: Each body needs a valid `AUTOMATON_AMS_API_KEY` to register
2. **Fleet deduplication**: AMS tracks registered agent IDs and can detect duplicate registrations
3. **Future**: Manifest signing with Ed25519 (infrastructure exists in `abot-security`, not yet enforced)

## Design Rationale

This trust model exists because the body is open-source. When the project scales:

- Competitors can fork the Rust runtime
- Users can modify their local bodies
- Third parties can build custom bodies

But they all need AMS for lifecycle management, memory, governance, and fleet coordination. The grants system ensures AMS stays in control of what any body is actually allowed to do, regardless of what it claims about itself.
