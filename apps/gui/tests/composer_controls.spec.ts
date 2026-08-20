// @vitest-environment jsdom

// Composer meta-row selectors: work mode, permission level, and model. Every
// mutation goes through the injected control dispatch and the pills re-render
// only from the returned Core state — including coupled changes the click
// never asked for.
import { describe, expect, test, vi } from "vitest";

import {
  renderD1Cockpit,
  type D1IntentResult,
  type D1RenderOptions,
} from "../src/screens/d1_cockpit";
import type { ComposerControlIntent } from "../src/models/composer";
import type { D1CockpitProjection } from "../src/models/workspace";
import { modelGroups } from "../src/components/composer_controls";
import { D1_PROJECTION } from "./support/d1_projection";

function result(
  projection: D1CockpitProjection,
  outcome: D1IntentResult["outcome"] = { state: "confirmed", reason: null },
): D1IntentResult {
  return { projection, pendingCommandId: null, outcome };
}

function mount(
  projection: D1CockpitProjection = D1_PROJECTION,
  sendComposerControl?: D1RenderOptions["sendComposerControl"],
  options: D1RenderOptions = {},
) {
  document.body.innerHTML = '<main id="app"></main>';
  const root = document.querySelector<HTMLElement>("#app")!;
  const idle = result(projection, { state: "idle", reason: null });
  const send = vi.fn(async () => idle);
  const poll = vi.fn(async () => idle);
  const controller = renderD1Cockpit(root, projection, send, poll, undefined, undefined, {
    poll: false,
    sendComposerControl,
    ...options,
  });
  return { root, controller, send, poll };
}

const noopControl = vi.fn(async () => result(D1_PROJECTION));

