# Design QA — GUI ACP Conversation Flow

## Scope

This QA is limited to the interaction requested in this change: the D1 center
surface must present one Core-owned ACP session as an ordered `YOU` / `VIDEN`
conversation, preserve earlier turns, and keep the composer below that flow.
Tool cards, permission contents, and the completeness of the Context Dock are
outside this focused pass.

## Source and implementation

- Authoritative source: `/var/folders/rm/hlc2td2x1rq11k_yknfsn1y80000gn/T/codex-clipboard-7004b8e0-f88e-4369-a7d6-8eb4d792a175.png`
- Normalized source: `apps/gui/evidence/0.1.0-rc.3/conversation-flow-reference-normalized.png`
- Native implementation: `apps/gui/evidence/0.1.0-rc.3/conversation-flow-implementation.jpeg`
- Source pixels: 3068 × 1862; normalized to 1229 × 746 without cropping.
- Implementation pixels: 1229 × 768, native macOS App capture.
- Density treatment: source width normalized to the native capture width;
  aspect ratio preserved; no crop or pixel-density interpolation was applied
  to the implementation.

## State and focus

- Source state: active coding turn with one user message, one assistant message,
  tool/test cards, live work, and a permission request.
- Implementation state: completed two-turn Codex ACP session restored from Core
  facts, with the composer ready for the next message.
- Full-view comparison: performed with the normalized source and implementation
  in the same visual comparison input.
- Focused region: ordered message headers, message bodies, vertical flow, and
  composer placement. The source's active permission/tool state was not
  reproduced because it is not required to prove ordered multi-turn dialogue.

## Review history

1. Initial native capture exposed a duplicate assistant response after restart.
2. The GUI now suppresses reconstructed transcript rows already represented by
   the canonical Core conversation while retaining non-dialogue rows.
3. A fresh Core conversation reducer and persisted ACP events restore the exact
   `YOU → VIDEN → YOU → VIDEN` order.
4. The final normalized comparison confirms the source interaction language:
   cyan `YOU`, amber `VIDEN`, unboxed message flow, and a bottom composer.

## Findings

- P0: none.
- P1: none.
- P2: none within the conversation-flow scope.
- P3: the deterministic verification response contains a Codex skills-budget
  warning, which is authentic Agent output rather than GUI placeholder copy.

## Final result

passed

