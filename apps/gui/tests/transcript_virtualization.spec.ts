// @vitest-environment jsdom

import { expect, test } from "vitest";

import { BoundedTranscript } from "../src/screens/d1_cockpit";

test("50k rows remain bounded while history scroll keeps its anchor", () => {
  const transcript = new BoundedTranscript(240);
  for (let index = 0; index < 50_000; index += 1) {
    transcript.append({ id: `row-${index}`, kind: "assistant", content: `output-${index}` });
  }
  expect(transcript.rows).toHaveLength(240);
  expect(transcript.rows.at(0)?.id).toBe("row-49760");
  expect(transcript.anchor).toBe("row-49999");

  transcript.setFollowLatest(false, "row-49900");
  for (let index = 50_000; index < 50_037; index += 1) {
    transcript.append({ id: `row-${index}`, kind: "assistant", content: `output-${index}` });
  }
  expect(transcript.anchor).toBe("row-49900");
  expect(transcript.newOutputCount).toBe(37);
  expect(transcript.visible(180, 36).some((row) => row.id === "row-49900")).toBe(true);
});

test("10k event bursts and resize reads keep a bounded visible window", () => {
  const transcript = new BoundedTranscript(240);
  for (let index = 0; index < 10_000; index += 1) {
    transcript.append({ id: `event-${index}`, kind: "tool", content: `event-${index}` });
  }
  expect(transcript.visible(180, 36)).toHaveLength(7);
  expect(transcript.visible(720, 36)).toHaveLength(22);
  expect(transcript.rows).toHaveLength(240);
});