describe("composer control selectors", () => {
  test("renders the three selectors with the current Core values", () => {
    const { root, controller } = mount(D1_PROJECTION, noopControl);

    const meta = root.querySelector<HTMLElement>("[data-composer-meta]");
    expect(meta).not.toBeNull();
    expect(
      root.querySelector('[data-control-toggle="work_mode"]')?.textContent,
    ).toContain("Build");
    expect(
      root.querySelector('[data-control-toggle="permission"]')?.textContent,
    ).toContain("PERM Ask");
    expect(root.querySelector('[data-control-toggle="model"]')?.textContent).toContain(
      "deepseek · deepseek-v4-flash",
    );
    controller.dispose();
  });

  test("without a host dispatch the meta row is omitted entirely", () => {
    const { root, controller } = mount(D1_PROJECTION, undefined);
    expect(root.querySelector("[data-composer-meta]")).toBeNull();
    controller.dispose();
  });

  test("selectors are disabled while the composer is not editable", () => {
    const projection = {
      ...D1_PROJECTION,
      composer: { ...D1_PROJECTION.composer, editable: false },
    };
    const { root, controller } = mount(projection, noopControl);
    for (const kind of ["work_mode", "permission", "model"]) {
      expect(
        root.querySelector<HTMLButtonElement>(`[data-control-toggle="${kind}"]`)?.disabled,
      ).toBe(true);
    }
    controller.dispose();
  });

  test("the work-mode popover lists the four Core modes and marks the current one", () => {
    const { root, controller } = mount(D1_PROJECTION, noopControl);

    root.querySelector<HTMLButtonElement>('[data-control-toggle="work_mode"]')!.click();
    const popover = root.querySelector<HTMLElement>('[data-control-popover="work_mode"]');
    expect(popover).not.toBeNull();
    expect(popover?.getAttribute("role")).toBe("listbox");
    const options = Array.from(
      popover!.querySelectorAll<HTMLButtonElement>("[data-control-option]"),
    );
    expect(options).toHaveLength(4);
    expect(options.map((option) => option.getAttribute("aria-selected"))).toEqual([
      "false",
      "true",
      "false",
      "false",
    ]);
    // The current option receives keyboard focus when the popover opens.
    expect(document.activeElement?.getAttribute("aria-selected")).toBe("true");
    controller.dispose();
  });

  test("the permission popover lists the five Core levels", () => {
    const { root, controller } = mount(D1_PROJECTION, noopControl);
    root.querySelector<HTMLButtonElement>('[data-control-toggle="permission"]')!.click();
    expect(
      root.querySelectorAll('[data-control-popover="permission"] [data-control-option]'),
    ).toHaveLength(5);
    controller.dispose();
  });

  test("model options come only from published provider and adapter data, grouped by source", () => {
    const projection: D1CockpitProjection = {
      ...D1_PROJECTION,
      agentAdapters: [
        {
          agentId: "codex-acp",
          displayName: "Codex",
          startability: "ready",
          diagnostics: [],
          models: ["gpt-5.3-codex", "gpt-5.3-codex-mini"],
        },
      ],
    };
    expect(modelGroups(projection)).toEqual([
      { providerId: "deepseek", label: "deepseek", models: ["deepseek-v4-flash"] },
      {
        providerId: "codex-acp",
        label: "Codex",
        models: ["gpt-5.3-codex", "gpt-5.3-codex-mini"],
      },
    ]);

    const { root, controller } = mount(projection, noopControl);
    root.querySelector<HTMLButtonElement>('[data-control-toggle="model"]')!.click();
    const options = Array.from(
      root.querySelectorAll<HTMLButtonElement>(
        '[data-control-popover="model"] [data-control-option]',
      ),
    );
    expect(options).toHaveLength(3);
    expect(options[0]?.getAttribute("aria-selected")).toBe("true");
    expect(options[0]?.textContent).toContain("deepseek-v4-flash");
    expect(options[1]?.textContent).toContain("Codex");
    controller.dispose();
  });

  test("Escape closes the popover and returns focus to its pill", () => {
    const { root, controller } = mount(D1_PROJECTION, noopControl);
    root.querySelector<HTMLButtonElement>('[data-control-toggle="work_mode"]')!.click();
    const option = document.activeElement as HTMLElement;
    option.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));

    expect(root.querySelector('[data-control-popover="work_mode"]')).toBeNull();
    expect(document.activeElement?.getAttribute("data-control-toggle")).toBe("work_mode");
    controller.dispose();
  });

  test("an outside click closes the popover", () => {
    const { root, controller } = mount(D1_PROJECTION, noopControl);
    root.querySelector<HTMLButtonElement>('[data-control-toggle="permission"]')!.click();
    expect(root.querySelector('[data-control-popover="permission"]')).not.toBeNull();

    document.body.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(root.querySelector('[data-control-popover="permission"]')).toBeNull();
    controller.dispose();
  });

  test("arrow keys move focus through the options", () => {
    const { root, controller } = mount(D1_PROJECTION, noopControl);
    root.querySelector<HTMLButtonElement>('[data-control-toggle="work_mode"]')!.click();
    const first = document.activeElement as HTMLButtonElement;
    first.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    expect(document.activeElement).not.toBe(first);
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }),
    );
    expect(document.activeElement).toBe(first);
    controller.dispose();
  });

  test("selecting an option dispatches its control intent with the selected Lane", async () => {
    const dispatched: Array<[ComposerControlIntent, string | null]> = [];
    const control = vi.fn(async (intent: ComposerControlIntent, laneId: string | null) => {
      dispatched.push([intent, laneId]);
      return result(D1_PROJECTION);
    });
    const { root, controller } = mount(D1_PROJECTION, control);

    root.querySelector<HTMLButtonElement>('[data-control-toggle="work_mode"]')!.click();
    root
      .querySelector<HTMLButtonElement>('[data-control-option-key="work_mode:plan"]')!
      .click();
    await vi.waitFor(() => expect(control).toHaveBeenCalledOnce());

    expect(dispatched[0]?.[0]).toEqual({ type: "set_work_mode", mode: "plan" });
    expect(dispatched[0]?.[1]).toBe("lane-core");
    controller.dispose();
  });

  test("the selectors are aria-busy and disabled while a control call is in flight", async () => {
    let resolve!: (value: D1IntentResult) => void;
    const control = vi.fn(
      () => new Promise<D1IntentResult>((done) => (resolve = done)),
    );
    const { root, controller } = mount(D1_PROJECTION, control);

    root.querySelector<HTMLButtonElement>('[data-control-toggle="permission"]')!.click();
    root
      .querySelector<HTMLButtonElement>('[data-control-option-key="permission:auto"]')!
      .click();

    const meta = root.querySelector<HTMLElement>("[data-composer-meta]");
    expect(meta?.getAttribute("aria-busy")).toBe("true");
    expect(
      root.querySelector<HTMLButtonElement>('[data-control-toggle="permission"]')?.disabled,
    ).toBe(true);

    resolve(result(D1_PROJECTION));
    await vi.waitFor(() =>
      expect(
        root.querySelector<HTMLElement>("[data-composer-meta]")?.getAttribute("aria-busy"),
      ).toBe("false"),
    );
    controller.dispose();
  });

  test("coupled changes Core made render on both selectors, never the optimistic click", async () => {
    // The operator selects Plan; Core's coupling rule also flips the
    // permission level to ReadOnly and publishes both in the snapshot.
    const coupled: D1CockpitProjection = {
      ...D1_PROJECTION,
      environment: {
        ...D1_PROJECTION.environment,
        workMode: "plan",
        permissionLevel: "read_only",
      },
      statusbar: {
        ...D1_PROJECTION.statusbar,
        workMode: "plan",
        permissionLevel: "read_only",
      },
    };
    const control = vi.fn(async () => result(coupled));
    const { root, controller } = mount(D1_PROJECTION, control);

    root.querySelector<HTMLButtonElement>('[data-control-toggle="work_mode"]')!.click();
    root
      .querySelector<HTMLButtonElement>('[data-control-option-key="work_mode:plan"]')!
      .click();
    await vi.waitFor(() => {
      expect(
        root.querySelector('[data-control-toggle="work_mode"]')?.textContent,
      ).toContain("Plan");
      expect(
        root.querySelector('[data-control-toggle="permission"]')?.textContent,
      ).toContain("Read Only");
    });
    // The statusbar reflects the same returned truth.
    expect(root.querySelector('[data-sb-segment="mode"]')?.textContent).toContain("plan");
    expect(root.querySelector('[data-sb-segment="perm"]')?.textContent).toContain(
      "read_only",
    );
    controller.dispose();
  });

  test("a Core rejection renders as a role=alert message in the composer region", async () => {
    const control = vi.fn(async () =>
      result(D1_PROJECTION, {
        state: "rejected",
        reason: "provider `codex-acp` is not the active provider `deepseek`",
      }),
    );
    const { root, controller } = mount(D1_PROJECTION, control);

    root.querySelector<HTMLButtonElement>('[data-control-toggle="work_mode"]')!.click();
    root
      .querySelector<HTMLButtonElement>('[data-control-option-key="work_mode:review"]')!
      .click();
    await vi.waitFor(() => {
      const alert = root.querySelector('[data-composer-region] [data-d1-rejection]');
      expect(alert?.getAttribute("role")).toBe("alert");
      expect(alert?.textContent).toContain("not the active provider");
    });
    // The pills keep showing the state Core still publishes.
    expect(root.querySelector('[data-control-toggle="work_mode"]')?.textContent).toContain(
      "Build",
    );
    controller.dispose();
  });

  test("a host transport failure renders through the same rejection alert", async () => {
    const control = vi.fn(async () => {
      throw new Error("Core adapter is not connected");
    });
    const { root, controller } = mount(D1_PROJECTION, control);

    root.querySelector<HTMLButtonElement>('[data-control-toggle="permission"]')!.click();
    root
      .querySelector<HTMLButtonElement>('[data-control-option-key="permission:full_access"]')!
      .click();
    await vi.waitFor(() => {
      expect(root.querySelector("[data-d1-rejection]")?.textContent).toContain(
        "Core adapter is not connected",
      );
    });
    controller.dispose();
  });
});
